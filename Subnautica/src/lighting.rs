//! Iluminación precalculada al iniciar:
//! - Mapa de sombras del sol (trazado con rayos una sola vez): sombras suaves baratas.
//! - Volumen de oclusión ambiental: zonas bajo el arco y grietas más oscuras.
//! Y funciones animadas: cáusticas y haces de luz.

use crate::{
    math::{Ray, Vec3},
    scene::Scene,
    texture::par_fill,
};

pub struct ShadowMap {
    size: usize,
    origin: Vec3,
    u: Vec3,
    v: Vec3,
    l: Vec3,
    half: f32,
    depth: Vec<f32>,
}

impl ShadowMap {
    pub fn empty() -> Self {
        Self {
            size: 0,
            origin: Vec3::ZERO,
            u: Vec3::ZERO,
            v: Vec3::ZERO,
            l: Vec3::UP,
            half: 1.0,
            depth: Vec::new(),
        }
    }

    pub fn build(scene: &Scene, center: Vec3, half: f32, size: usize) -> Self {
        let l = scene.sun_dir;
        let u = l.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
        let v = l.cross(u).normalized();
        let origin = center + l * 40.0;
        let depth = par_fill(size, size, |x, y| {
            let a = ((x as f32 + 0.5) / size as f32 * 2.0 - 1.0) * half;
            let b = ((y as f32 + 0.5) / size as f32 * 2.0 - 1.0) * half;
            let ray = Ray::new(origin + u * a + v * b, -l);
            scene.first_opaque(&ray, 120.0).unwrap_or(f32::INFINITY)
        });
        Self {
            size,
            origin,
            u,
            v,
            l,
            half,
            depth,
        }
    }

    #[inline(always)]
    fn coords(&self, p: Vec3) -> (f32, f32, f32) {
        let rel = p - self.origin;
        let s = self.size as f32;
        let x = (rel.dot(self.u) / self.half * 0.5 + 0.5) * s - 0.5;
        let y = (rel.dot(self.v) / self.half * 0.5 + 0.5) * s - 0.5;
        (x, y, -rel.dot(self.l))
    }

    #[inline(always)]
    fn lit(&self, x: i32, y: i32, d: f32) -> f32 {
        let n = self.size as i32;
        if x < 0 || y < 0 || x >= n || y >= n {
            return 1.0;
        }
        if d <= self.depth[(y * n + x) as usize] { 1.0 } else { 0.0 }
    }

    /// Visibilidad del sol con filtro tienda 4x4 (sombra suave).
    #[inline]
    pub fn visibility(&self, p: Vec3) -> f32 {
        if self.size == 0 {
            return 1.0;
        }
        let (x, y, d) = self.coords(p);
        let d = d - 0.05;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let (xi, yi) = (x0 as i32, y0 as i32);
        // Caja de 3x3 texeles combinada con interpolación bilineal (4x4 consultas).
        let wx = [1.0 - fx, 1.0, 1.0, fx];
        let wy = [1.0 - fy, 1.0, 1.0, fy];
        let mut sum = 0.0;
        for j in 0..4 {
            let mut row = 0.0;
            for i in 0..4 {
                row += wx[i] * self.lit(xi - 1 + i as i32, yi - 1 + j as i32, d);
            }
            sum += row * wy[j];
        }
        sum * (1.0 / 9.0)
    }

    /// Filtro bilineal 2x2 (más barato, para el modo interactivo).
    #[inline]
    pub fn visibility_bilinear(&self, p: Vec3) -> f32 {
        if self.size == 0 {
            return 1.0;
        }
        let (x, y, d) = self.coords(p);
        let d = d - 0.05;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let (xi, yi) = (x0 as i32, y0 as i32);
        let a = self.lit(xi, yi, d) + (self.lit(xi + 1, yi, d) - self.lit(xi, yi, d)) * fx;
        let b = self.lit(xi, yi + 1, d) + (self.lit(xi + 1, yi + 1, d) - self.lit(xi, yi + 1, d)) * fx;
        a + (b - a) * fy
    }

    /// Coordenadas (texel x, texel y, profundidad) del punto `o + d*s0` y su incremento por paso `dt`.
    pub fn ray_coords(&self, o: Vec3, d: Vec3, s0: f32, dt: f32) -> ([f32; 3], [f32; 3]) {
        if self.size == 0 {
            return ([0.0; 3], [0.0; 3]);
        }
        let k = 0.5 * self.size as f32 / self.half;
        let (x, y, z) = self.coords(o + d * s0);
        let dx = d.dot(self.u) * k * dt;
        let dy = d.dot(self.v) * k * dt;
        let dz = -d.dot(self.l) * dt;
        ([x, y, z - 0.1], [dx, dy, dz])
    }

