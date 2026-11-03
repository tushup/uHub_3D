//! The one add path shared by the app and the CLI (spec §A1.2):
//! validate, path-trace the thumbnail when the draft has none,
//! save. Also re-rendering an existing asset's thumbnail in place.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use image::ImageFormat;
use uuid::Uuid;

use crate::error::LibError;
use crate::images::write_hdr_thumbnail;
use crate::library::{AssetEntry, Library};
use crate::mesh::{load_mesh, uv_sphere, MeshGroup};
use crate::model::{Asset, AssetDraft, AssetType, AttributeDraft, MaterialDraft};
use crate::render::{bind_materials, render, Film, Progress, RenderMaterial, RenderOptions, Scene};
use crate::view::View;

fn io_invalid(context: &str, e: impl std::fmt::Display) -> LibError {
    LibError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("{context}: {e}"),
    ))
}

/// Expand draft materials to one render material per mesh group,
/// following the binding rule (defaults for unbound groups).
fn render_materials(
    groups: &[MeshGroup],
    drafts: &[MaterialDraft],
) -> Result<Vec<RenderMaterial>, LibError> {
    let converted: Vec<RenderMaterial> = drafts
        .iter()
        .map(RenderMaterial::from_draft)
        .collect::<Result<_, _>>()?;
    Ok(bind_materials(groups, drafts)
        .iter()
        .map(|&i| {
            if i == usize::MAX {
                RenderMaterial::default()
            } else {
                converted[i].clone()
            }
        })
        .collect())
}

fn model_scene(draft: &AssetDraft, view: &View) -> Result<Scene, LibError> {
    let src = draft
        .model_src
        .as_ref()
        .ok_or_else(|| LibError::MissingFile(PathBuf::new()))?;
    let mesh = load_mesh(src)?;
    let materials = render_materials(&mesh.groups, &draft.materials)?;
    let bounds = mesh.bounds();
    Ok(Scene::build(&mesh, &bounds, view, &materials))
}

fn material_scene(drafts: &[MaterialDraft], view: &View) -> Result<Scene, LibError> {
    // The Material preview sphere tiles its UVs 2x1 (spec §A1.2).
    let mut mesh = uv_sphere(96, 48);
    for uv in &mut mesh.uvs {
        uv[0] *= 2.0;
    }
    let materials = render_materials(&mesh.groups, drafts)?;
    let bounds = mesh.bounds();
    Ok(Scene::build(&mesh, &bounds, view, &materials))
}

fn saved_drafts(asset: &Asset, dir: &Path) -> Vec<MaterialDraft> {
    asset
        .materials
        .iter()
        .map(|m| MaterialDraft {
            original_name: m.original_name.clone(),
            name: m.name.clone(),
            attributes: m
                .attributes
                .iter()
                .map(|a| AttributeDraft {
                    kind: a.kind,
                    value: a.value,
                    texture_src: a.texture.as_ref().map(|t| dir.join(t)),
                    strength: a.strength,
                    normal_y: a.normal_y,
                })
                .collect(),
        })
        .collect()
}

/// Add an asset: validate first (nothing renders or is written for
/// an invalid draft), render the thumbnail for 3D/Material drafts
/// without one, then save. The view that was rendered is stored.
pub fn add_asset(
    lib: &Library,
    draft: &AssetDraft,
    opts: &RenderOptions,
    cancel: &AtomicBool,
    on_pass: impl FnMut(&Progress, &Film),
) -> Result<AssetEntry, LibError> {
    lib.validate(draft)?;
    let mut draft = draft.clone();
    let mut rendered_thumb: Option<PathBuf> = None;
    if draft.thumbnail_src.is_none() && draft.asset_type != AssetType::Hdri {
        let view = draft.view.unwrap_or_default();
        let scene = match draft.asset_type {
            AssetType::Model => model_scene(&draft, &view)?,
            _ => material_scene(&draft.materials, &view)?,
        };
        let img = render(&scene, opts, cancel, on_pass)?;
        let tmp = std::env::temp_dir().join(format!("uhub-thumbnail-{}.png", Uuid::new_v4()));
        img.save_with_format(&tmp, ImageFormat::Png)
            .map_err(|e| io_invalid("Failed to write rendered thumbnail", e))?;
        draft.thumbnail_src = Some(tmp.clone());
        rendered_thumb = Some(tmp);
        draft.view = Some(view);
    }
    let entry = lib.save(&draft);
    if let Some(tmp) = rendered_thumb {
        let _ = fs::remove_file(tmp);
    }
    entry
}

