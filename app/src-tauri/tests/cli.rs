//! End-to-end tests for the `uhub` CLI binary (spec §A1.7).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};
use uhub3d_lib::library::Library;

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run(library: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uhub"))
        .arg("--json")
        .arg("--library")
        .arg(library)
        .args(args)
        .output()
        .unwrap()
}

fn run_in(library: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uhub"))
        .arg("--json")
        .arg("--library")
        .arg(library)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn first_json(out: &Output) -> Value {
    let text = stdout(out);
    serde_json::from_str(text.lines().next().unwrap_or("")).expect("stdout is JSON")
}

fn png_size(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n", "PNG magic");
    let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (w, h)
}

fn write_manifest(dir: &Path, name: &str, value: &Value) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    path
}

fn material_manifest() -> Value {
    json!({
        "assetType": "material",
        "name": "Oak",
        "category": "Wood",
        "tags": [],
        "materials": [{
            "originalName": "Oak",
            "name": "Oak",
            "attributes": [{ "kind": "BaseColor", "value": [0.4, 0.25, 0.1] }],
        }],
        "rotation": 0,
        "intensity": 1,
    })
}

fn add_material(library: &Path, work: &Path) -> Value {
    let manifest = write_manifest(work, "oak.json", &material_manifest());
    let out = run(
        library,
        &["add", manifest.to_str().unwrap(), "--samples", "1"],
    );
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    first_json(&out)
}

#[test]
fn cli_info_reports_root_and_counts() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let out = run(&library, &["info"]);
    assert_eq!(out.status.code(), Some(0));
    let v = first_json(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["library"], library.display().to_string());
    assert_eq!(v["counts"]["model"], 0);
    add_material(&library, work.path());
    let v = first_json(&run(&library, &["info"]));
    assert_eq!(v["counts"]["material"], 1);
    assert_eq!(v["counts"]["model"], 0);
}

#[test]
fn cli_add_material_renders_thumbnail() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let v = add_material(&library, work.path());
    assert_eq!(v["ok"], true);
    assert_eq!(v["type"], "material");
    let dir = PathBuf::from(v["dir"].as_str().unwrap());
    assert_eq!(png_size(&dir.join("thumbnail.png")), (512, 512));
    let lib = Library { root: library };
    let entries = lib.scan();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].asset.view, Some(uhub3d_lib::view::View::default()));
}

#[test]
fn cli_add_model_from_fbx_fixture() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let manifest = write_manifest(
        work.path(),
        "cube.json",
        &json!({
            "assetType": "model",
            "name": "Cube",
            "category": "Furniture",
            "tags": [],
            "modelSrc": fx("cube_2mat.fbx").display().to_string(),
            "materials": [],
            "rotation": 0,
            "intensity": 1,
        }),
    );
    let out = run(&library, &["add", manifest.to_str().unwrap(), "--samples", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let v = first_json(&out);
    let dir = PathBuf::from(v["dir"].as_str().unwrap());
    assert!(dir.join("Cube.fbx").is_file());
    assert_eq!(png_size(&dir.join("thumbnail.png")), (512, 512));
}

#[test]
fn cli_add_hdri() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let manifest = write_manifest(
        work.path(),
        "sky.json",
        &json!({
            "assetType": "hdri",
            "name": "Sky",
            "category": "Skies",
            "tags": [],
            "hdriSrc": fx("sky.hdr").display().to_string(),
            "materials": [],
            "rotation": 30,
            "intensity": 1.5,
        }),
    );
    let out = run(&library, &["add", manifest.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let v = first_json(&out);
    let dir = PathBuf::from(v["dir"].as_str().unwrap());
    assert!(dir.join("Sky.hdr").is_file());
    assert_eq!(png_size(&dir.join("thumbnail.png")), (1024, 512));
}

#[test]
fn cli_dry_run_writes_nothing() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let manifest = write_manifest(
        work.path(),
        "cube.json",
        &json!({
            "assetType": "model",
            "name": "Cube",
            "category": "Furniture",
            "tags": [],
            "modelSrc": fx("cube_2mat.fbx").display().to_string(),
            "materials": [],
            "rotation": 0,
            "intensity": 1,
        }),
    );
    let out = run(&library, &["add", manifest.to_str().unwrap(), "--dry-run"]);
    assert_eq!(out.status.code(), Some(0));
    let v = first_json(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["files"], json!(["Cube.fbx", "thumbnail.png", "asset.json"]));
    assert!(v["dir"].as_str().unwrap().ends_with("Cube"));
    assert!(!library.exists(), "dry-run must not create the library");
}

#[test]
fn cli_name_exists_exit_4() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    add_material(&library, work.path());
    let manifest = write_manifest(work.path(), "oak2.json", &material_manifest());
    let out = run(&library, &["add", manifest.to_str().unwrap(), "--samples", "1"]);
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(first_json(&out)["error"]["code"], "name_exists");
}

