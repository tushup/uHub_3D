//! The `uhub` CLI for agents (spec §A1.7). Same code path as the
//! app (library + pipeline), so results match the GUI.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use crate::error::LibError;
use crate::images::hdr_size;
use crate::library::{AssetEntry, Library};
use crate::mesh::load_mesh;
use crate::model::{AssetDraft, AssetType};
use crate::parse::parse_model;
use crate::pipeline;
use crate::render::RenderOptions;
use crate::settings::{settings_path, Settings};
use crate::view::View;

pub const GUIDE: &str = include_str!("../../../docs/cli/AGENT_GUIDE.md");
pub const SCHEMA: &str = include_str!("manifest.schema.json");

// ------------------------------------------------------------- errors

#[derive(Debug)]
struct CliError {
    code: String,
    message: String,
    exit: i32,
    /// True when the error was already printed (batch items).
    printed: bool,
}

impl CliError {
    fn new(code: &str, message: impl Into<String>, exit: i32) -> CliError {
        CliError {
            code: code.into(),
            message: message.into(),
            exit,
            printed: false,
        }
    }

    fn usage(message: impl Into<String>) -> CliError {
        CliError::new("usage", message, 2)
    }

    fn not_found(reference: &str) -> CliError {
        CliError::new("not_found", format!("asset not found: {reference}"), 5)
    }

    fn invalid_input(message: impl Into<String>) -> CliError {
        CliError::new("invalid_input", message, 3)
    }
}

impl From<LibError> for CliError {
    fn from(e: LibError) -> CliError {
        let (code, exit) = match &e {
            LibError::InvalidName(_) => ("invalid_name", 3),
            LibError::NameExists(_) => ("name_exists", 4),
            LibError::UnsupportedFormat(_) => ("unsupported_format", 3),
            LibError::MissingFile(_) => ("missing_file", 3),
            LibError::Parse(_) => ("unsupported_format", 3),
            LibError::OutsideLibrary(_) => ("not_found", 5),
            LibError::Cancelled => ("render_failed", 6),
            LibError::Bridge(_) | LibError::Io(_) => ("io", 1),
        };
        CliError::new(code, e.to_string(), exit)
    }
}

// ------------------------------------------------------------- output

struct Out {
    json: bool,
}

impl Out {
    fn ok(&self, payload: Value, human: String) {
        if self.json {
            println!("{}", serde_json::to_string(&payload).unwrap_or_default());
        } else {
            println!("{human}");
        }
    }

    fn err(&self, e: &CliError) {
        if self.json {
            println!(
                "{}",
                json!({ "ok": false, "error": { "code": e.code, "message": e.message } })
            );
        } else {
            eprintln!("error: {}", e.message);
        }
    }
}

// ---------------------------------------------------------------- CLI

#[derive(Parser)]
#[command(name = "uhub", version, about = "uHub 3D library CLI for agents")]
struct Cli {
    /// Library root. Default: UHUB_LIBRARY, then the app settings,
    /// then ~/Documents/uHub3D/Database.
    #[arg(long, global = true)]
    library: Option<PathBuf>,
    /// Machine-readable output: stdout carries only JSON.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the agent guide (markdown).
    Guide,
    /// Print the manifest JSON Schema.
    Schema,
    /// Library root, version and asset counts per type.
    Info,
    /// List categories, optionally for one type.
    Categories {
        #[arg(long = "type", value_parser = ["model", "material", "hdri"])]
        ty: Option<String>,
    },
    /// List assets with optional filters.
    List {
        #[arg(long = "type", value_parser = ["model", "material", "hdri"])]
        ty: Option<String>,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        search: Option<String>,
    },
    /// Show one asset: its asset.json and folder.
    Show { asset_ref: String },
    /// Inspect a model or HDRI file without adding it.
    Inspect { file: PathBuf },
    /// Add one asset (or a batch) from a manifest file or stdin (-).
    Add {
        manifest: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        seconds: Option<f32>,
        #[arg(long)]
        samples: Option<u32>,
        /// Skip OIDN denoising of the rendered thumbnail.
        #[arg(long = "no-denoise")]
        no_denoise: bool,
    },
    /// Re-render an asset's thumbnail in place.
    Render {
        asset_ref: String,
        #[arg(long)]
        yaw: Option<f32>,
        #[arg(long)]
        pitch: Option<f32>,
        #[arg(long)]
        zoom: Option<f32>,
        #[arg(long)]
        seconds: Option<f32>,
        #[arg(long)]
        samples: Option<u32>,
        /// Skip OIDN denoising of the rendered thumbnail.
        #[arg(long = "no-denoise")]
        no_denoise: bool,
    },
    /// Move an asset to the trash. Requires --yes.
    Delete {
        asset_ref: String,
        #[arg(long)]
        yes: bool,
    },
}

