//! Texturas en memoria (generadas por código), muestreo bilineal y cubemaps.

use crate::math::Vec3;

/// Llena un buffer `w*h` en paralelo usando hilos de la librería estándar.
pub fn par_fill<T: Send + Clone + Default>(
    w: usize,
    h: usize,
    f: impl Fn(usize, usize) -> T + Sync,
) -> Vec<T> {
    let mut data = vec![T::default(); w * h];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows_per = h.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (chunk_index, chunk) in data.chunks_mut(rows_per * w).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (i, texel) in chunk.iter_mut().enumerate() {
                    let y = chunk_index * rows_per + i / w;
                    *texel = f(i % w, y);
                }
            });
        }
    });
    data
}

/// Textura RGB (lineal) con tamaño potencia de dos y repetición.
pub struct Texture {
    w: usize,
    h: usize,
    data: Vec<Vec3>,
}

impl Texture {
    /// `f(u, v)` recibe coordenadas en [0,1).
    pub fn generate(size: usize, f: impl Fn(f32, f32) -> Vec3 + Sync) -> Self {
        assert!(size.is_power_of_two());
        let inv = 1.0 / size as f32;
        let data = par_fill(size, size, |x, y| {
            f((x as f32 + 0.5) * inv, (y as f32 + 0.5) * inv)
        });
        Self { w: size, h: size, data }
    }

    #[inline(always)]
    pub fn sample(&self, u: f32, v: f32) -> Vec3 {
        let x = u * self.w as f32 - 0.5;
        let y = v * self.h as f32 - 0.5;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let mw = self.w as i32 - 1;
        let mh = self.h as i32 - 1;
        let xa = (x0 as i32 & mw) as usize;
        let xb = ((x0 as i32 + 1) & mw) as usize;
        let ya = (y0 as i32 & mh) as usize * self.w;
        let yb = ((y0 as i32 + 1) & mh) as usize * self.w;
        let a = self.data[ya + xa];
        let b = self.data[ya + xb];
        let c = self.data[yb + xa];
        let d = self.data[yb + xb];
        let top = a + (b - a) * fx;
        let bottom = c + (d - c) * fx;
        top + (bottom - top) * fy
    }

    /// Muestreo sin repetición (para las caras del cubemap).
    #[inline(always)]
    pub fn sample_clamped(&self, u: f32, v: f32) -> Vec3 {
        let x = (u * self.w as f32 - 0.5).clamp(0.0, self.w as f32 - 1.001);
        let y = (v * self.h as f32 - 0.5).clamp(0.0, self.h as f32 - 1.001);
        let xa = x as usize;
        let ya = y as usize;
        let fx = x - xa as f32;
        let fy = y - ya as f32;
        let a = self.data[ya * self.w + xa];
        let b = self.data[ya * self.w + xa + 1];
        let c = self.data[(ya + 1) * self.w + xa];
        let d = self.data[(ya + 1) * self.w + xa + 1];
        let top = a + (b - a) * fx;
        let bottom = c + (d - c) * fx;
        top + (bottom - top) * fy
    }

    pub fn size(&self) -> (usize, usize) {
        (self.w, self.h)
    }

    pub fn texel(&self, x: usize, y: usize) -> Vec3 {
        self.data[y * self.w + x]
    }
}

/// Textura de un canal (cáusticas, máscaras).
pub struct Gray {
    size: usize,
    data: Vec<f32>,
}

impl Gray {
    pub fn generate(size: usize, f: impl Fn(f32, f32) -> f32 + Sync) -> Self {
        assert!(size.is_power_of_two());
        let inv = 1.0 / size as f32;
        let data = par_fill(size, size, |x, y| {
            f((x as f32 + 0.5) * inv, (y as f32 + 0.5) * inv)
        });
        Self { size, data }
    }

