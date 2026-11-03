//! Release bench: render three scenes at 512 px for 10 s each and
//! report the samples reached (Milestone 1.6 performance table).
//! Run with: `cargo run --release --example render_bench`

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use uhub3d_lib::mesh::{load_mesh, uv_sphere, Mesh};
use uhub3d_lib::render::{render, RenderMaterial, RenderOptions, Scene};
use uhub3d_lib::view::View;

fn bench(name: &str, scene: &Scene) {
    let denoise = std::env::var("UHUB_BENCH_DENOISE").is_ok_and(|v| v == "1");
    let opts = RenderOptions {
        size: 512,
        seconds: 10.0,
        max_samples: None,
        seed: 0,
        denoise,
    };
    let cancel = AtomicBool::new(false);
    let samples = std::cell::Cell::new(0u32);
    let start = Instant::now();
    render(scene, &opts, &cancel, |p, _| samples.set(p.samples)).expect("render finishes");
    let elapsed = start.elapsed().as_secs_f32();
    println!(
        "{name}: size=512 seconds={elapsed:.2} spp={} triangles={}",
        samples.get(),
        scene.triangles.len()
    );
}

fn scene_for(mesh: &Mesh) -> Scene {
    let bounds = mesh.bounds();
    let materials = vec![RenderMaterial::default(); mesh.groups.len().max(1)];
    Scene::build(mesh, &bounds, &View::default(), &materials)
}

fn main() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube_2mat.fbx");
    let mesh = load_mesh(&fixture).expect("fixture loads");
    bench("fbx fixture", &scene_for(&mesh));

    let sphere = uv_sphere(96, 48);
    bench("material sphere", &scene_for(&sphere));

    let big = uv_sphere(512, 256);
    bench("uv_sphere(512, 256)", &scene_for(&big));
}
