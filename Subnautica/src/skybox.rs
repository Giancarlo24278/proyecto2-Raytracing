//! Skybox con cubemaps generados por código:
//! - `sky`: el cielo sobre el mar, visible a través de la ventana de Snell.
//! - `haze`: el fondo submarino (agua lejana, siluetas de rocas y haces de luz).

use crate::{
    math::{Vec3, smoothstep},
    noise::{fbm2, value2},
    texture::CubeMap,
};
use std::f32::consts::TAU;

pub const HORIZON: Vec3 = Vec3::new(0.025, 0.25, 0.34);
pub const DEEP: Vec3 = Vec3::new(0.006, 0.07, 0.11);
pub const SURFACE_GLOW: Vec3 = Vec3::new(0.12, 0.52, 0.60);

/// Degradado base del agua (también es el color de la niebla).
pub fn water_gradient(d: Vec3) -> Vec3 {
    if d.y >= 0.0 {
        HORIZON.lerp(SURFACE_GLOW, smoothstep(0.0, 0.85, d.y).powf(0.8))
    } else {
        HORIZON.lerp(DEEP, smoothstep(0.0, 0.55, -d.y))
    }
}

/// Cubemap del fondo submarino.
pub fn build_haze(size: usize) -> CubeMap {
    CubeMap::generate(size, |d| {
        let mut c = water_gradient(d);
        let az = d.z.atan2(d.x) / TAU + 0.5;
        let y = d.y;

        // Capa lejana de formaciones rocosas (muy diluida por la distancia).
        let far_h = -0.02 + 0.10 * fbm2(az * 9.0, 0.5, 9, 4)
            + 0.10 * smoothstep(0.72, 0.95, value2(az * 5.0, 3.5, 5));
        if y < far_h {
            let k = smoothstep(far_h, far_h - 0.03, y);
            c = c.lerp(Vec3::new(0.030, 0.24, 0.31), 0.45 * k);
        }
        // Capa más cercana: agujas y un arco distante.
        let spire = smoothstep(0.80, 0.97, value2(az * 14.0, 7.0, 14));
        let near_h = -0.06 + 0.07 * fbm2(az * 6.0 + 2.0, 1.5, 6, 4) + 0.22 * spire;
        if y < near_h {
            let k = smoothstep(near_h, near_h - 0.02, y);
            c = c.lerp(Vec3::new(0.022, 0.19, 0.26), 0.55 * k);
        }
        // Manchas lejanas de pasto rojo sobre el fondo (bajo el horizonte).
        if y < 0.02 && y > -0.35 {
            let patch = smoothstep(0.62, 0.78, fbm2(az * 22.0, (y + 1.0) * 6.0, 22, 4));
            let k = patch * smoothstep(-0.35, -0.04, y) * (1.0 - smoothstep(-0.01, 0.02, y));
            c = c.lerp(Vec3::new(0.16, 0.05, 0.09), k * 0.3);
        }
        // Haces de luz que bajan desde la superficie.
        let shafts = value2(az * 46.0, 0.0, 46).powi(3) * smoothstep(-0.05, 0.45, y)
            * (1.0 - smoothstep(0.55, 0.95, y));
        c += Vec3::new(0.05, 0.16, 0.16) * shafts;
        c
    })
}

/// Dirección (en el aire) hacia el sol.
pub fn sun_air_direction() -> Vec3 {
    Vec3::new(0.42, 0.80, 0.30).normalized()
}

/// Cubemap del cielo visto desde fuera del agua (sol, nubes y horizonte).
pub fn build_sky(size: usize) -> CubeMap {
    let sun = sun_air_direction();
    CubeMap::generate(size, move |d| {
        if d.y < 0.0 {
            return Vec3::new(0.02, 0.12, 0.18);
        }
        let zenith = Vec3::new(0.10, 0.32, 0.85);
        let horizon = Vec3::new(0.62, 0.80, 0.95);
        let mut c = horizon.lerp(zenith, smoothstep(0.0, 0.7, d.y).powf(0.7));
        // Nubes proyectadas sobre un plano.
        let k = 1.0 / (d.y + 0.12);
        let px = d.x * k * 1.4;
        let pz = d.z * k * 1.4;
        let cloud = smoothstep(0.50, 0.78, fbm2(px + 13.0, pz + 7.0, 0, 6));
        c = c.lerp(Vec3::new(1.0, 1.0, 1.0) * 1.1, cloud * smoothstep(0.02, 0.25, d.y) * 0.85);
        // Sol HDR con halo.
        let cos_s = d.dot(sun);
        let disk = smoothstep(0.9993, 0.9997, cos_s);
        let glow = cos_s.max(0.0).powf(220.0) * 3.0 + cos_s.max(0.0).powf(16.0) * 0.35;
        c + Vec3::new(1.0, 0.93, 0.80) * (disk * 40.0 + glow)
    })
}
