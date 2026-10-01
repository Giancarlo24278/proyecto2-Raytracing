//! Bucle interactivo: ventana de raylib, resolución dinámica y refinamiento progresivo.

use crate::{
    camera::Camera,
    fauna,
    input::{FreeCam, Orbit},
    renderer::{self, Quality, Target},
    rl::{Audio, Canvas, Sfx, Track, Window, key, rgba},
    scene::Scene,
};
use std::time::Instant;

const MAX_W: usize = 1920;
const MAX_H: usize = 1080;
/// Muestras acumuladas cuando la escena está quieta (antialiasing).
const MAX_PASSES: u32 = 32;
/// Duración del ciclo automático de día y noche (segundos).
const CYCLE: f32 = 120.0;
const MUSIC_VOLUME: f32 = 0.16;
/// Solo se reproducen los primeros segundos del audio del rugido.
const ROAR_LENGTH: f32 = 4.7;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Auto,
    Fixed(f32),
}

pub fn run(scene: &Scene) {
    let win = Window::open(1280, 720, "Diorama submarino | Raytracing en CPU + raylib");
    let mut canvas = Canvas::new(MAX_W, MAX_H);
    let mut orbit = Orbit::new(0.0, 0.10, 12.5);
    let mut target = Target::new(640, 360);
    let mut mode = Mode::Auto;
    let mut scale: f32 = 0.5;
    let mut time: f32 = 0.0;
    let mut animate = true;
    let mut hud = true;
    let mut last_camera: Option<Camera> = None;
    let mut last_time = -1.0f32;
    let mut pass: u32 = 0;
    let mut row_cursor: usize = 0;
    let mut band: usize = 32;
    let mut render_ms = 0.0f32;
    let mut fps_smooth = 0.0f32;
    let mut shots = 0;
    let mut message: Option<(String, f64)> = None;
    // Día / noche: `night` se acerca suavemente a `night_goal`.
    let mut night: f32 = 0.0;
    let mut night_goal: f32 = 0.0;
    let mut auto_cycle = false;
    let mut cycle_time: f32 = 0.0;
    let mut last_night = -1.0f32;
    let mut free: Option<FreeCam> = None;

    // --- Audio (también con raylib) ------------------------------------------------
    let audio = Audio::open();
    let music = Track::load(&audio, &audio_path("musica_salutations.mp3"), MUSIC_VOLUME);
    let ambience = Track::load(&audio, &audio_path("ambiente_agua.mp3"), 0.22);
    let sfx_peeper = Sfx::load(&audio, &audio_path("peeper.mp3"));
    let sfx_bubble = Sfx::load(&audio, &audio_path("burbuja.mp3"));
    let sfx_stalker = Sfx::load(&audio, &audio_path("stalker.mp3"));
    let mut peeper_wait = 0.0f32;
    let mut stalker_wait = 0.0f32;
    let mut sound_time = 0.0f32;
    let mut last_cam_for_free: Option<Camera> = None;
    // Evento "ataque": tiempo transcurrido (None = no está pasando).
    let sfx_roar = Sfx::load(&audio, &audio_path("rugido.mp3"));
    let mut event: Option<f32> = None;
    let mut roar_time: Option<f32> = None;
    let mut second_roar = false;
    let mut music_volume = MUSIC_VOLUME;

    while !win.should_close() {
        let dt = win.frame_time().clamp(0.0, 0.1);
        if dt > 0.0 {
            fps_smooth = fps_smooth * 0.9 + (1.0 / dt) * 0.1;
        }

        // --- Teclado -------------------------------------------------------------
        if win.key_pressed(key::SPACE) {
            animate = !animate;
        }
        if win.key_pressed(key::R) {
            orbit.auto_rotate = !orbit.auto_rotate;
        }
        if win.key_pressed(key::H) {
            hud = !hud;
        }
        if win.key_pressed(key::N) {
            auto_cycle = false;
            night_goal = if night_goal > 0.5 { 0.0 } else { 1.0 };
        }
        if win.key_pressed(key::C) {
            auto_cycle = !auto_cycle;
            // Empieza el ciclo desde la hora actual.
            cycle_time = (1.0 - 2.0 * night.clamp(0.0, 1.0)).acos() / std::f32::consts::TAU * CYCLE;
        }
        // Y = ataque: solo se puede activar cuando el anterior ya terminó.
        if win.key_pressed(key::Y) && event.is_none() {
            event = Some(0.0);
            second_roar = false;
            sfx_roar.stop();
            sfx_roar.play(0.35);
            roar_time = Some(0.0);
        }
        if win.key_pressed(key::F) {
            free = match free {
                Some(_) => None,
                None => last_cam_for_free.as_ref().map(FreeCam::from_camera),
            };
        }
        if auto_cycle {
            // Ciclo completo de 2 minutos: día -> noche -> día.
            cycle_time += dt;
            night = 0.5 - 0.5 * (cycle_time / CYCLE * std::f32::consts::TAU).cos();
            night_goal = if night > 0.5 { 1.0 } else { 0.0 };
        } else {
            // Transición gradual de ~4 segundos.
            let step = dt / 4.0;
            night += (night_goal - night).clamp(-step, step);
        }
        for (k, m) in [
            (key::ZERO, Mode::Auto),
            (key::ONE, Mode::Fixed(0.33)),
            (key::TWO, Mode::Fixed(0.5)),
            (key::THREE, Mode::Fixed(0.75)),
            (key::FOUR, Mode::Fixed(1.0)),
        ] {
            if win.key_pressed(k) {
                mode = m;
                if let Mode::Fixed(s) = m {
                    scale = s;
                }
            }
        }
        if let Some(f) = free.as_mut() {
            f.update(&win, scene, dt);
        } else {
            orbit.update(&win, dt);
        }
        if animate {
            time += dt;
        }

        let (ww, wh) = win.size();
        let (ww, wh) = (ww.max(64), wh.max(64));
        let aspect = ww as f32 / wh as f32;
        let camera = match &free {
            Some(f) => f.camera(aspect),
            None => orbit.camera(scene, aspect, dt),
        };
        last_cam_for_free = Some(camera);

        if win.key_pressed(key::P) || win.key_pressed(key::F12) {
            win.begin();
            canvas.draw(target.w, target.h, ww, wh);
            win.rect(0, 0, ww, 40, rgba(0, 0, 0, 160));
            win.text("Guardando captura en alta calidad...", 14, 10, 20, rgba(255, 255, 255, 255));
            win.end();
            shots += 1;
            let name = format!("captura_{shots:02}.bmp");
            let ok = save_screenshot(scene, &camera, time, night, ww as usize, wh as usize, &name);
            message = Some((
                if ok { format!("Captura guardada: {name}") } else { "No se pudo guardar".into() },
                win.time() + 3.0,
            ));
            last_camera = None;
        }

        let moved = match &last_camera {
            Some(c) => !c.same_as(&camera),
            None => true,
        };
        // --- Evento del leviatán ----------------------------------------------------
        let mut music_goal = MUSIC_VOLUME;
        if let Some(e) = event.as_mut() {
            *e += dt;
            let e = *e;
            if e >= fauna::EVENT_CENTER - 1.6 && !second_roar {
                // Segundo rugido, fuerte, justo cuando pasa por el centro.
                second_roar = true;
                sfx_roar.stop();
                sfx_roar.play(1.0);
                roar_time = Some(0.0);
            }
            music_goal = if e < fauna::EVENT_APPEAR {
                MUSIC_VOLUME * 0.3
            } else if e < fauna::EVENT_CENTER + 4.0 {
                0.0
            } else {
                MUSIC_VOLUME
            };
            if e >= fauna::EVENT_LENGTH {
                event = None;
            }
        }
        // Solo se usan los primeros 4.7 s del audio (el resto repite el rugido).
        if let Some(t) = roar_time.as_mut() {
            *t += dt;
            if *t > ROAR_LENGTH {
                sfx_roar.stop();
                roar_time = None;
            }
        }
        music_volume += (music_goal - music_volume) * (dt * 1.5).min(1.0);
        music.set_volume(music_volume);

        let changed = moved
            || (time - last_time).abs() > 1e-6
            || (night - last_night).abs() > 1e-5
            || event.is_some();
        let frame = scene.frame(time, night, event);

        // --- Sonidos según dónde está la cámara -------------------------------------
        music.update();
        ambience.update();
        peeper_wait -= dt;
        stalker_wait -= dt;
        let ear = camera.position;
        let nearest = |species: u16| {
            frame
                .creatures
                .iter()
                .filter(|c| c.species == species)
                .map(|c| (c.pos - ear).length())
                .fold(f32::INFINITY, f32::min)
        };
        let d_peeper = nearest(0);
        if d_peeper < 4.5 && peeper_wait <= 0.0 && !sfx_peeper.playing() {
            sfx_peeper.play(0.15 + 0.5 * (1.0 - d_peeper / 4.5));
            peeper_wait = 4.0 + (time * 7.3).sin().abs() * 2.0;
        }
        let d_stalker = nearest(3);
        if d_stalker < 10.0 && stalker_wait <= 0.0 && !sfx_stalker.playing() {
            sfx_stalker.play(0.25 + 0.35 * (1.0 - d_stalker / 10.0));
            stalker_wait = 6.0 + (time * 3.1).sin().abs() * 3.0;
        }
        if time > sound_time {
            for p in scene.bubble_spawns(sound_time, time) {
                let d = (p - ear).length();
                sfx_bubble.play(0.28 / (1.0 + d * 0.12));
            }
        }
        sound_time = time;

        if changed {
            // Cuadro interactivo completo con resolución dinámica.
            let rw = ((ww as f32 * scale) as usize).clamp(64, MAX_W) & !1;
            let rh = ((wh as f32 * scale) as usize).clamp(36, MAX_H) & !1;
            target.resize(rw, rh);
            let t0 = Instant::now();
            renderer::render(scene, &frame, &camera, Quality::INTERACTIVE, &mut target, 0, 0..rh);
            render_ms = t0.elapsed().as_secs_f32() * 1000.0;
            if mode == Mode::Auto {
                // Objetivo ~30 FPS: ajusta el área renderizada según el tiempo medido.
                let wanted = (scale * (33.0 / render_ms.max(1.0)).sqrt()).clamp(0.25, 1.0);
                scale += (wanted - scale) * 0.3;
                scale = (scale * 32.0).round() / 32.0;
            }
            canvas.upload(&target.rgba, target.w, target.h);
            pass = 0;
            row_cursor = 0;
            last_camera = Some(camera);
            last_time = time;
            last_night = night;
        } else if pass < MAX_PASSES {
            // Escena quieta: refinamiento progresivo por franjas (no congela la ventana).
            let fw = (ww as usize).min(MAX_W) & !1;
            let fh = (wh as usize).min(MAX_H) & !1;
            if pass == 0 && row_cursor == 0 {
                target.resize_preserving(fw, fh);
            }
            let end = (row_cursor + band).min(target.h);
            let t0 = Instant::now();
            renderer::render(scene, &frame, &camera, Quality::HIGH, &mut target, pass, row_cursor..end);
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            // Ajusta el alto de la franja para gastar ~45 ms por cuadro.
            let per_row = ms / (end - row_cursor).max(1) as f32;
            band = ((45.0 / per_row.max(0.01)) as usize).clamp(8, 1080);
            row_cursor = end;
            if row_cursor >= target.h {
                row_cursor = 0;
                pass += 1;
            }
            canvas.upload(&target.rgba, target.w, target.h);
        } else {
            // Imagen terminada y nada cambia: no gastar CPU.
            std::thread::sleep(std::time::Duration::from_millis(8));
        }

        // --- Dibujo ---------------------------------------------------------------
        win.begin();
        canvas.draw(target.w, target.h, ww, wh);
        if hud {
            draw_hud(&win, fps_smooth, render_ms, &target, scale, mode, animate, orbit.auto_rotate, pass, free.is_some(), night, auto_cycle);
        }
        if let Some((text, until)) = &message {
            if win.time() < *until {
                win.rect(0, wh - 40, ww, 40, rgba(0, 0, 0, 150));
                win.text(text, 14, wh - 30, 20, rgba(255, 240, 160, 255));
            }
        }
        win.end();
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_hud(
    win: &Window,
    fps: f32,
    render_ms: f32,
    target: &Target,
    scale: f32,
    mode: Mode,
    animate: bool,
    auto_rotate: bool,
    pass: u32,
    free: bool,
    night: f32,
    auto_cycle: bool,
) {
    let mode_text = match mode {
        Mode::Auto => format!("auto {:.0}%", scale * 100.0),
        Mode::Fixed(s) => format!("fija {:.0}%", s * 100.0),
    };
    let status = if !animate {
        if pass < MAX_PASSES {
            format!("Pausa: refinando {}/{}", pass + 1, MAX_PASSES)
        } else {
            "Pausa: imagen final".to_string()
        }
    } else {
        "Animacion: ON".to_string()
    };
    let white = rgba(235, 250, 255, 255);
    let soft = rgba(170, 225, 235, 255);
    let hour = if night < 0.05 {
        "Dia"
    } else if night > 0.95 {
        "Noche"
    } else if night > 0.5 {
        "Anocheciendo"
    } else {
        "Amaneciendo"
    };
    win.rect(8, 8, 600, 128, rgba(0, 20, 30, 150));
    win.text(
        &format!("FPS {:.0}  |  render {:.0} ms  |  {}x{} ({})", fps, render_ms, target.w, target.h, mode_text),
        18,
        16,
        18,
        white,
    );
    win.text(
        &format!(
            "{status}  |  {}  |  {hour}{}",
            if free { "Modo buzo" } else if auto_rotate { "Orbita (giro auto)" } else { "Orbita" },
            if auto_cycle { " (ciclo 2 min)" } else { "" }
        ),
        18,
        38,
        18,
        white,
    );
    if free {
        win.text("Mouse/flechas: mirar   W A S D: nadar   E/Q: subir/bajar   Shift: rapido", 18, 62, 16, soft);
    } else {
        win.text("Mouse/A D: girar   W S: inclinar   Rueda/Q E: zoom   R: giro auto", 18, 62, 16, soft);
    }
    win.text("F: modo buzo/orbita   N: dia/noche   C: ciclo automatico   Y: ataque", 18, 84, 16, soft);
    win.text("Espacio: pausa  1-4/0: calidad  P: captura  H: ocultar", 18, 106, 16, soft);
}

/// Render de alta calidad (varias muestras por píxel) guardado como BMP.
pub fn save_screenshot(scene: &Scene, camera: &Camera, time: f32, night: f32, w: usize, h: usize, path: &str) -> bool {
    let frame = scene.frame(time, night, None);
    let mut target = Target::new(w, h);
    for pass in 0..12 {
        renderer::render(scene, &frame, camera, Quality::HIGH, &mut target, pass, 0..h);
    }
    renderer::save_bmp(path, &target.rgba, w, h).is_ok()
}

/// Ruta de un archivo de audio (junto al proyecto, o relativa a la carpeta actual).
fn audio_path(name: &str) -> String {
    let in_project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("audio").join(name);
    if in_project.exists() {
        in_project.to_string_lossy().into_owned()
    } else {
        format!("assets/audio/{name}")
    }
}
