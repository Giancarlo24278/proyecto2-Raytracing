//! Ruido procedural (value noise, fbm y Voronoi) escrito a mano, sin librerías.

use crate::math::{Vec3, fract};

#[inline(always)]
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline(always)]
pub fn hash2(x: i32, y: i32) -> f32 {
    let h = hash_u32((x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841));
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

#[inline(always)]
pub fn hash3(x: i32, y: i32, z: i32) -> f32 {
    let h = hash_u32(
        (x as u32).wrapping_mul(0x8da6_b343)
            ^ (y as u32).wrapping_mul(0xd816_3841)
            ^ (z as u32).wrapping_mul(0xcb1a_b31f),
    );
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

#[inline(always)]
fn fade(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[inline(always)]
fn wrap(i: i32, period: i32) -> i32 {
    if period > 0 { i.rem_euclid(period) } else { i }
}

/// Value noise 2D en [0,1]. Si `period > 0` el ruido es periódico (texturas que se repiten).
pub fn value2(x: f32, y: f32, period: i32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let fx = fade(x - xi);
    let fy = fade(y - yi);
    let (x0, y0) = (xi as i32, yi as i32);
    let (ax, bx) = (wrap(x0, period), wrap(x0 + 1, period));
    let (ay, by) = (wrap(y0, period), wrap(y0 + 1, period));
    let a = hash2(ax, ay);
    let b = hash2(bx, ay);
    let c = hash2(ax, by);
    let d = hash2(bx, by);
    let top = a + (b - a) * fx;
    let bottom = c + (d - c) * fx;
    top + (bottom - top) * fy
}

/// Fractal Brownian motion 2D periódico.
pub fn fbm2(x: f32, y: f32, period: i32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    let mut p = period;
    for _ in 0..octaves {
        sum += value2(x * freq, y * freq, p) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
        if p > 0 {
            p *= 2;
        }
    }
    sum / norm
}

/// Value noise 3D en [0,1].
pub fn value3(p: Vec3) -> f32 {
    let (xi, yi, zi) = (p.x.floor(), p.y.floor(), p.z.floor());
    let (fx, fy, fz) = (fade(p.x - xi), fade(p.y - yi), fade(p.z - zi));
    let (x, y, z) = (xi as i32, yi as i32, zi as i32);
    let c000 = hash3(x, y, z);
    let c100 = hash3(x + 1, y, z);
    let c010 = hash3(x, y + 1, z);
    let c110 = hash3(x + 1, y + 1, z);
    let c001 = hash3(x, y, z + 1);
    let c101 = hash3(x + 1, y, z + 1);
    let c011 = hash3(x, y + 1, z + 1);
    let c111 = hash3(x + 1, y + 1, z + 1);
    let x00 = c000 + (c100 - c000) * fx;
    let x10 = c010 + (c110 - c010) * fx;
    let x01 = c001 + (c101 - c001) * fx;
    let x11 = c011 + (c111 - c011) * fx;
    let y0 = x00 + (x10 - x00) * fy;
    let y1 = x01 + (x11 - x01) * fy;
    y0 + (y1 - y0) * fz
}

/// Value noise 3D con su gradiente analítico (para relieve/bump mapping).
pub fn value3_grad(p: Vec3) -> (f32, Vec3) {
    let (xi, yi, zi) = (p.x.floor(), p.y.floor(), p.z.floor());
    let (fx, fy, fz) = (p.x - xi, p.y - yi, p.z - zi);
    let (u, v, w) = (fade(fx), fade(fy), fade(fz));
    let (du, dv, dw) = (
        6.0 * fx * (1.0 - fx),
        6.0 * fy * (1.0 - fy),
        6.0 * fz * (1.0 - fz),
    );
    let (x, y, z) = (xi as i32, yi as i32, zi as i32);
    let a = hash3(x, y, z);
    let b = hash3(x + 1, y, z);
    let c = hash3(x, y + 1, z);
    let d = hash3(x + 1, y + 1, z);
    let e = hash3(x, y, z + 1);
    let f = hash3(x + 1, y, z + 1);
    let g = hash3(x, y + 1, z + 1);
    let h = hash3(x + 1, y + 1, z + 1);
    let k1 = b - a;
    let k2 = c - a;
    let k3 = e - a;
    let k4 = a - b - c + d;
    let k5 = a - c - e + g;
    let k6 = a - b - e + f;
    let k7 = -a + b + c - d + e - f - g + h;
    let value = a + k1 * u + k2 * v + k3 * w + k4 * u * v + k5 * v * w + k6 * w * u + k7 * u * v * w;
    let grad = Vec3::new(
        du * (k1 + k4 * v + k6 * w + k7 * v * w),
        dv * (k2 + k4 * u + k5 * w + k7 * u * w),
        dw * (k3 + k6 * u + k5 * v + k7 * u * v),
    );
    (value, grad)
}

/// Voronoi 2D periódico: devuelve (distancia al punto más cercano, al segundo, id de celda).
pub fn voronoi2(x: f32, y: f32, period: i32) -> (f32, f32, f32) {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = fract(x);
    let fy = fract(y);
    let mut f1 = 8.0f32;
    let mut f2 = 8.0f32;
    let mut id = 0.0;
    for j in -1..=1 {
        for i in -1..=1 {
            let cx = wrap(xi + i, period);
            let cy = wrap(yi + j, period);
            let px = i as f32 + hash2(cx, cy);
            let py = j as f32 + hash2(cy + 97, cx + 31);
            let dx = px - fx;
            let dy = py - fy;
            let d = (dx * dx + dy * dy).sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
                id = hash2(cx + 13, cy + 71);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    (f1, f2, id)
}
