use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use uuid::Uuid;

use crate::error::LibError;
use crate::images::{hdr_size, write_hdr_thumbnail, write_thumbnail};
use crate::model::{Asset, AssetDraft, AssetType, Attribute, HdriInfo, Material, MaterialDraft, ModelInfo};
use crate::naming::{sanitize_name, texture_ext, texture_file_name, unique_names};
use crate::objfix::{mtllib_of, rewrite_mtl, rewrite_obj};
use crate::parse::parse_model;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetEntry {
    pub dir: PathBuf,
    pub asset: Asset,
}

/// The exact outcome of an add: target folder and file list,
/// rendered thumbnails included. Produced by `Library::validate`;
/// used by the CLI `--dry-run` and the dialog's Saves-to card so
/// naming has one source of truth.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSave {
    pub dir: PathBuf,
    /// Sanitized folder name for the asset; `<Name>` while the
    /// name field is empty or unsanitizable (see `Library::plan`).
    pub dir_name: String,
    /// `dir` relative to the library root's parent, forward
    /// slashes, trailing slash — what the Saves-to card shows.
    pub rel_dir: String,
    pub files: Vec<String>,
}

pub(crate) fn planned_files(draft: &AssetDraft, name: &str) -> Vec<String> {
    let mut files = Vec::new();
    let final_names = unique_names(
        &draft
            .materials
            .iter()
            .map(|m| m.name.clone())
            .collect::<Vec<_>>(),
    );
    match draft.asset_type {
        AssetType::Model => {
            if let Some(src) = &draft.model_src {
                let ext = src
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                files.push(format!("{name}.{ext}"));
                if ext == "obj" {
                    files.push(format!("{name}.mtl"));
                }
            }
            for (draft_mat, final_name) in draft.materials.iter().zip(&final_names) {
                for attr in &draft_mat.attributes {
                    if let Some(src) = &attr.texture_src {
                        if let Ok(ext) = texture_ext(src) {
                            files.push(texture_file_name(name, Some(final_name), attr.kind, &ext));
                        }
                    }
                }
            }
        }
        AssetType::Material => {
            if let Some(mat) = draft.materials.first() {
                for attr in &mat.attributes {
                    if let Some(src) = &attr.texture_src {
                        if let Ok(ext) = texture_ext(src) {
                            files.push(texture_file_name(name, None, attr.kind, &ext));
                        }
                    }
                }
            }
        }
        AssetType::Hdri => {
            files.push(format!("{name}.hdr"));
        }
    }
    // Every add ends with a thumbnail (override, path-traced render
    // or HDRI tonemap) and the manifest.
    files.push("thumbnail.png".to_string());
    files.push("asset.json".to_string());
    files
}

pub struct Library {
    pub root: PathBuf,
}

/// Formats are checked before existence so a wrong extension reports
/// `UnsupportedFormat` even when the file also does not exist.
fn validate_sources(draft: &AssetDraft) -> Result<(), LibError> {
    match draft.asset_type {
        AssetType::Model => {
            let src = draft
                .model_src
                .as_ref()
                .ok_or_else(|| LibError::MissingFile(PathBuf::new()))?;
            let ext = src
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            if !matches!(ext.as_deref(), Some("fbx" | "obj")) {
                return Err(LibError::UnsupportedFormat(src.display().to_string()));
            }
        }
        AssetType::Hdri => {
            let src = draft
                .hdri_src
                .as_ref()
                .ok_or_else(|| LibError::MissingFile(PathBuf::new()))?;
            let ext = src
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            if ext.as_deref() != Some("hdr") {
                return Err(LibError::UnsupportedFormat(src.display().to_string()));
            }
        }
        AssetType::Material => {}
    }
    if let Some(p) = &draft.thumbnail_src {
        texture_ext(p)?;
    }
    for m in &draft.materials {
        for a in &m.attributes {
            if let Some(p) = &a.texture_src {
                texture_ext(p)?;
            }
        }
    }
    let mut sources: Vec<&PathBuf> = Vec::new();
    if let Some(p) = &draft.model_src {
        sources.push(p);
    }
    if let Some(p) = &draft.hdri_src {
        sources.push(p);
    }
    if let Some(p) = &draft.thumbnail_src {
        sources.push(p);
    }
    for m in &draft.materials {
        for a in &m.attributes {
            if let Some(p) = &a.texture_src {
                sources.push(p);
            }
        }
    }
    for p in sources {
        if !p.exists() {
            return Err(LibError::MissingFile(p.clone()));
        }
    }
    Ok(())
}