#[test]
fn cli_batch_continues_on_error() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let batch = json!([
        material_manifest(),
        {
            "assetType": "model",
            "name": "Ghost",
            "category": "Furniture",
            "tags": [],
            "modelSrc": work.path().join("missing.fbx").display().to_string(),
            "materials": [],
            "rotation": 0,
            "intensity": 1,
        },
    ]);
    let manifest = write_manifest(work.path(), "batch.json", &batch);
    let out = run(&library, &["add", manifest.to_str().unwrap(), "--samples", "1"]);
    assert_eq!(out.status.code(), Some(3));
    let text = stdout(&out);
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert_eq!(lines[0]["ok"], true);
    assert_eq!(lines[1]["ok"], false);
    assert_eq!(lines[1]["error"]["code"], "missing_file");
    let lib = Library { root: library };
    assert_eq!(lib.scan().len(), 1, "the good item was added");
}

#[test]
fn cli_relative_paths_resolve_from_manifest_dir() {
    let work = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    fs::copy(fx("cube_2mat.fbx"), work.path().join("chair.fbx")).unwrap();
    fs::copy(fx("red.png"), work.path().join("tex.png")).unwrap();
    let manifest = write_manifest(
        work.path(),
        "chair.json",
        &json!({
            "assetType": "model",
            "name": "Chair",
            "category": "Furniture",
            "tags": [],
            "modelSrc": "chair.fbx",
            "materials": [{
                "originalName": "Red",
                "name": "Seat",
                "attributes": [{ "kind": "BaseColor", "textureSrc": "tex.png" }],
            }],
            "rotation": 0,
            "intensity": 1,
        }),
    );
    let out = run_in(
        &library,
        elsewhere.path(),
        &["add", manifest.to_str().unwrap(), "--samples", "1"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(first_json(&out)["ok"], true);
}

#[test]
fn cli_list_filters() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    add_material(&library, work.path());
    let sky = write_manifest(
        work.path(),
        "sky.json",
        &json!({
            "assetType": "hdri",
            "name": "Sky",
            "category": "Skies",
            "tags": [],
            "hdriSrc": fx("sky.hdr").display().to_string(),
            "materials": [],
            "rotation": 0,
            "intensity": 1,
        }),
    );
    assert_eq!(run(&library, &["add", sky.to_str().unwrap()]).status.code(), Some(0));

    let v = first_json(&run(&library, &["list"]));
    assert_eq!(v["assets"].as_array().unwrap().len(), 2);
    let v = first_json(&run(&library, &["list", "--type", "material"]));
    let assets = v["assets"].as_array().unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0]["name"], "Oak");
    let v = first_json(&run(&library, &["list", "--search", "sky"]));
    let assets = v["assets"].as_array().unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0]["name"], "Sky");
    let v = first_json(&run(&library, &["categories"]));
    let cats = v["categories"].as_array().unwrap();
    assert!(cats.iter().any(|c| c["category"] == "Wood" && c["assets"] == 1), "{cats:?}");
}

#[test]
fn cli_show_by_id_and_by_path() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let added = add_material(&library, work.path());
    let id = added["id"].as_str().unwrap().to_string();
    for reference in [id.as_str(), "Materials/Wood/Oak"] {
        let out = run(&library, &["show", reference]);
        assert_eq!(out.status.code(), Some(0), "ref {reference}");
        let v = first_json(&out);
        assert_eq!(v["asset"]["name"], "Oak");
        assert_eq!(v["asset"]["type"], "material");
    }
}

