//! Cámara orbital: rotación 360° alrededor del diorama, inclinación y zoom,
//! con suavizado y colisión para que la cámara no atraviese las rocas.

use crate::{
    camera::Camera,
    math::{Ray, Vec3},
    rl::{MOUSE_LEFT, Window, key},
    scene::Scene,
};

pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    goal_yaw: f32,
    goal_pitch: f32,
    goal_dist: f32,
    pub auto_rotate: bool,
    eff_dist: f32,
}

pub const MIN_DIST: f32 = 3.0;
pub const MAX_DIST: f32 = 24.0;

impl Orbit {
    pub fn new(yaw: f32, pitch: f32, dist: f32) -> Self {
        Self {
            yaw,
            pitch,
            dist,
            goal_yaw: yaw,
            goal_pitch: pitch,
            goal_dist: dist,
            auto_rotate: false,
            eff_dist: dist,
        }
    }

    pub fn update(&mut self, win: &Window, dt: f32) {
        let turn = 1.3 * dt;
        if win.key_down(key::A) || win.key_down(key::LEFT) {
            self.goal_yaw -= turn;
        }
        if win.key_down(key::D) || win.key_down(key::RIGHT) {
            self.goal_yaw += turn;
        }
        if win.key_down(key::W) || win.key_down(key::UP) {
            self.goal_pitch += turn * 0.6;
        }
        if win.key_down(key::S) || win.key_down(key::DOWN) {
            self.goal_pitch -= turn * 0.6;
        }
        if win.key_down(key::E) || win.key_down(key::EQUAL) || win.key_down(key::KP_ADD) {
            self.goal_dist *= 1.0 - 1.4 * dt;
        }
        if win.key_down(key::Q) || win.key_down(key::MINUS) || win.key_down(key::KP_SUBTRACT) {
            self.goal_dist *= 1.0 + 1.4 * dt;
        }
        let wheel = win.wheel();
        if wheel != 0.0 {
            self.goal_dist *= 0.88f32.powf(wheel);
        }
        if win.mouse_down(MOUSE_LEFT) {
            let (dx, dy) = win.mouse_delta();
            self.goal_yaw += dx * 0.006;
            self.goal_pitch += dy * 0.004;
        }
        if self.auto_rotate {
            self.goal_yaw += 0.22 * dt;
        }
        self.goal_pitch = self.goal_pitch.clamp(-0.55, 1.15);
        self.goal_dist = self.goal_dist.clamp(MIN_DIST, MAX_DIST);
        let k = 1.0 - (-dt * 9.0).exp();
        self.yaw += (self.goal_yaw - self.yaw) * k;
        self.pitch += (self.goal_pitch - self.pitch) * k;
        self.dist += (self.goal_dist - self.dist) * k;
    }

    pub fn camera(&mut self, scene: &Scene, aspect: f32, dt: f32) -> Camera {
        let focus = scene.focus;
        let dir = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        );
        // Si una formación lejana (a más de 7 m del centro) queda entre la cámara y el
        // arco, la cámara pasa al frente de ella. El arco en sí no la mueve.
        let mut d = self.dist;
        for _ in 0..24 {
            let limit = d - 7.0;
            if limit <= 0.0 {
                break;
            }
            let pos = focus + dir * d;
            let inside = scene.inside_solid(pos, 0.3);
            match scene.first_opaque(&Ray::new(pos, -dir), limit) {
                Some(t) => d -= t + 0.3,
                None if inside => d -= 0.4,
                None => break,
            }
        }
        // Cerca del centro: nunca dentro de una roca (se aleja hasta quedar libre).
        for _ in 0..40 {
            if !scene.inside_solid(focus + dir * d, 0.3) {
                break;
            }
            d += 0.25;
        }
        // Al acercarse por un obstáculo se salta de inmediato; al alejarse, suave.
        if d < self.eff_dist || dt <= 0.0 {
            self.eff_dist = d;
        } else {
            self.eff_dist += (d - self.eff_dist) * (1.0 - (-dt * 3.0).exp());
        }
        let mut pos = focus + dir * self.eff_dist;
        pos.y = pos.y.clamp(0.35, scene.water_y - 0.4);
        // Pegada al fondo podría quedar dentro de una duna: se sube un poco.
        for _ in 0..12 {
            if !scene.inside_solid(pos, 0.2) {
                break;
            }
            pos.y += 0.25;
        }
        Camera::look_at(pos, focus, 55.0, aspect)
    }
}

/// Cámara libre ("modo buzo"): se nada por el diorama sin salir de sus límites
/// ni atravesar las rocas.
pub struct FreeCam {
    pub pos: Vec3,
    yaw: f32,
    pitch: f32,
}

/// Mitad del ancho del área por la que se puede nadar.
const FREE_LIMIT: f32 = 20.0;

impl FreeCam {
    pub fn from_camera(cam: &Camera) -> Self {
        let f = cam.forward();
        Self {
            pos: cam.position,
            yaw: f.x.atan2(f.z),
            pitch: f.y.clamp(-1.0, 1.0).asin(),
        }
    }

    fn look(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        )
    }

    pub fn update(&mut self, win: &Window, scene: &Scene, dt: f32) {
        // Mirar: arrastrar con el mouse o flechas.
        if win.mouse_down(MOUSE_LEFT) {
            let (dx, dy) = win.mouse_delta();
            self.yaw -= dx * 0.005;
            self.pitch -= dy * 0.004;
        }
        let turn = 1.4 * dt;
        if win.key_down(key::LEFT) {
            self.yaw += turn;
        }
        if win.key_down(key::RIGHT) {
            self.yaw -= turn;
        }
        if win.key_down(key::UP) {
            self.pitch += turn * 0.7;
        }
        if win.key_down(key::DOWN) {
            self.pitch -= turn * 0.7;
        }
        self.pitch = self.pitch.clamp(-1.35, 1.35);

        // Nadar: W/S adelante-atrás, A/D a los lados, E/Q subir-bajar, Shift = rápido.
        let fwd = self.look();
        let right = fwd.cross(Vec3::UP).normalized();
        let mut mv = Vec3::ZERO;
        if win.key_down(key::W) {
            mv += fwd;
        }
        if win.key_down(key::S) {
            mv -= fwd;
        }
        if win.key_down(key::D) {
            mv += right;
        }
        if win.key_down(key::A) {
            mv -= right;
        }
        if win.key_down(key::E) {
            mv += Vec3::UP;
        }
        if win.key_down(key::Q) {
            mv -= Vec3::UP;
        }
        let speed = if win.key_down(key::LEFT_SHIFT) { 7.0 } else { 3.0 };
        let mut delta = if mv.length_sq() > 0.0 { mv.normalized() * (speed * dt) } else { Vec3::ZERO };
        delta += fwd * (win.wheel() * 0.6);
        // Se mueve eje por eje para "resbalar" a lo largo de las rocas.
        for axis in 0..3 {
            let mut next = self.pos;
            match axis {
                0 => next.x += delta.x,
                1 => next.y += delta.y,
                _ => next.z += delta.z,
            }
            next.x = next.x.clamp(-FREE_LIMIT, FREE_LIMIT);
            next.z = next.z.clamp(-FREE_LIMIT, FREE_LIMIT);
            next.y = next.y.clamp(0.4, scene.water_y - 0.5);
            if !scene.inside_solid(next, 0.3) {
                self.pos = next;
            }
        }
    }

    pub fn camera(&self, aspect: f32) -> Camera {
        Camera::look_at(self.pos, self.pos + self.look(), 60.0, aspect)
    }
}