fn copy_attributes(
    draft_mat: &MaterialDraft,
    tmp: &Path,
    asset_name: &str,
    material_name: Option<&str>,
) -> Result<Vec<Attribute>, LibError> {
    let mut out = Vec::new();
    for attr in &draft_mat.attributes {
        let texture = match &attr.texture_src {
            Some(src) => {
                let ext = texture_ext(src)?;
                let file_name = texture_file_name(asset_name, material_name, attr.kind, &ext);
                fs::copy(src, tmp.join(&file_name))?;
                Some(file_name)
            }
            None => None,
        };
        out.push(Attribute {
            kind: attr.kind,
            value: attr.value,
            texture,
            strength: attr.strength,
            normal_y: attr.normal_y,
        });
    }
    Ok(out)
}

impl Library {
    pub fn asset_dir(&self, t: AssetType, category: &str, name: &str) -> PathBuf {
        self.root.join(t.folder()).join(category).join(name)
    }

    /// Plan a save without validating anything: never fails. An
    /// empty or unsanitizable category/name becomes the `<Category>`
    /// / `<Name>` placeholder, so the Saves-to card can always show
    /// the target shape. With valid input the folder is exactly the
    /// one `validate` (and therefore `save`) will use. The file list
    /// needs the whole draft, so it stays empty here; `validate`
    /// and the `plan_save` command fill it in.
    pub fn plan(&self, asset_type: AssetType, category: &str, name: &str) -> PlannedSave {
        let dir_name = sanitize_name(name).unwrap_or_else(|| "<Name>".to_string());
        let category_dir = sanitize_name(category).unwrap_or_else(|| "<Category>".to_string());
        let dir = self.asset_dir(asset_type, &category_dir, &dir_name);
        let root_name = self
            .root
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string());
        let rel_dir = format!(
            "{root_name}/{}/{category_dir}/{dir_name}/",
            asset_type.folder()
        );
        PlannedSave {
            dir,
            dir_name,
            rel_dir,
            files: Vec::new(),
        }
    }

    /// Validate a draft and plan its save without writing anything:
    /// name and category rules, target folder, source formats and
    /// existence, and the exact file list (see `PlannedSave`).
    pub fn validate(&self, draft: &AssetDraft) -> Result<PlannedSave, LibError> {
        let name = sanitize_name(&draft.name)
            .ok_or_else(|| LibError::InvalidName(draft.name.clone()))?;
        sanitize_name(&draft.category)
            .ok_or_else(|| LibError::InvalidName(draft.category.clone()))?;
        let mut planned = self.plan(draft.asset_type, &draft.category, &draft.name);
        if planned.dir.exists() {
            return Err(LibError::NameExists(name));
        }
        validate_sources(draft)?;
        planned.files = planned_files(draft, &name);
        Ok(planned)
    }

    pub fn save(&self, draft: &AssetDraft) -> Result<AssetEntry, LibError> {
        let planned = self.validate(draft)?;
        let name = sanitize_name(&draft.name)
            .ok_or_else(|| LibError::InvalidName(draft.name.clone()))?;
        let category = sanitize_name(&draft.category)
            .ok_or_else(|| LibError::InvalidName(draft.category.clone()))?;
        let final_dir = planned.dir;

        fs::create_dir_all(&self.root)?;
        let tmp = self.root.join(format!(".tmp-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp)?;
        let asset = match self.save_into(draft, &tmp, &name, &category) {
            Ok(asset) => asset,
            Err(e) => {
                let _ = fs::remove_dir_all(&tmp);
                return Err(e);
            }
        };
        if let Some(parent) = final_dir.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Err(e) = fs::rename(&tmp, &final_dir) {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e.into());
        }
        Ok(AssetEntry {
            dir: final_dir,
            asset,
        })
    }

    fn save_into(
        &self,
        draft: &AssetDraft,
        tmp: &Path,
        name: &str,
        category: &str,
    ) -> Result<Asset, LibError> {
        let final_names = unique_names(
            &draft
                .materials
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<_>>(),
        );
        let mut model_info = None;
        let mut hdri_info = None;
        let mut materials: Vec<Material> = Vec::new();
        let mut thumbnail: Option<String> = None;

        match draft.asset_type {
            AssetType::Model => {
                let src = draft
                    .model_src
                    .as_ref()
                    .ok_or_else(|| LibError::MissingFile(PathBuf::new()))?;
                let parsed = parse_model(src)?;
                let src_ext = src
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                let model_file = format!("{name}.{src_ext}");
                let renames: HashMap<String, String> = draft
                    .materials
                    .iter()
                    .zip(&final_names)
                    .map(|(d, f)| (d.original_name.clone(), f.clone()))
                    .collect();
                if parsed.format == "fbx" {
                    // FBX is copied byte-for-byte; plugins apply the renames.
                    fs::copy(src, tmp.join(&model_file))?;
                } else {
                    let obj_text = fs::read_to_string(src)?;
                    let mtl_name = format!("{name}.mtl");
                    fs::write(tmp.join(&model_file), rewrite_obj(&obj_text, &mtl_name, &renames))?;
                    let mtl_out = mtllib_of(&obj_text)
                        .and_then(|orig| src.parent().map(|p| p.join(orig)))
                        .and_then(|p| fs::read_to_string(p).ok())
                        .map(|text| rewrite_mtl(&text, &renames))
                        .unwrap_or_else(|| {
                            final_names.iter().map(|n| format!("newmtl {n}\n")).collect()
                        });
                    fs::write(tmp.join(&mtl_name), mtl_out)?;
                }
                model_info = Some(ModelInfo {
                    file: model_file,
                    format: parsed.format,
                    meshes: parsed.meshes,
                    vertices: parsed.vertices,
                });
                for (draft_mat, final_name) in draft.materials.iter().zip(&final_names) {
                    materials.push(Material {
                        original_name: draft_mat.original_name.clone(),
                        name: final_name.clone(),
                        attributes: copy_attributes(draft_mat, tmp, name, Some(final_name))?,
                    });
                }
            }
            AssetType::Material => {
                let final_name = final_names.first().cloned().unwrap_or_else(|| name.to_string());
                let empty = MaterialDraft {
                    original_name: String::new(),
                    name: final_name.clone(),
                    attributes: vec![],
                };
                let draft_mat = draft.materials.first().unwrap_or(&empty);
                materials.push(Material {
                    original_name: final_name.clone(),
                    name: final_name,
                    attributes: copy_attributes(draft_mat, tmp, name, None)?,
                });
            }
            AssetType::Hdri => {
                let src = draft
                    .hdri_src
                    .as_ref()
                    .ok_or_else(|| LibError::MissingFile(PathBuf::new()))?;
                fs::copy(src, tmp.join(format!("{name}.hdr")))?;
                let (width, height) = hdr_size(src)?;
                hdri_info = Some(HdriInfo {
                    file: format!("{name}.hdr"),
                    width,
                    height,
                    rotation: draft.rotation,
                    intensity: draft.intensity,
                });
            }
        }

        match draft.asset_type {
            AssetType::Hdri => {
                match &draft.thumbnail_src {
                    Some(t) => write_thumbnail(t, &tmp.join("thumbnail.png"))?,
                    None => write_hdr_thumbnail(
                        draft
                            .hdri_src
                            .as_ref()
                            .ok_or_else(|| LibError::MissingFile(PathBuf::new()))?,
                        &tmp.join("thumbnail.png"),
                        draft.rotation,
                        draft.intensity,
                    )?,
                }
                thumbnail = Some("thumbnail.png".to_string());
            }
            _ => {
                if let Some(t) = &draft.thumbnail_src {
                    write_thumbnail(t, &tmp.join("thumbnail.png"))?;
                    thumbnail = Some("thumbnail.png".to_string());
                }
            }
        }

        let asset = Asset {
            schema: 1,
            id: Uuid::new_v4().to_string(),
            r#type: draft.asset_type,
            name: name.to_string(),
            category: category.to_string(),
            tags: draft.tags.clone(),
            created: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            thumbnail,
            model: model_info,
            materials,
            hdri: hdri_info,
            view: draft.view,
        };
        let json = serde_json::to_string_pretty(&asset)
            .map_err(|e| LibError::Parse(e.to_string()))?;
        fs::write(tmp.join("asset.json"), json)?;
        Ok(asset)
    }

    // shortcut: full rescan on start; add an index cache if libraries pass ~10k assets
    pub fn scan(&self) -> Vec<AssetEntry> {
        let mut out = Vec::new();
        for t in [AssetType::Model, AssetType::Material, AssetType::Hdri] {
            let type_dir = self.root.join(t.folder());
            let Ok(cats) = fs::read_dir(&type_dir) else {
                continue;
            };
            for cat in cats.flatten() {
                let cat_path = cat.path();
                if !cat_path.is_dir() {
                    continue;
                }
                let Ok(entries) = fs::read_dir(&cat_path) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let dir = entry.path();
                    let json_path = dir.join("asset.json");
                    let Ok(text) = fs::read_to_string(&json_path) else {
                        continue;
                    };
                    match serde_json::from_str::<Asset>(&text) {
                        Ok(asset) => out.push(AssetEntry { dir, asset }),
                        Err(e) => eprintln!("skipping {}: {e}", json_path.display()),
                    }
                }
            }
        }
        out.sort_by(|a, b| a.asset.name.cmp(&b.asset.name).then_with(|| a.dir.cmp(&b.dir)));
        out
    }

    pub fn delete(&self, dir: &Path) -> Result<(), LibError> {
        let root_c = self.root.canonicalize().unwrap_or_else(|_| self.root.clone());
        let dir_c = dir
            .canonicalize()
            .map_err(|_| LibError::OutsideLibrary(dir.to_path_buf()))?;
        if dir_c == root_c || !dir_c.starts_with(&root_c) {
            return Err(LibError::OutsideLibrary(dir.to_path_buf()));
        }
        // Only real asset folders may be deleted: <root>/<type>/<category>/<name>
        // with an asset.json inside. Anything else is refused, never trashed.
        let rel = dir_c
            .strip_prefix(&root_c)
            .map_err(|_| LibError::OutsideLibrary(dir.to_path_buf()))?;
        let parts: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let is_asset_dir = parts.len() == 3
            && matches!(parts[0].as_str(), "3D" | "Materials" | "HDRI")
            && dir_c.join("asset.json").is_file();
        if !is_asset_dir {
            return Err(LibError::OutsideLibrary(dir.to_path_buf()));
        }
        trash::delete(&dir_c)
            .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::model::{AssetDraft, AssetType, AttrKind, AttrValue, AttributeDraft, MaterialDraft};

    fn fx(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn root() -> (tempfile::TempDir, Library) {
        let d = tempfile::tempdir().unwrap();
        let lib = Library {
            root: d.path().to_path_buf(),
        };
        (d, lib)
    }

    fn attr(kind: AttrKind, value: Option<AttrValue>, tex: Option<PathBuf>) -> AttributeDraft {
        AttributeDraft {
            kind,
            value,
            texture_src: tex,
            strength: None,
            normal_y: None,
        }
    }

    fn fbx_draft() -> AssetDraft {
        AssetDraft {
            asset_type: AssetType::Model,
            name: "Old Chair".into(),
            category: "Furni ture".into(),
            tags: vec![],
            model_src: Some(fx("cube_2mat.fbx")),
            hdri_src: None,
            thumbnail_src: Some(fx("big.png")),
            materials: vec![
                MaterialDraft {
                    original_name: "Red".into(),
                    name: "Seat".into(),
                    attributes: vec![attr(AttrKind::BaseColor, None, Some(fx("red.png")))],
                },
                MaterialDraft {
                    original_name: "Blue".into(),
                    name: "Legs".into(),
                    attributes: vec![attr(AttrKind::Roughness, Some(AttrValue::Scalar(0.4)), None)],
                },
            ],
            rotation: 0.0,
            intensity: 1.0,
            view: None,
        }
    }

    fn hdri_draft() -> AssetDraft {
        AssetDraft {
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
        }
    }

    #[test]
    fn library_plan_exact_folder() {
        // plan() with valid input names exactly the folder that
        // validate (and therefore save) will use.
        let (_d, lib) = root();
        let draft = fbx_draft();
        let planned = lib.plan(draft.asset_type, &draft.category, &draft.name);
        let validated = lib.validate(&draft).unwrap();
        assert_eq!(planned.dir, validated.dir);
        assert_eq!(planned.dir_name, "Old_Chair");
        assert!(
            planned.rel_dir.ends_with("3D/Furni_ture/Old_Chair/"),
            "rel_dir: {}",
            planned.rel_dir
        );
    }

    #[test]
    fn library_plan_placeholders() {
        // plan() never fails: empty fields become placeholders.
        let d = tempfile::tempdir().unwrap();
        let lib = Library {
            root: d.path().join("Database"),
        };
        let planned = lib.plan(AssetType::Model, "", "");
        assert_eq!(planned.rel_dir, "Database/3D/<Category>/<Name>/");
        assert_eq!(planned.dir_name, "<Name>");
    }

    #[test]
    fn save_model_fbx_layout() {
        let (d, lib) = root();
        let entry = lib.save(&fbx_draft()).unwrap();
        let dir = d.path().join("3D").join("Furni_ture").join("Old_Chair");
        assert_eq!(entry.dir, dir);
        assert_eq!(
            std::fs::read(dir.join("Old_Chair.fbx")).unwrap(),
            std::fs::read(fx("cube_2mat.fbx")).unwrap()
        );
        assert!(dir.join("Old_Chair_Seat_BaseColor.png").is_file());
        assert_eq!(
            image::image_dimensions(dir.join("thumbnail.png")).unwrap(),
            (512, 512)
        );
        assert!(dir.join("asset.json").is_file());
        assert_eq!(entry.asset.materials[0].original_name, "Red");
        assert_eq!(entry.asset.materials[0].name, "Seat");
        assert_eq!(entry.asset.model.as_ref().unwrap().format, "fbx");
        assert_eq!(entry.asset.name, "Old_Chair");
        assert_eq!(entry.asset.category, "Furni_ture");
    }

    #[test]
    fn save_model_obj_rewrites() {
        let (_d, lib) = root();
        let mut draft = fbx_draft();
        draft.model_src = Some(fx("cube_2mat.obj"));
        draft.thumbnail_src = None;
        let entry = lib.save(&draft).unwrap();
        let obj = std::fs::read_to_string(entry.dir.join("Old_Chair.obj")).unwrap();
        assert!(obj.contains("mtllib Old_Chair.mtl"), "{obj}");
        assert!(obj.contains("usemtl Seat"), "{obj}");
        let mtl = std::fs::read_to_string(entry.dir.join("Old_Chair.mtl")).unwrap();
        assert!(mtl.contains("newmtl Seat"), "{mtl}");
    }

    #[test]
    fn save_model_dedupes_materials() {
        let (_d, lib) = root();
        let mut draft = fbx_draft();
        draft.materials[0].name = "Wood".into();
        draft.materials[1].name = "Wood".into();
        let entry = lib.save(&draft).unwrap();
        let names: Vec<_> = entry.asset.materials.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Wood", "Wood_2"]);
    }

    #[test]
    fn save_material_layout() {
        let (d, lib) = root();
        let draft = AssetDraft {
            asset_type: AssetType::Material,
            name: "Oak".into(),
            category: "Wood".into(),
            tags: vec![],
            model_src: None,
            hdri_src: None,
            thumbnail_src: None,
            materials: vec![MaterialDraft {
                original_name: "".into(),
                name: "Oak".into(),
                attributes: vec![attr(AttrKind::Roughness, None, Some(fx("gray.tga")))],
            }],
            rotation: 0.0,
            intensity: 1.0,
            view: None,
        };
        let entry = lib.save(&draft).unwrap();
        assert_eq!(entry.dir, d.path().join("Materials").join("Wood").join("Oak"));
        assert!(entry.dir.join("Oak_Roughness.tga").is_file());
        assert_eq!(entry.asset.materials.len(), 1);
    }

    #[test]
    fn save_hdri_layout() {
        let (_d, lib) = root();
        let mut draft = hdri_draft();
        draft.rotation = 45.0;
        draft.intensity = 2.0;
        let entry = lib.save(&draft).unwrap();
        assert!(entry.dir.join("Sky.hdr").is_file());
        // HDRI thumbnails are 1024x512 since Amendment A1 §A1.4.
        assert_eq!(
            image::image_dimensions(entry.dir.join("thumbnail.png")).unwrap(),
            (1024, 512)
        );
        let hdri = entry.asset.hdri.as_ref().unwrap();
        assert_eq!((hdri.width, hdri.height), (512, 256));
        assert_eq!(hdri.rotation, 45.0);
        assert_eq!(hdri.intensity, 2.0);
    }

    #[test]
    fn save_rejects_existing_name() {
        let (_d, lib) = root();
        let entry = lib.save(&fbx_draft()).unwrap();
        let before = std::fs::read(entry.dir.join("asset.json")).unwrap();
        assert!(matches!(lib.save(&fbx_draft()), Err(LibError::NameExists(_))));
        assert_eq!(std::fs::read(entry.dir.join("asset.json")).unwrap(), before);
    }

    #[test]
    fn save_is_atomic_on_missing_texture() {
        let (d, lib) = root();
        let mut draft = fbx_draft();
        draft.materials[0].attributes[0].texture_src = Some(d.path().join("nope.png"));
        assert!(matches!(lib.save(&draft), Err(LibError::MissingFile(_))));
        assert!(!d.path().join("3D").join("Furni_ture").join("Old_Chair").exists());
        let leftovers: Vec<_> = std::fs::read_dir(d.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp-"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn save_accepts_unicode_source_paths() {
        let (_d, lib) = root();
        let src_dir = tempfile::tempdir().unwrap();
        let tex_dir = src_dir.path().join("Téxtures");
        std::fs::create_dir_all(&tex_dir).unwrap();
        let tex = tex_dir.join("Wood Base.PNG");
        std::fs::copy(fx("red.png"), &tex).unwrap();
        let mut draft = fbx_draft();
        draft.materials[0].attributes[0].texture_src = Some(tex);
        let entry = lib.save(&draft).unwrap();
        assert!(entry.dir.join("Old_Chair_Seat_BaseColor.png").is_file());
    }

    #[test]
    fn save_rejects_exr_hdri() {
        let (_d, lib) = root();
        let mut draft = hdri_draft();
        draft.hdri_src = Some(PathBuf::from("x.exr"));
        assert!(matches!(lib.save(&draft), Err(LibError::UnsupportedFormat(_))));
    }

    #[test]
    fn scan_finds_saved() {
        let (d, lib) = root();
        lib.save(&fbx_draft()).unwrap();
        lib.save(&hdri_draft()).unwrap();
        let broken = d.path().join("3D").join("Junk").join("Broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("asset.json"), "{").unwrap();
        assert_eq!(lib.scan().len(), 2);
    }

    #[test]
    fn delete_refuses_type_folder() {
        let (d, lib) = root();
        let entry = lib.save(&fbx_draft()).unwrap();
        let e = lib.delete(&d.path().join("3D")).unwrap_err();
        assert!(matches!(e, LibError::OutsideLibrary(_)), "{e:?}");
        assert!(entry.dir.join("asset.json").exists());
    }

    #[test]
    fn delete_refuses_category_folder() {
        let (d, lib) = root();
        let entry = lib.save(&fbx_draft()).unwrap();
        let e = lib.delete(&d.path().join("3D").join("Furni_ture")).unwrap_err();
        assert!(matches!(e, LibError::OutsideLibrary(_)), "{e:?}");
        assert!(entry.dir.join("asset.json").exists());
    }

    #[test]
    fn delete_refuses_folder_without_asset_json() {
        let (d, lib) = root();
        let dir = d.path().join("3D").join("Props").join("NotAnAsset");
        std::fs::create_dir_all(&dir).unwrap();
        let e = lib.delete(&dir).unwrap_err();
        assert!(matches!(e, LibError::OutsideLibrary(_)), "{e:?}");
        assert!(dir.exists());
    }

    #[test]
    fn delete_moves_asset_to_trash() {
        let (_d, lib) = root();
        let entry = lib.save(&fbx_draft()).unwrap();
        lib.delete(&entry.dir).unwrap();
        assert!(!entry.dir.exists());
    }

    #[test]
    fn delete_outside_root_refused() {
        let (_d, lib) = root();
        assert!(matches!(
            lib.delete(std::path::Path::new("/")),
            Err(LibError::OutsideLibrary(_))
        ));
    }
}