fn type_filter(ty: &Option<String>) -> Option<AssetType> {
    match ty.as_deref() {
        Some("model") => Some(AssetType::Model),
        Some("material") => Some(AssetType::Material),
        Some("hdri") => Some(AssetType::Hdri),
        _ => None,
    }
}

fn type_name(t: AssetType) -> &'static str {
    match t {
        AssetType::Model => "model",
        AssetType::Material => "material",
        AssetType::Hdri => "hdri",
    }
}

fn library_root(cli: &Cli) -> PathBuf {
    if let Some(dir) = &cli.library {
        return dir.clone();
    }
    if let Ok(env) = std::env::var("UHUB_LIBRARY") {
        if !env.is_empty() {
            return PathBuf::from(env);
        }
    }
    Settings::load(&settings_path()).library_root
}

/// A ref is an asset id, or `<3D|Materials|HDRI>/<Category>/<Name>`.
/// Category and name may be given in display form ("Demo Oak"):
/// they are sanitized before comparison, like stored names.
fn resolve(lib: &Library, reference: &str) -> Result<AssetEntry, CliError> {
    let entries = lib.scan();
    if let Some(e) = entries.iter().find(|e| e.asset.id == reference) {
        return Ok(e.clone());
    }
    let parts: Vec<&str> = reference.split(['/', '\\']).collect();
    if parts.len() == 3 {
        if let Some(e) = entries.iter().find(|e| {
            e.asset.r#type.folder() == parts[0]
                && e.asset.category
                    == crate::naming::sanitize_name(parts[1])
                        .unwrap_or_else(|| parts[1].to_string())
                && e.asset.name
                    == crate::naming::sanitize_name(parts[2])
                        .unwrap_or_else(|| parts[2].to_string())
        }) {
            return Ok(e.clone());
        }
    }
    Err(CliError::not_found(reference))
}

pub fn run() -> i32 {
    let cli = Cli::parse();
    let out = Out { json: cli.json };
    match dispatch(&cli, &out) {
        Ok(()) => 0,
        Err(e) => {
            if !e.printed {
                out.err(&e);
            }
            e.exit
        }
    }
}