/// Replace `name` inside `dir` atomically: write beside it, rename
/// over. The original survives a failed write.
fn write_atomic(
    dir: &Path,
    name: &str,
    write: impl FnOnce(&Path) -> Result<(), LibError>,
) -> Result<(), LibError> {
    let tmp = dir.join(format!(".{name}-{}.tmp", Uuid::new_v4()));
    let result = write(&tmp).and_then(|_| fs::rename(&tmp, dir.join(name)).map_err(LibError::from));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn write_asset_json(dir: &Path, asset: &Asset) -> Result<(), LibError> {
    let json = serde_json::to_string_pretty(asset)
        .map_err(|e| LibError::Parse(e.to_string()))?;
    write_atomic(dir, "asset.json", |tmp| {
        fs::write(tmp, &json).map_err(LibError::from)
    })
}

/// Re-render an asset's thumbnail from its stored (or given) view
/// and store that view. HDRI assets regenerate the 1024x512
/// tonemapped thumbnail instead.
pub fn rerender(
    lib: &Library,
    entry: &AssetEntry,
    view: Option<View>,
    opts: &RenderOptions,
    cancel: &AtomicBool,
    on_pass: impl FnMut(&Progress, &Film),
) -> Result<AssetEntry, LibError> {
    if !entry.dir.starts_with(&lib.root) {
        return Err(LibError::OutsideLibrary(entry.dir.clone()));
    }
    let asset = entry.asset.clone();
    match asset.r#type {
        AssetType::Hdri => {
            let info = asset
                .hdri
                .as_ref()
                .ok_or_else(|| LibError::Parse("HDRI asset without hdri info".into()))?;
            let src = entry.dir.join(&info.file);
            let (rotation, intensity) = (info.rotation, info.intensity);
            write_atomic(&entry.dir, "thumbnail.png", |tmp| {
                write_hdr_thumbnail(&src, tmp, rotation, intensity)
            })?;
            Ok(entry.clone())
        }
        _ => {
            let view = view.or(asset.view).unwrap_or_default();
            let drafts = saved_drafts(&asset, &entry.dir);
            let scene = match asset.r#type {
                AssetType::Model => {
                    let info = asset
                        .model
                        .as_ref()
                        .ok_or_else(|| LibError::Parse("model asset without model info".into()))?;
                    let mesh = load_mesh(&entry.dir.join(&info.file))?;
                    let materials = render_materials(&mesh.groups, &drafts)?;
                    let bounds = mesh.bounds();
                    Scene::build(&mesh, &bounds, &view, &materials)
                }
                _ => material_scene(&drafts, &view)?,
            };
            let img = render(&scene, opts, cancel, on_pass)?;
            write_atomic(&entry.dir, "thumbnail.png", |tmp| {
                img.save_with_format(tmp, ImageFormat::Png)
                    .map_err(|e| io_invalid("Failed to write rendered thumbnail", e))
            })?;
            let mut updated = asset.clone();
            updated.view = Some(view);
            updated.thumbnail = Some("thumbnail.png".to_string());
            write_asset_json(&entry.dir, &updated)?;
            Ok(AssetEntry {
                dir: entry.dir.clone(),
                asset: updated,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AttrKind, AttrValue};

    fn fx(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn root() -> (tempfile::TempDir, Library) {
        let d = tempfile::tempdir().unwrap();
        let lib = Library { root: d.path().to_path_buf() };
        (d, lib)
    }

    fn fast_opts() -> RenderOptions {
        RenderOptions { size: 64, seconds: 60.0, max_samples: Some(1), seed: 0, denoise: false }
    }

    fn model_draft() -> AssetDraft {
        AssetDraft {
            asset_type: AssetType::Model,
            name: "Cube".into(),
            category: "Furniture".into(),
            tags: vec![],
            model_src: Some(fx("cube_2mat.fbx")),
            hdri_src: None,
            thumbnail_src: None,
            materials: vec![],
            rotation: 0.0,
            intensity: 1.0,
            view: None,
        }
    }

    fn material_draft() -> AssetDraft {
        AssetDraft {
            asset_type: AssetType::Material,
            name: "Oak".into(),
            category: "Wood".into(),
            tags: vec![],
            model_src: None,
            hdri_src: None,
            thumbnail_src: None,
            materials: vec![MaterialDraft {
                original_name: "Oak".into(),
                name: "Oak".into(),
                attributes: vec![AttributeDraft {
                    kind: AttrKind::BaseColor,
                    value: Some(AttrValue::Color([0.4, 0.25, 0.1])),
                    texture_src: None,
                    strength: None,
                    normal_y: None,
                }],
            }],
            rotation: 0.0,
            intensity: 1.0,
            view: Some(View { yaw: -30.0, pitch: 15.0, zoom: 1.0 }),
        }
    }

    #[test]
    fn add_asset_validates_before_render() {
        let (_d, lib) = root();
        let mut draft = model_draft();
        draft.model_src = Some(fx("does-not-exist.fbx"));
        let called = std::cell::Cell::new(false);
        let result = add_asset(&lib, &draft, &fast_opts(), &AtomicBool::new(false), |_, _| {
            called.set(true);
        });
        assert!(matches!(result, Err(LibError::MissingFile(_))), "{result:?}");
        assert!(!called.get());
        assert!(lib.scan().is_empty());
    }

    #[test]
    fn add_material_writes_rgba_512_thumbnail_and_view() {
        let (_d, lib) = root();
        let opts = RenderOptions { size: 512, seconds: 60.0, max_samples: Some(1), seed: 0, denoise: false };
        let entry = add_asset(&lib, &material_draft(), &opts, &AtomicBool::new(false), |_, _| {})
            .unwrap();
        let thumb = entry.dir.join("thumbnail.png");
        assert_eq!(image::image_dimensions(&thumb).unwrap(), (512, 512));
        let img = image::open(&thumb).unwrap();
        assert!(matches!(img, image::DynamicImage::ImageRgba8(_)), "RGBA thumbnail");
        assert_eq!(entry.asset.view, material_draft().view);
        assert_eq!(entry.asset.thumbnail.as_deref(), Some("thumbnail.png"));
    }

    #[test]
    fn add_with_thumbnail_src_skips_render() {
        let (_d, lib) = root();
        let mut draft = model_draft();
        draft.thumbnail_src = Some(fx("red.png"));
        let called = std::cell::Cell::new(false);
        let entry = add_asset(&lib, &draft, &fast_opts(), &AtomicBool::new(false), |_, _| {
            called.set(true);
        })
        .unwrap();
        assert!(!called.get());
        assert_eq!(
            image::image_dimensions(entry.dir.join("thumbnail.png")).unwrap(),
            (64, 64)
        );
    }

    #[test]
    fn add_hdri_thumbnail_1024x512() {
        let (_d, lib) = root();
        let draft = AssetDraft {
            asset_type: AssetType::Hdri,
            name: "Sky".into(),
            category: "Skies".into(),
            tags: vec![],
            model_src: None,
            hdri_src: Some(fx("sky.hdr")),
            thumbnail_src: None,
            materials: vec![],
            rotation: 0.0,
            intensity: 1.0,
            view: None,
        };
        let called = std::cell::Cell::new(false);
        let entry = add_asset(&lib, &draft, &fast_opts(), &AtomicBool::new(false), |_, _| {
            called.set(true);
        })
        .unwrap();
        assert!(!called.get());
        assert_eq!(
            image::image_dimensions(entry.dir.join("thumbnail.png")).unwrap(),
            (1024, 512)
        );
    }

    #[test]
    fn rerender_updates_view_and_thumbnail() {
        let (_d, lib) = root();
        let entry = add_asset(&lib, &model_draft(), &fast_opts(), &AtomicBool::new(false), |_, _| {})
            .unwrap();
        let before = fs::read(entry.dir.join("thumbnail.png")).unwrap();
        let new_view = View { yaw: 120.0, pitch: -10.0, zoom: 1.5 };
        let updated = rerender(
            &lib,
            &entry,
            Some(new_view),
            &fast_opts(),
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(updated.asset.view, Some(new_view));
        let after = fs::read(entry.dir.join("thumbnail.png")).unwrap();
        assert_ne!(before, after, "thumbnail changes with the view");
        // And it persists on disk.
        let scanned = lib.scan();
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].asset.view, Some(new_view));
    }

    #[test]
    fn plan_save_matches_save() {
        let (d, lib) = root();
        let draft = model_draft();
        let planned = lib.validate(&draft).unwrap();
        let entry = add_asset(&lib, &draft, &fast_opts(), &AtomicBool::new(false), |_, _| {})
            .unwrap();
        assert_eq!(planned.dir, entry.dir);
        let mut on_disk: Vec<String> = fs::read_dir(&entry.dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        let mut want = planned.files.clone();
        want.sort();
        assert_eq!(on_disk, want, "planned {:?} vs disk {:?}", planned.files, on_disk);
        assert!(d.path().join("3D").exists());
    }

    #[test]
    fn cancel_leaves_no_folder() {
        let (_d, lib) = root();
        let cancel = AtomicBool::new(true);
        let result = add_asset(&lib, &model_draft(), &fast_opts(), &cancel, |_, _| {});
        assert!(matches!(result, Err(LibError::Cancelled)), "{result:?}");
        assert!(lib.scan().is_empty());
        assert!(!lib.asset_dir(AssetType::Model, "Furniture", "Cube").exists());
    }
}