    /// Escala los valores para que el promedio sea `target`.
    pub fn with_mean(mut self, target: f32) -> Self {
        let mean = self.data.iter().sum::<f32>() / self.data.len() as f32;
        let k = target / mean.max(1e-6);
        for v in &mut self.data {
            *v *= k;
        }
        self
    }

    /// Desenfoque de caja separable con repetición (para los rayos de luz).
    pub fn blurred(&self, radius: i32) -> Self {
        let n = self.size as i32;
        let m = n - 1;
        let norm = 1.0 / (2 * radius + 1) as f32;
        let mut tmp = vec![0.0; self.data.len()];
        for y in 0..n {
            for x in 0..n {
                let mut acc = 0.0;
                for k in -radius..=radius {
                    acc += self.data[(y * n + ((x + k) & m)) as usize];
                }
                tmp[(y * n + x) as usize] = acc * norm;
            }
        }
        let mut out = vec![0.0; self.data.len()];
        for y in 0..n {
            for x in 0..n {
                let mut acc = 0.0;
                for k in -radius..=radius {
                    acc += tmp[(((y + k) & m) * n + x) as usize];
                }
                out[(y * n + x) as usize] = acc * norm;
            }
        }
        Self {
            size: self.size,
            data: out,
        }
    }

    #[inline(always)]
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let s = self.size as f32;
        let x = u * s - 0.5;
        let y = v * s - 0.5;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let m = self.size as i32 - 1;
        let xa = (x0 as i32 & m) as usize;
        let xb = ((x0 as i32 + 1) & m) as usize;
        let ya = (y0 as i32 & m) as usize * self.size;
        let yb = ((y0 as i32 + 1) & m) as usize * self.size;
        let a = self.data[ya + xa];
        let b = self.data[ya + xb];
        let c = self.data[yb + xa];
        let d = self.data[yb + xb];
        let top = a + (b - a) * fx;
        let bottom = c + (d - c) * fx;
        top + (bottom - top) * fy
    }
}

/// Cubemap de 6 caras: +X, -X, +Y, -Y, +Z, -Z.
pub struct CubeMap {
    faces: Vec<Texture>,
}

impl CubeMap {
    /// Genera cada cara evaluando `f(dirección)` para cada texel.
    pub fn generate(size: usize, f: impl Fn(Vec3) -> Vec3 + Sync) -> Self {
        let faces = (0..6)
            .map(|face| {
                Texture::generate(size, |u, v| {
                    let a = u * 2.0 - 1.0;
                    let b = v * 2.0 - 1.0;
                    f(face_direction(face, a, b).normalized())
                })
            })
            .collect();
        Self { faces }
    }

    #[inline(always)]
    pub fn sample(&self, d: Vec3) -> Vec3 {
        let ax = d.x.abs();
        let ay = d.y.abs();
        let az = d.z.abs();
        let (face, a, b, m) = if ax >= ay && ax >= az {
            if d.x > 0.0 { (0, -d.z, -d.y, ax) } else { (1, d.z, -d.y, ax) }
        } else if ay >= az {
            if d.y > 0.0 { (2, d.x, d.z, ay) } else { (3, d.x, -d.z, ay) }
        } else if d.z > 0.0 {
            (4, d.x, -d.y, az)
        } else {
            (5, -d.x, -d.y, az)
        };
        let inv = 0.5 / m;
        self.faces[face].sample_clamped(a * inv + 0.5, b * inv + 0.5)
    }

    pub fn face(&self, i: usize) -> &Texture {
        &self.faces[i]
    }
}

/// Dirección (no normalizada) para la coordenada (a,b) ∈ [-1,1]² de una cara.
fn face_direction(face: usize, a: f32, b: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -b, -a),
        1 => Vec3::new(-1.0, -b, a),
        2 => Vec3::new(a, 1.0, b),
        3 => Vec3::new(a, -1.0, -b),
        4 => Vec3::new(a, -b, 1.0),
        _ => Vec3::new(-a, -b, -1.0),
    }
}