#[test]
fn cli_inspect_fbx_lists_materials_and_textures() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let out = run(&library, &["inspect", fx("cube_2mat.fbx").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let v = first_json(&out);
    assert_eq!(v["format"], "fbx");
    let materials = v["materials"].as_array().unwrap();
    assert!(materials.iter().any(|m| m["name"] == "Red"), "{materials:?}");
    assert!(
        materials.iter().any(|m| !m["textures"].as_array().unwrap().is_empty()),
        "found textures listed: {materials:?}"
    );
    assert!(v["bounds"]["min"].is_array());
}

#[test]
fn cli_render_updates_view() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    add_material(&library, work.path());
    let out = run(
        &library,
        &["render", "Materials/Wood/Oak", "--yaw", "45", "--samples", "1"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let lib = Library { root: library };
    let entries = lib.scan();
    assert_eq!(entries[0].asset.view.unwrap().yaw, 45.0);
}

#[test]
fn cli_delete_requires_yes() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let added = add_material(&library, work.path());
    let dir = PathBuf::from(added["dir"].as_str().unwrap());
    let out = run(&library, &["delete", "Materials/Wood/Oak"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(dir.join("asset.json").is_file(), "folder untouched without --yes");
    let out = run(&library, &["delete", "Materials/Wood/Oak", "--yes"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!dir.exists(), "folder moved to trash");
}

#[test]
fn cli_show_accepts_display_name_ref() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let mut manifest = material_manifest();
    manifest["name"] = json!("My Mat");
    let path = write_manifest(work.path(), "mymat.json", &manifest);
    let out = run(&library, &["add", path.to_str().unwrap(), "--samples", "1"]);
    assert_eq!(out.status.code(), Some(0));
    // Stored name is sanitized to My_Mat; the display form resolves.
    let out = run(&library, &["show", "Materials/Wood/My Mat"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_eq!(first_json(&out)["asset"]["name"], "My_Mat");
}

#[test]
fn cli_no_denoise_flag() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let manifest = write_manifest(work.path(), "oak.json", &material_manifest());
    let out = run(
        &library,
        &["add", manifest.to_str().unwrap(), "--samples", "1", "--no-denoise"],
    );
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let v = first_json(&out);
    assert_eq!(v["ok"], true);
    let dir = PathBuf::from(v["dir"].as_str().unwrap());
    assert_eq!(png_size(&dir.join("thumbnail.png")), (512, 512));
    let out = run(
        &library,
        &["render", "Materials/Wood/Oak", "--samples", "1", "--no-denoise"],
    );
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(first_json(&out)["ok"], true);
}
#[test]
fn cli_not_found_exit_5() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let out = run(&library, &["show", "3D/Nope/Nothing"]);
    assert_eq!(out.status.code(), Some(5));
    assert_eq!(first_json(&out)["error"]["code"], "not_found");
}

#[test]
fn cli_guide_and_schema_print() {
    let work = tempfile::tempdir().unwrap();
    let library = work.path().join("lib");
    let out = Command::new(env!("CARGO_BIN_EXE_uhub"))
        .arg("guide")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let guide = String::from_utf8(out.stdout).unwrap();
    assert!(guide.contains("# uHub 3D"), "guide is markdown");
    assert!(guide.contains("inspect"));

    let out = Command::new(env!("CARGO_BIN_EXE_uhub"))
        .arg("schema")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let schema: Value = serde_json::from_slice(&out.stdout).expect("schema is JSON");
    assert!(schema["definitions"]["asset"].is_object());

    // Every ```json example in the guide must pass a dry-run.
    let blocks: Vec<String> = guide
        .split("```json")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap_or("").trim().to_string())
        .collect();
    assert_eq!(blocks.len(), 3, "one manifest per asset type");
    let assets_dir = work.path().join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::copy(fx("cube_2mat.fbx"), assets_dir.join("chair.fbx")).unwrap();
    fs::copy(fx("red.png"), assets_dir.join("chair_seat_basecolor.png")).unwrap();
    fs::copy(fx("red.png"), assets_dir.join("oak_basecolor.png")).unwrap();
    fs::copy(fx("red.png"), assets_dir.join("oak_normal.png")).unwrap();
    fs::copy(fx("sky.hdr"), assets_dir.join("sky.hdr")).unwrap();
    for (i, block) in blocks.iter().enumerate() {
        let manifest = assets_dir.join(format!("example{i}.json"));
        fs::write(&manifest, block).unwrap();
        let out = run(&library, &["add", manifest.to_str().unwrap(), "--dry-run"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "guide example {i} must dry-run: {}",
            stdout(&out)
        );
    }
}