    /// Consulta de un solo texel con coordenadas ya calculadas.
    #[inline(always)]
    pub fn lit_at(&self, x: f32, y: f32, d: f32) -> f32 {
        if self.size == 0 {
            return 1.0;
        }
        self.lit((x + 0.5) as i32 - (x < -0.5) as i32, (y + 0.5) as i32 - (y < -0.5) as i32, d)
    }
}

pub struct AoVolume {
    min: Vec3,
    cell: f32,
    nx: usize,
    ny: usize,
    nz: usize,
    data: Vec<f32>,
}

impl AoVolume {
    pub fn empty() -> Self {
        Self {
            min: Vec3::ZERO,
            cell: 1.0,
            nx: 0,
            ny: 0,
            nz: 0,
            data: Vec::new(),
        }
    }

    pub fn build(scene: &Scene, min: Vec3, max: Vec3, cell: f32, rays: usize, radius: f32) -> Self {
        let nx = ((max.x - min.x) / cell).ceil() as usize + 1;
        let ny = ((max.y - min.y) / cell).ceil() as usize + 1;
        let nz = ((max.z - min.z) / cell).ceil() as usize + 1;
        // Direcciones en espiral de Fibonacci sobre la esfera.
        let golden = std::f32::consts::PI * (3.0 - 5.0f32.sqrt());
        let dirs: Vec<(Vec3, f32)> = (0..rays)
            .map(|i| {
                let y = 1.0 - (i as f32 + 0.5) / rays as f32 * 2.0;
                let r = (1.0 - y * y).sqrt();
                let a = golden * i as f32;
                let d = Vec3::new(a.cos() * r, y, a.sin() * r);
                (d, 0.2 + d.y.max(0.0))
            })
            .collect();
        let wsum: f32 = dirs.iter().map(|d| d.1).sum();
        let data = par_fill(nx * nz, ny, |xz, y| {
            let x = xz % nx;
            let z = xz / nx;
            let p = min + Vec3::new(x as f32, y as f32, z as f32) * cell;
            let mut free = 0.0;
            for (d, w) in &dirs {
                if !scene.occluded(&Ray::new(p, *d), radius) {
                    free += w;
                }
            }
            free / wsum
        });
        Self {
            min,
            cell,
            nx,
            ny,
            nz,
            data,
        }
    }

    #[inline]
    pub fn sample(&self, p: Vec3) -> f32 {
        if self.data.is_empty() {
            return 1.0;
        }
        let q = (p - self.min) * (1.0 / self.cell);
        let x = q.x.clamp(0.0, self.nx as f32 - 1.001);
        let y = q.y.clamp(0.0, self.ny as f32 - 1.001);
        let z = q.z.clamp(0.0, self.nz as f32 - 1.001);
        let (xi, yi, zi) = (x as usize, y as usize, z as usize);
        let (fx, fy, fz) = (x - xi as f32, y - yi as f32, z - zi as f32);
        // Índice: fila = y, columna = x + z*nx.
        let row = self.nx * self.nz;
        let at = |xx: usize, yy: usize, zz: usize| self.data[yy * row + zz * self.nx + xx];
        let c00 = at(xi, yi, zi) + (at(xi + 1, yi, zi) - at(xi, yi, zi)) * fx;
        let c10 = at(xi, yi + 1, zi) + (at(xi + 1, yi + 1, zi) - at(xi, yi + 1, zi)) * fx;
        let c01 = at(xi, yi, zi + 1) + (at(xi + 1, yi, zi + 1) - at(xi, yi, zi + 1)) * fx;
        let c11 = at(xi, yi + 1, zi + 1) + (at(xi + 1, yi + 1, zi + 1) - at(xi, yi + 1, zi + 1)) * fx;
        let c0 = c00 + (c10 - c00) * fy;
        let c1 = c01 + (c11 - c01) * fy;
        c0 + (c1 - c0) * fz
    }
}

/// Punto de la superficie del agua por donde entró la luz del sol que llega a `p`.
#[inline(always)]
pub fn surface_entry(scene: &Scene, p: Vec3) -> (f32, f32, f32) {
    let depth = (scene.water_y - p.y).max(0.0);
    let k = depth / scene.sun_dir.y;
    (p.x + scene.sun_dir.x * k, p.z + scene.sun_dir.z * k, depth)
}

/// Cáusticas animadas: dos capas de una red de Voronoi que se desplazan.
#[inline]
pub fn caustic(scene: &Scene, p: Vec3, time: f32) -> f32 {
    let (qx, qz, depth) = surface_entry(scene, p);
    let u = qx * 0.21;
    let v = qz * 0.21;
    let c1 = scene.caustics.sample(u + time * 0.021, v + time * 0.013);
    let c2 = scene
        .caustics
        .sample(u * 1.37 - time * 0.016 + 0.37, v * 1.37 + time * 0.019 + 0.11);
    let c = c1 * c2;
    let k = 0.75 * (-depth * 0.03).exp();
    (1.0 - k) + k * c * 4.0
}
