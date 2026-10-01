//! Enlace mínimo con raylib (solo se usa para abrir la ventana, leer el teclado
//! y el mouse, y mostrar la imagen que calcula el raytracer en la CPU).
//!
//! Se declara directamente la API de C de raylib 5.5 (`raylib.h`), así que no hace
//! falta ningún crate extra ni CMake/LLVM para compilar.

#![allow(non_snake_case, dead_code)]

use std::ffi::{CString, c_char, c_int, c_uint, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Image {
    pub data: *mut c_void,
    pub width: c_int,
    pub height: c_int,
    pub mipmaps: c_int,
    pub format: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Texture2D {
    pub id: c_uint,
    pub width: c_int,
    pub height: c_int,
    pub mipmaps: c_int,
    pub format: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AudioStream {
    buffer: *mut c_void,
    processor: *mut c_void,
    sample_rate: c_uint,
    sample_size: c_uint,
    channels: c_uint,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Music {
    stream: AudioStream,
    frame_count: c_uint,
    looping: bool,
    ctx_type: c_int,
    ctx_data: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Sound {
    stream: AudioStream,
    frame_count: c_uint,
}

unsafe extern "C" {
    fn InitAudioDevice();
    fn CloseAudioDevice();
    fn IsAudioDeviceReady() -> bool;
    fn SetAudioStreamBufferSizeDefault(size: c_int);
    fn LoadMusicStream(file: *const c_char) -> Music;
    fn IsMusicValid(music: Music) -> bool;
    fn UnloadMusicStream(music: Music);
    fn PlayMusicStream(music: Music);
    fn UpdateMusicStream(music: Music);
    fn SetMusicVolume(music: Music, volume: f32);
    fn LoadSound(file: *const c_char) -> Sound;
    fn IsSoundValid(sound: Sound) -> bool;
    fn UnloadSound(sound: Sound);
    fn PlaySound(sound: Sound);
    fn StopSound(sound: Sound);
    fn IsSoundPlaying(sound: Sound) -> bool;
    fn SetSoundVolume(sound: Sound, volume: f32);
}

unsafe extern "C" {
    fn SetConfigFlags(flags: c_uint);
    fn SetTraceLogLevel(level: c_int);
    fn InitWindow(width: c_int, height: c_int, title: *const c_char);
    fn CloseWindow();
    fn WindowShouldClose() -> bool;
    fn SetTargetFPS(fps: c_int);
    fn GetFrameTime() -> f32;
    fn GetTime() -> f64;
    fn GetScreenWidth() -> c_int;
    fn GetScreenHeight() -> c_int;
    fn BeginDrawing();
    fn EndDrawing();
    fn ClearBackground(color: Color);
    fn LoadTextureFromImage(image: Image) -> Texture2D;
    fn UnloadTexture(texture: Texture2D);
    fn UpdateTextureRec(texture: Texture2D, rec: Rectangle, pixels: *const c_void);
    fn SetTextureFilter(texture: Texture2D, filter: c_int);
    fn DrawTexturePro(
        texture: Texture2D,
        source: Rectangle,
        dest: Rectangle,
        origin: Vector2,
        rotation: f32,
        tint: Color,
    );
    fn DrawRectangle(x: c_int, y: c_int, width: c_int, height: c_int, color: Color);
    fn DrawText(text: *const c_char, x: c_int, y: c_int, size: c_int, color: Color);
    fn IsKeyDown(key: c_int) -> bool;
    fn IsKeyPressed(key: c_int) -> bool;
    fn IsMouseButtonDown(button: c_int) -> bool;
    fn GetMouseDelta() -> Vector2;
    fn GetMouseWheelMove() -> f32;
}

pub const FLAG_WINDOW_RESIZABLE: u32 = 0x0000_0004;
pub const FLAG_VSYNC_HINT: u32 = 0x0000_0040;
const LOG_WARNING: c_int = 4;
const PIXELFORMAT_UNCOMPRESSED_R8G8B8A8: c_int = 7;
const TEXTURE_FILTER_BILINEAR: c_int = 1;

pub mod key {
    pub const SPACE: i32 = 32;
    pub const ZERO: i32 = 48;
    pub const ONE: i32 = 49;
    pub const TWO: i32 = 50;
    pub const THREE: i32 = 51;
    pub const FOUR: i32 = 52;
    pub const A: i32 = 65;
    pub const C: i32 = 67;
    pub const F: i32 = 70;
    pub const N: i32 = 78;
    pub const LEFT_SHIFT: i32 = 340;
    pub const D: i32 = 68;
    pub const E: i32 = 69;
    pub const H: i32 = 72;
    pub const P: i32 = 80;
    pub const Q: i32 = 81;
    pub const R: i32 = 82;
    pub const Y: i32 = 89;
    pub const S: i32 = 83;
    pub const W: i32 = 87;
    pub const RIGHT: i32 = 262;
    pub const LEFT: i32 = 263;
    pub const DOWN: i32 = 264;
    pub const UP: i32 = 265;
    pub const F12: i32 = 301;
    pub const KP_SUBTRACT: i32 = 333;
    pub const KP_ADD: i32 = 334;
    pub const MINUS: i32 = 45;
    pub const EQUAL: i32 = 61;
}

pub const MOUSE_LEFT: i32 = 0;

pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}

/// Ventana de raylib. Se cierra sola al salir de alcance.
pub struct Window {
    _private: (),
}

impl Window {
    pub fn open(width: i32, height: i32, title: &str) -> Self {
        let title = CString::new(title).unwrap_or_default();
        unsafe {
            SetTraceLogLevel(LOG_WARNING);
            SetConfigFlags(FLAG_WINDOW_RESIZABLE | FLAG_VSYNC_HINT);
            InitWindow(width, height, title.as_ptr());
            SetTargetFPS(0);
        }
        Self { _private: () }
    }

    pub fn should_close(&self) -> bool {
        unsafe { WindowShouldClose() }
    }

    pub fn frame_time(&self) -> f32 {
        unsafe { GetFrameTime() }
    }

    pub fn time(&self) -> f64 {
        unsafe { GetTime() }
    }

    pub fn size(&self) -> (i32, i32) {
        unsafe { (GetScreenWidth(), GetScreenHeight()) }
    }

    pub fn key_down(&self, k: i32) -> bool {
        unsafe { IsKeyDown(k) }
    }

    pub fn key_pressed(&self, k: i32) -> bool {
        unsafe { IsKeyPressed(k) }
    }

    pub fn mouse_down(&self, b: i32) -> bool {
        unsafe { IsMouseButtonDown(b) }
    }

    pub fn mouse_delta(&self) -> (f32, f32) {
        let d = unsafe { GetMouseDelta() };
        (d.x, d.y)
    }

    pub fn wheel(&self) -> f32 {
        unsafe { GetMouseWheelMove() }
    }

    pub fn begin(&self) {
        unsafe {
            BeginDrawing();
            ClearBackground(rgba(4, 30, 40, 255));
        }
    }

    pub fn end(&self) {
        unsafe { EndDrawing() }
    }

    pub fn rect(&self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        unsafe { DrawRectangle(x, y, w, h, color) }
    }

    pub fn text(&self, text: &str, x: i32, y: i32, size: i32, color: Color) {
        if let Ok(t) = CString::new(text) {
            unsafe { DrawText(t.as_ptr(), x, y, size, color) }
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { CloseWindow() }
    }
}

/// Textura de GPU donde se sube la imagen del raytracer en cada cuadro.
pub struct Canvas {
    texture: Texture2D,
    pub w: usize,
    pub h: usize,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        let mut pixels = vec![0u8; w * h * 4];
        let image = Image {
            data: pixels.as_mut_ptr().cast(),
            width: w as c_int,
            height: h as c_int,
            mipmaps: 1,
            format: PIXELFORMAT_UNCOMPRESSED_R8G8B8A8,
        };
        let texture = unsafe { LoadTextureFromImage(image) };
        unsafe { SetTextureFilter(texture, TEXTURE_FILTER_BILINEAR) };
        Self { texture, w, h }
    }

    /// Sube `rgba` (de tamaño `w*h*4`) a la esquina superior izquierda de la textura.
    pub fn upload(&mut self, rgba: &[u8], w: usize, h: usize) {
        assert!(w <= self.w && h <= self.h && rgba.len() >= w * h * 4);
        let rec = Rectangle {
            x: 0.0,
            y: 0.0,
            width: w as f32,
            height: h as f32,
        };
        unsafe { UpdateTextureRec(self.texture, rec, rgba.as_ptr().cast()) };
    }

    /// Dibuja la región `w*h` escalada a toda la ventana.
    pub fn draw(&self, w: usize, h: usize, dest_w: i32, dest_h: i32) {
        // Centros de los texeles de borde: evita mezclar con datos fuera de la región.
        let src = Rectangle {
            x: 0.5,
            y: 0.5,
            width: w as f32 - 1.0,
            height: h as f32 - 1.0,
        };
        let dst = Rectangle {
            x: 0.0,
            y: 0.0,
            width: dest_w as f32,
            height: dest_h as f32,
        };
        unsafe {
            DrawTexturePro(
                self.texture,
                src,
                dst,
                Vector2 { x: 0.0, y: 0.0 },
                0.0,
                rgba(255, 255, 255, 255),
            )
        };
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe { UnloadTexture(self.texture) }
    }
}

/// Dispositivo de audio de raylib (música de fondo y efectos).
pub struct Audio {
    ok: bool,
}

impl Audio {
    pub fn open() -> Self {
        unsafe {
            InitAudioDevice();
            // Búfer grande: el raytracer ocupa el hilo principal varios ms por cuadro
            // y así la música no se corta.
            SetAudioStreamBufferSizeDefault(16384);
            Self { ok: IsAudioDeviceReady() }
        }
    }

    pub fn ready(&self) -> bool {
        self.ok
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        if self.ok {
            unsafe { CloseAudioDevice() }
        }
    }
}

/// Pista en streaming que se repite (música, ambiente).
pub struct Track {
    music: Option<Music>,
}

impl Track {
    pub fn load(audio: &Audio, path: &str, volume: f32) -> Self {
        if !audio.ready() {
            return Self { music: None };
        }
        let Ok(c) = CString::new(path) else { return Self { music: None } };
        let music = unsafe { LoadMusicStream(c.as_ptr()) };
        if !unsafe { IsMusicValid(music) } {
            eprintln!("No se pudo cargar el audio {path}");
            return Self { music: None };
        }
        unsafe {
            SetMusicVolume(music, volume);
            PlayMusicStream(music);
        }
        Self { music: Some(music) }
    }

    pub fn set_volume(&self, volume: f32) {
        if let Some(m) = self.music {
            unsafe { SetMusicVolume(m, volume.clamp(0.0, 1.0)) }
        }
    }

    /// Hay que llamarlo cada cuadro para rellenar el búfer.
    pub fn update(&self) {
        if let Some(m) = self.music {
            unsafe { UpdateMusicStream(m) }
        }
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        if let Some(m) = self.music {
            unsafe { UnloadMusicStream(m) }
        }
    }
}

/// Efecto de sonido corto.
pub struct Sfx {
    sound: Option<Sound>,
}

impl Sfx {
    pub fn load(audio: &Audio, path: &str) -> Self {
        if !audio.ready() {
            return Self { sound: None };
        }
        let Ok(c) = CString::new(path) else { return Self { sound: None } };
        let sound = unsafe { LoadSound(c.as_ptr()) };
        if !unsafe { IsSoundValid(sound) } {
            eprintln!("No se pudo cargar el audio {path}");
            return Self { sound: None };
        }
        Self { sound: Some(sound) }
    }

    pub fn play(&self, volume: f32) {
        if let Some(s) = self.sound {
            unsafe {
                SetSoundVolume(s, volume.clamp(0.0, 1.0));
                PlaySound(s);
            }
        }
    }

    pub fn stop(&self) {
        if let Some(s) = self.sound {
            unsafe { StopSound(s) }
        }
    }

    pub fn playing(&self) -> bool {
        self.sound.is_some_and(|s| unsafe { IsSoundPlaying(s) })
    }
}

impl Drop for Sfx {
    fn drop(&mut self) {
        if let Some(s) = self.sound {
            unsafe { UnloadSound(s) }
        }
    }
}