fn dispatch(cli: &Cli, out: &Out) -> Result<(), CliError> {
    let lib = Library {
        root: library_root(cli),
    };
    match &cli.command {
        Command::Guide => {
            print!("{GUIDE}");
            Ok(())
        }
        Command::Schema => {
            print!("{SCHEMA}");
            Ok(())
        }
        Command::Info => {
            let entries = lib.scan();
            let count = |t: AssetType| entries.iter().filter(|e| e.asset.r#type == t).count();
            let (m, mat, h) = (
                count(AssetType::Model),
                count(AssetType::Material),
                count(AssetType::Hdri),
            );
            out.ok(
                json!({
                    "ok": true,
                    "library": lib.root.display().to_string(),
                    "version": env!("CARGO_PKG_VERSION"),
                    "counts": { "model": m, "material": mat, "hdri": h },
                }),
                format!(
                    "Library: {}\nVersion: {}\nAssets: {m} model, {mat} material, {h} hdri",
                    lib.root.display(),
                    env!("CARGO_PKG_VERSION")
                ),
            );
            Ok(())
        }
        Command::Categories { ty } => {
            let filter = type_filter(ty);
            let mut counts: Vec<(String, String, usize)> = Vec::new();
            for e in lib.scan() {
                if filter.is_some_and(|f| e.asset.r#type != f) {
                    continue;
                }
                let key = (
                    type_name(e.asset.r#type).to_string(),
                    e.asset.category.clone(),
                );
                if let Some(row) = counts
                    .iter_mut()
                    .find(|row| row.0 == key.0 && row.1 == key.1)
                {
                    row.2 += 1;
                } else {
                    counts.push((key.0, key.1, 1));
                }
            }
            counts.sort();
            let payload: Vec<Value> = counts
                .iter()
                .map(|(t, c, n)| json!({ "type": t, "category": c, "assets": n }))
                .collect();
            let human = counts
                .iter()
                .map(|(t, c, n)| format!("{t}/{c}: {n}"))
                .collect::<Vec<_>>()
                .join("\n");
            out.ok(json!({ "ok": true, "categories": payload }), human);
            Ok(())
        }
        Command::List {
            ty,
            category,
            search,
        } => {
            let filter = type_filter(ty);
            let search = search.as_ref().map(|s| s.to_lowercase());
            let scanned = lib.scan();
            let assets: Vec<&AssetEntry> = scanned
                .iter()
                .filter(|e| filter.is_none_or(|f| e.asset.r#type == f))
                .filter(|e| category.as_ref().is_none_or(|c| e.asset.category == *c))
                .filter(|e| {
                    search
                        .as_ref()
                        .is_none_or(|s| e.asset.name.to_lowercase().contains(s))
                })
                .collect();
            let payload: Vec<Value> = assets
                .iter()
                .map(|e| {
                    json!({
                        "id": e.asset.id,
                        "type": type_name(e.asset.r#type),
                        "name": e.asset.name,
                        "category": e.asset.category,
                        "dir": e.dir.display().to_string(),
                    })
                })
                .collect();
            let human = assets
                .iter()
                .map(|e| {
                    format!(
                        "{}/{}/{}  {}",
                        e.asset.r#type.folder(),
                        e.asset.category,
                        e.asset.name,
                        e.asset.id
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            out.ok(json!({ "ok": true, "assets": payload }), human);
            Ok(())
        }
        Command::Show { asset_ref } => {
            let entry = resolve(&lib, asset_ref)?;
            out.ok(
                json!({
                    "ok": true,
                    "dir": entry.dir.display().to_string(),
                    "asset": entry.asset,
                }),
                format!(
                    "Folder: {}\n{}",
                    entry.dir.display(),
                    serde_json::to_string_pretty(&entry.asset).unwrap_or_default()
                ),
            );
            Ok(())
        }
        Command::Inspect { file } => inspect(file, out),
        Command::Add {
            manifest,
            dry_run,
            seconds,
            samples,
            no_denoise,
        } => add(
            &lib,
            out,
            manifest,
            AddOptions {
                dry_run: *dry_run,
                seconds: *seconds,
                samples: *samples,
                no_denoise: *no_denoise,
            },
        ),
        Command::Render {
            asset_ref,
            yaw,
            pitch,
            zoom,
            seconds,
            samples,
            no_denoise,
        } => {
            let entry = resolve(&lib, asset_ref)?;
            let view = if yaw.is_some() || pitch.is_some() || zoom.is_some() {
                let base = entry.asset.view.unwrap_or_default();
                Some(View {
                    yaw: yaw.unwrap_or(base.yaw),
                    pitch: pitch.unwrap_or(base.pitch),
                    zoom: zoom.unwrap_or(base.zoom),
                })
            } else {
                None
            };
            let opts = RenderOptions {
                size: 512,
                seconds: seconds.unwrap_or(10.0),
                max_samples: *samples,
                seed: 0,
                denoise: !no_denoise,
            };
            let cancel = AtomicBool::new(false);
            let mut last = Instant::now() - std::time::Duration::from_secs(1);
            let updated = pipeline::rerender(&lib, &entry, view, &opts, &cancel, |p, _| {
                if last.elapsed() >= std::time::Duration::from_secs(1) {
                    last = Instant::now();
                    eprintln!(
                        "rendering {:.1}/{:.1} s - {} spp",
                        p.elapsed, opts.seconds, p.samples
                    );
                }
            })
            .map_err(|e| CliError::new("render_failed", e.to_string(), 6))?;
            out.ok(
                json!({
                    "ok": true,
                    "dir": updated.dir.display().to_string(),
                    "view": updated.asset.view,
                }),
                format!(
                    "Rendered {} ({})",
                    updated.asset.name,
                    updated.dir.display()
                ),
            );
            Ok(())
        }
        Command::Delete { asset_ref, yes } => {
            if !yes {
                return Err(CliError::usage("delete requires --yes"));
            }
            let entry = resolve(&lib, asset_ref)?;
            lib.delete(&entry.dir)?;
            out.ok(
                json!({ "ok": true, "deleted": entry.dir.display().to_string() }),
                format!("Deleted {}", entry.asset.name),
            );
            Ok(())
        }
    }
}

fn inspect(file: &Path, out: &Out) -> Result<(), CliError> {
    if !file.exists() {
        return Err(LibError::MissingFile(file.to_path_buf()).into());
    }
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "fbx" | "obj" => {
            let parsed = parse_model(file)?;
            let mesh = load_mesh(file)?;
            let bounds = mesh.bounds();
            let materials: Vec<Value> = parsed
                .materials
                .iter()
                .map(|m| {
                    json!({
                        "name": m.name,
                        "textures": m.textures.iter().map(|(kind, path)| {
                            json!({
                                "kind": serde_json::to_value(kind).unwrap_or_default(),
                                "path": path.display().to_string(),
                            })
                        }).collect::<Vec<_>>(),
                    })
                })
                .collect();
            out.ok(
                json!({
                    "ok": true,
                    "format": parsed.format,
                    "meshes": parsed.meshes,
                    "vertices": parsed.vertices,
                    "bounds": { "min": bounds.min, "max": bounds.max },
                    "materials": materials,
                }),
                format!(
                    "{}: {} meshes, {} vertices, bounds {:?} .. {:?}, {} materials",
                    parsed.format,
                    parsed.meshes,
                    parsed.vertices,
                    bounds.min,
                    bounds.max,
                    parsed.materials.len()
                ),
            );
            Ok(())
        }
        "hdr" => {
            let (width, height) = hdr_size(file)?;
            out.ok(
                json!({ "ok": true, "format": "hdr", "width": width, "height": height }),
                format!("hdr: {width}x{height}"),
            );
            Ok(())
        }
        _ => Err(LibError::UnsupportedFormat(file.display().to_string()).into()),
    }
}

/// Resolve relative manifest paths against the manifest folder
/// (or the current directory for stdin).
fn absolutize(draft: &mut AssetDraft, base: &Path) {
    fn fix(slot: &mut Option<PathBuf>, base: &Path) {
        if let Some(p) = slot {
            if p.is_relative() {
                *p = base.join(&p);
            }
        }
    }
    fix(&mut draft.model_src, base);
    fix(&mut draft.hdri_src, base);
    fix(&mut draft.thumbnail_src, base);
    for m in &mut draft.materials {
        for a in &mut m.attributes {
            fix(&mut a.texture_src, base);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AddOptions {
    dry_run: bool,
    seconds: Option<f32>,
    samples: Option<u32>,
    no_denoise: bool,
}

fn add(lib: &Library, out: &Out, manifest: &str, options: AddOptions) -> Result<(), CliError> {
    let (text, base) = if manifest == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| CliError::new("io", e.to_string(), 1))?;
        (
            buf,
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        )
    } else {
        let path = PathBuf::from(manifest);
        let text = std::fs::read_to_string(&path)
            .map_err(|_| CliError::from(LibError::MissingFile(path.clone())))?;
        let base = path
            .parent()
            .map(|p| p.to_path_buf())
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| PathBuf::from("."));
        (text, base)
    };
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| CliError::invalid_input(format!("manifest is not valid JSON: {e}")))?;
    let items: Vec<Value> = match value {
        Value::Array(items) => items,
        Value::Object(_) => vec![value],
        _ => {
            return Err(CliError::invalid_input(
                "manifest must be an asset object or an array of them",
            ))
        }
    };
    let batch = items.len() > 1;
    let mut worst_exit = 0;
    for item in items {
        let result = add_one(lib, out, item, &base, options);
        match result {
            Ok(()) => {}
            Err(e) => {
                out.err(&e);
                worst_exit = worst_exit.max(e.exit);
                if !batch {
                    return Err(CliError { printed: true, ..e });
                }
            }
        }
    }
    if worst_exit > 0 {
        return Err(CliError {
            code: "batch".into(),
            message: "one or more items failed".into(),
            exit: worst_exit,
            printed: true,
        });
    }
    Ok(())
}

fn add_one(
    lib: &Library,
    out: &Out,
    item: Value,
    base: &Path,
    options: AddOptions,
) -> Result<(), CliError> {
    let mut draft: AssetDraft = serde_json::from_value(item).map_err(|e| {
        CliError::invalid_input(format!("manifest item does not match the schema: {e}"))
    })?;
    absolutize(&mut draft, base);
    if options.dry_run {
        let planned = lib.validate(&draft)?;
        out.ok(
            json!({
                "ok": true,
                "dryRun": true,
                "dir": planned.dir.display().to_string(),
                "files": planned.files,
            }),
            format!(
                "Would create {}:\n{}",
                planned.dir.display(),
                planned
                    .files
                    .iter()
                    .map(|f| format!("  {f}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        );
        return Ok(());
    }
    let opts = RenderOptions {
        size: 512,
        seconds: options.seconds.unwrap_or(10.0),
        max_samples: options.samples,
        seed: 0,
        denoise: !options.no_denoise,
    };
    let cancel = AtomicBool::new(false);
    let mut last = Instant::now() - std::time::Duration::from_secs(1);
    let entry = pipeline::add_asset(lib, &draft, &opts, &cancel, |p, _| {
        if last.elapsed() >= std::time::Duration::from_secs(1) {
            last = Instant::now();
            eprintln!(
                "rendering {:.1}/{:.1} s - {} spp",
                p.elapsed, opts.seconds, p.samples
            );
        }
    })?;
    out.ok(
        json!({
            "ok": true,
            "id": entry.asset.id,
            "type": type_name(entry.asset.r#type),
            "name": entry.asset.name,
            "dir": entry.dir.display().to_string(),
        }),
        format!(
            "Added {} {} -> {}",
            type_name(entry.asset.r#type),
            entry.asset.name,
            entry.dir.display()
        ),
    );
    Ok(())
}
