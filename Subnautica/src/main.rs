mod app;
mod bvh;
mod camera;
mod fauna;
mod input;
mod lighting;
mod material;
mod math;
mod noise;
mod primitives;
mod renderer;
mod rl;
mod rng;
mod scene;
mod skybox;
mod texture;
mod underwater_scene;

use renderer::{Quality, Target};
use std::time::Instant;

fn arg<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> T {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    println!("Diorama submarino con raytracing (Rust + raylib)");
    let t0 = Instant::now();
    let scene = underwater_scene::build();
    println!(
        "Escena lista en {:.2} s: {} primitivas, {} nodos BVH, {} materiales",
        t0.elapsed().as_secs_f32(),
        scene.prims.len(),
        scene.bvh.node_count(),
        scene.lib.materials.len()
    );
    for m in &scene.lib.materials {
        println!(
            "  - {:<20} albedo x{:.2}  especular {:.2}  reflectividad {:.2}  transparencia {:.2}  IOR {:.2}",
            m.name,
            m.albedo.luminance(),
            m.specular,
            m.reflectivity,
            m.transparency,
            m.ior
        );
    }

    if args.iter().any(|a| a == "--export-textures") {
        export_textures(&scene);
        return;
    }

    if let Some(i) = args.iter().position(|a| a == "--shot") {
        // Render sin ventana (útil para el README y para pruebas).
        let path = args.get(i + 1).cloned().unwrap_or_else(|| "captura.bmp".into());
        let w: usize = arg(&args, "--w", 1280);
        let h: usize = arg(&args, "--h", 720);
        let passes: u32 = arg(&args, "--passes", 8);
        let time: f32 = arg(&args, "--time", 2.0);
        let mut orbit = input::Orbit::new(
            arg(&args, "--yaw", 0.0),
            arg(&args, "--pitch", 0.10),
            arg(&args, "--dist", 12.5),
        );
        orbit.auto_rotate = false;
        let mut camera = orbit.camera(&scene, w as f32 / h as f32, 0.0);
        // --cam px,py,pz,tx,ty,tz: cámara libre (posición y punto mirado).
        if let Some(j) = args.iter().position(|a| a == "--cam") {
            let v: Vec<f32> = args
                .get(j + 1)
                .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
                .unwrap_or_default();
            if v.len() == 6 {
                camera = camera::Camera::look_at(
                    math::Vec3::new(v[0], v[1], v[2]),
                    math::Vec3::new(v[3], v[4], v[5]),
                    60.0,
                    w as f32 / h as f32,
                );
            }
        }
        let event: f32 = arg(&args, "--event", -1.0);
        let frame = scene.frame(time, arg(&args, "--night", 0.0), (event >= 0.0).then_some(event));
        let quality = if args.iter().any(|a| a == "--fast") {
            Quality::INTERACTIVE
        } else {
            Quality::HIGH
        };
        let mut target = Target::new(w, h);
        let t1 = Instant::now();
        for pass in 0..passes.max(1) {
            renderer::render(&scene, &frame, &camera, quality, &mut target, pass, 0..h);
        }
        let ms = t1.elapsed().as_secs_f32() * 1000.0 / passes.max(1) as f32;
        println!("Render {w}x{h}: {ms:.1} ms por pasada");
        if let Err(e) = renderer::save_bmp(&path, &target.rgba, w, h) {
            eprintln!("No se pudo guardar {path}: {e}");
        } else {
            println!("Guardado {path}");
        }
        return;
    }

    println!("Mouse/A D: girar | W S: inclinar | Rueda/Q E: zoom | R: giro automatico | F: modo buzo");
    println!("N: dia/noche | C: ciclo automatico | Y: ataque | Espacio: pausa | 1-4/0: calidad | P: captura | H: HUD");
    app::run(&scene);
}

/// Guarda las texturas procedurales y las caras del skybox como BMP en `assets/`.
fn export_textures(scene: &scene::Scene) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let to_rgba = |tex: &texture::Texture| {
        let (w, h) = tex.size();
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let c = tex.texel(x, y);
                let g = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
                out.extend_from_slice(&[g(c.x), g(c.y), g(c.z), 255]);
            }
        }
        (out, w, h)
    };
    let _ = std::fs::create_dir_all(base.join("textures"));
    let _ = std::fs::create_dir_all(base.join("skybox"));
    for (tex, name) in scene.lib.textures.iter().zip(&scene.lib.names) {
        let (data, w, h) = to_rgba(tex);
        let _ = renderer::save_bmp(base.join("textures").join(format!("{name}.bmp")), &data, w, h);
    }
    for (i, face) in ["px", "nx", "py", "ny", "pz", "nz"].iter().enumerate() {
        let (data, w, h) = to_rgba(scene.haze.face(i));
        let _ = renderer::save_bmp(base.join("skybox").join(format!("agua_{face}.bmp")), &data, w, h);
        let (data, w, h) = to_rgba(scene.sky.face(i));
        let _ = renderer::save_bmp(base.join("skybox").join(format!("cielo_{face}.bmp")), &data, w, h);
    }
    println!("Texturas exportadas en {}", base.display());
}
