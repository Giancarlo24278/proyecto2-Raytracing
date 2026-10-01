//! Enlaza raylib 5.5 precompilado (incluido en `vendor/raylib`).
//! No usa crates externos: solo la librería estándar de Rust.

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let vendor = manifest.join("vendor").join("raylib");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=vendor/raylib");

    match os.as_str() {
        "windows" => {
            if target_env != "msvc" {
                panic!(
                    "Este proyecto incluye raylib para el toolchain MSVC de Windows. \
                     Usa `rustup default stable-x86_64-pc-windows-msvc`."
                );
            }
            let dir = vendor.join("windows-msvc");
            println!("cargo:rustc-link-search=native={}", dir.display());
            // raylibdll.lib es la librería de importación de raylib.dll.
            println!("cargo:rustc-link-lib=dylib=raylibdll");
            // Copia raylib.dll junto al .exe (target/debug o target/release).
            let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
            if let Some(profile_dir) = out.ancestors().nth(3) {
                let dll = dir.join("raylib.dll");
                let _ = fs::copy(&dll, profile_dir.join("raylib.dll"));
                let deps = profile_dir.join("deps");
                if deps.exists() {
                    let _ = fs::copy(&dll, deps.join("raylib.dll"));
                }
            }
        }
        "linux" => {
            let dir = vendor.join("linux");
            if dir.exists() {
                println!("cargo:rustc-link-search=native={}", dir.display());
            }
            println!("cargo:rustc-link-lib=static=raylib");
            for lib in ["GL", "m", "pthread", "dl", "rt", "X11"] {
                println!("cargo:rustc-link-lib=dylib={lib}");
            }
        }
        "macos" => {
            let dir = vendor.join("macos");
            if dir.exists() {
                println!("cargo:rustc-link-search=native={}", dir.display());
            }
            println!("cargo:rustc-link-lib=static=raylib");
            for fw in ["OpenGL", "Cocoa", "IOKit", "CoreVideo", "CoreAudio", "AudioToolbox"] {
                println!("cargo:rustc-link-lib=framework={fw}");
            }
        }
        other => panic!("Sistema operativo no soportado: {other}"),
    }
}
