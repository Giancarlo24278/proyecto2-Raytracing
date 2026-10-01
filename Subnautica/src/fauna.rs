//! Animales: peepers (con el ojo luminoso) y bladderfish, hechos con figuras simples.
//!
//! Cada especie es una plantilla de primitivas en su propio espacio local
//! (x = hacia adelante, y = arriba, z = costado). En cada cuadro solo se calcula la
//! posición y orientación de cada animal; el rayo se lleva al espacio local del
//! animal y se prueba contra la plantilla (no hace falta reconstruir el BVH).

use crate::{
    material::{
        BEAK, BLADDER, BLADDER_SPINE, EYE_WHITE, JELLY_BELL, JELLY_RIBBON, JELLY_TENTACLE, PEEPER_BODY, PEEPER_EYE,
        MOUTH, PEEPER_FIN, REAPER_RED, REAPER_SKIN, STALKER_PURPLE, STALKER_SKIN, TEETH,
    },
    math::{Mat3, Vec3},
    primitives::{Cutout, Prim, Shape},
};
use std::f32::consts::{PI, TAU};

/// Fuente de luz puntual que además dibuja un halo en el agua.
#[derive(Clone, Copy, Debug)]
pub struct Glow {
    pub pos: Vec3,
    /// Color * intensidad.
    pub color: Vec3,
    /// Alcance de la luz en metros.
    pub radius: f32,
    /// Tamaño del halo visible alrededor de la fuente.
    pub halo: f32,
}

pub struct Species {
    pub prims: Vec<Prim>,
    pub radius: f32,
    /// Velocidad del aleteo (multiplica el tiempo del vaivén de la cola).
    pub wag: f32,
    /// Luces que lleva el animal (posición local, color, alcance, halo).
    pub lights: Vec<(Vec3, Vec3, f32, f32)>,
    /// Flota en vertical (medusas) en vez de apuntar hacia donde nada.
    pub upright: bool,
}

/// Recorrido elíptico con ondulación vertical.
#[derive(Clone, Copy)]
pub struct Path {
    pub center: Vec3,
    pub rx: f32,
    pub rz: f32,
    pub bob: f32,
    /// Radianes por segundo (negativo = sentido contrario).
    pub speed: f32,
    pub phase: f32,
}

impl Path {
    fn at(&self, t: f32) -> (Vec3, Vec3) {
        let a = self.speed * t + self.phase;
        let pos = self.center
            + Vec3::new(self.rx * a.cos(), self.bob * (a * 1.7).sin(), self.rz * a.sin());
        let vel = Vec3::new(
            -self.rx * a.sin(),
            self.bob * 1.7 * (a * 1.7).cos(),
            self.rz * a.cos(),
        ) * self.speed;
        (pos, vel.normalized())
    }
}

pub struct Swimmer {
    pub species: usize,
    pub path: Path,
    /// Posición dentro del cardumen (x adelante, y arriba, z costado).
    pub offset: Vec3,
    pub phase: f32,
}

/// Animal colocado en un cuadro concreto.
#[derive(Clone, Copy)]
pub struct Creature {
    pub species: u16,
    pub pos: Vec3,
    pub rot: Mat3,
    pub time: f32,
}

/// Base ortonormal con el eje x local apuntando en `forward`.
pub fn basis_x(forward: Vec3) -> Mat3 {
    let x = forward.normalized();
    let helper = if x.y.abs() < 0.95 { Vec3::UP } else { Vec3::new(0.0, 0.0, 1.0) };
    let z = x.cross(helper).normalized();
    let y = z.cross(x).normalized();
    Mat3 { c0: x, c1: y, c2: z }
}

impl Swimmer {
    pub fn place(&self, species: &[Species], t: f32) -> Creature {
        let (p, fwd) = self.path.at(t);
        let sp = &species[self.species];
        if sp.upright {
            // Medusa: sube y baja despacio y gira un poco sobre sí misma.
            return Creature {
                species: self.species as u16,
                pos: p,
                rot: Mat3::from_euler(t * 0.15 + self.phase, (t * 0.5 + self.phase).sin() * 0.06, 0.0),
                time: t * sp.wag + self.phase * 3.0,
            };
        }
        let frame = basis_x(fwd);
        // Pequeño serpenteo individual.
        let wob = Vec3::new(
            0.0,
            (t * 1.1 + self.phase).sin() * 0.12,
            (t * 0.8 + self.phase * 1.3).sin() * 0.15,
        );
        let pos = p + frame.mul(self.offset + wob);
        // Leve inclinación al girar.
        let roll = (t * 0.9 + self.phase).sin() * 0.12;
        let rot = frame.mul_mat(Mat3::from_euler(0.0, roll, 0.0));
        Creature {
            species: self.species as u16,
            pos,
            rot,
            time: t * sp.wag + self.phase * 3.0,
        }
    }
}

fn part(shape: Shape, c: Vec3, half: Vec3, rot: Mat3, mat: u16) -> Prim {
    Prim::new(shape, c, half, rot, mat)
}

/// Peeper: cuerpo azul oscuro, ojo enorme amarillo que brilla, pico naranja y aletas.
pub fn peeper() -> Species {
    let s = 1.3;
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * s;
    let mut prims = vec![
        part(Shape::Sphere, v(0.0, 0.0, 0.0), v(0.17, 0.15, 0.12), Mat3::IDENTITY, PEEPER_BODY),
        part(Shape::Sphere, v(-0.19, 0.0, 0.0), v(0.22, 0.085, 0.075), Mat3::IDENTITY, PEEPER_BODY),
        // Pico.
        part(Shape::Sphere, v(0.17, -0.005, 0.0), v(0.05, 0.03, 0.03), Mat3::IDENTITY, BEAK),
        // Aleta dorsal y ventral (curvas hacia atrás).
        part(
            Shape::Cube,
            v(-0.06, 0.20, 0.0),
            v(0.06, 0.13, 0.010),
            Mat3::from_euler(0.0, 0.0, 0.7),
            PEEPER_FIN,
        )
        .with_cutout(Cutout::Leaf, [0.0, 1.0]),
        part(
            Shape::Cube,
            v(-0.06, -0.19, 0.0),
            v(0.06, 0.12, 0.010),
            Mat3::from_euler(0.0, 0.0, PI - 0.7),
            PEEPER_FIN,
        )
        .with_cutout(Cutout::Leaf, [0.0, 1.0]),
    ];
    // Ojos: un disco a cada lado.
    for side in [1.0f32, -1.0] {
        prims.push(part(
            Shape::Cylinder,
            v(0.03, 0.0, 0.115 * side),
            v(0.105, 0.014, 0.105),
            Mat3::from_y_axis(Vec3::new(0.0, 0.0, side), 0.0),
            PEEPER_EYE,
        ));
    }
    // Cola en abanico que se mueve de lado a lado.
    // Cuadrado delgado con recorte en forma de cola (eje y local = hacia atrás).
    let tail_rot = Mat3 {
        c0: Vec3::new(0.0, 1.0, 0.0),
        c1: Vec3::new(-1.0, 0.0, 0.0),
        c2: Vec3::new(0.0, 0.0, -1.0),
    };
    let tail = part(Shape::Cube, v(-0.50, 0.0, 0.0), v(0.17, 0.12, 0.008), tail_rot, PEEPER_FIN)
        .with_cutout(Cutout::Tail, [0.0, 1.0])
        .with_sway(Vec3::new(0.0, 0.0, 1.0), 0.06 * s, 0.0);
    prims.push(tail);
    Species {
        prims,
        radius: 0.62 * s,
        wag: 5.0,
        lights: vec![(v(0.03, 0.0, 0.0), Vec3::new(1.0, 0.72, 0.18) * 0.55, 2.0, 0.24)],
        upright: false,
    }
}

/// Bladderfish simplificado: vejiga lila transparente, espina con manchas y un ojo.
pub fn bladderfish() -> Species {
    let mut prims = vec![
        part(Shape::Sphere, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.27, 0.10, 0.25), Mat3::IDENTITY, BLADDER),
        part(
            Shape::Cylinder,
            Vec3::new(0.02, 0.09, 0.0),
            Vec3::new(0.028, 0.32, 0.028),
            Mat3::from_y_axis(Vec3::new(1.0, 0.0, 0.0), 0.0),
            BLADDER_SPINE,
        ),
        part(Shape::Sphere, Vec3::new(0.33, 0.10, 0.0), Vec3::new(0.075, 0.06, 0.06), Mat3::IDENTITY, BEAK),
    ];
    for side in [1.0f32, -1.0] {
        prims.push(part(
            Shape::Sphere,
            Vec3::new(0.35, 0.12, 0.045 * side),
            Vec3::splat(0.035),
            basis_x(Vec3::new(0.6, 0.2, side)),
            EYE_WHITE,
        ));
    }
    // Pequeños tubos laterales (como las "patitas" del juego).
    for (x, side) in [(0.12f32, 1.0f32), (-0.12, 1.0), (0.12, -1.0), (-0.12, -1.0)] {
        let dir = Vec3::new(x * 1.5, -0.4, side).normalized();
        prims.push(
            part(
                Shape::Cylinder,
                Vec3::new(x, -0.02, 0.24 * side) + dir * 0.06,
                Vec3::new(0.012, 0.07, 0.012),
                Mat3::from_y_axis(dir, 0.0),
                BLADDER_SPINE,
            )
            .with_sway(Vec3::new(1.0, 0.0, 0.0), 0.03, x * 3.0),
        );
    }
    Species {
        prims,
        radius: 0.5,
        wag: 2.0,
        lights: Vec::new(),
        upright: false,
    }
}

/// Medusa alargada simplificada: campana, tentáculos finos y una cinta en espiral.
pub fn jellyfish() -> Species {
    let mut prims = vec![
        part(Shape::Sphere, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.27, 0.32, 0.27), Mat3::IDENTITY, JELLY_BELL),
        // Tallo central.
        part(Shape::Cylinder, Vec3::new(0.0, -0.62, 0.0), Vec3::new(0.03, 0.5, 0.03), Mat3::IDENTITY, JELLY_BELL)
            .with_cutout(Cutout::None, [1.0, 0.0])
            .with_sway(Vec3::new(1.0, 0.0, 0.3), 0.10, 0.0),
    ];
    // Cuatro tentáculos y una cinta corta en espiral (versión simple).
    for k in 0..4 {
        let a = k as f32 * TAU / 4.0 + 0.4;
        let rim = Vec3::new(a.cos() * 0.2, -0.14, a.sin() * 0.2);
        let len = 1.0 + 0.15 * (k % 2) as f32;
        prims.push(
            part(
                Shape::Cylinder,
                rim + Vec3::new(0.0, -len * 0.5, 0.0),
                Vec3::new(0.014, len * 0.5, 0.014),
                Mat3::IDENTITY,
                JELLY_TENTACLE,
            )
            .with_cutout(Cutout::None, [1.0, 0.0])
            .with_sway(Vec3::new(a.cos(), 0.0, a.sin() + 0.5), 0.12, k as f32),
        );
    }
    let n = 6;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let a = t * TAU * 1.5;
        let radial = Vec3::new(a.cos(), 0.0, a.sin());
        let rot = Mat3 { c0: radial, c1: Vec3::UP, c2: radial.cross(Vec3::UP) };
        let width = 0.09 * (1.0 - 0.4 * t);
        prims.push(
            part(
                Shape::Cube,
                radial * (width * 0.9) + Vec3::new(0.0, -0.32 - t * 1.0, 0.0),
                Vec3::new(width, 0.09, 0.01),
                rot,
                JELLY_RIBBON,
            )
            .with_cutout(Cutout::None, [t, t + 1.0 / n as f32])
            .with_sway(Vec3::new(1.0, 0.0, 0.3), 0.10, 0.0),
        );
    }
    Species {
        prims,
        radius: 1.9,
        wag: 1.2,
        lights: vec![(Vec3::new(0.0, -0.1, 0.0), Vec3::new(0.62, 0.30, 1.0) * 0.45, 2.6, 0.5)],
        upright: true,
    }
}

/// Stalker simplificado: cuerpo alargado, hocico largo con dientes y placa morada,
/// espinas moradas en el lomo, aletas-patas y cola.
pub fn stalker() -> Species {
    let mut prims = vec![
        part(Shape::Sphere, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.9, 0.42, 0.38), Mat3::IDENTITY, STALKER_SKIN),
        part(Shape::Sphere, Vec3::new(0.95, 0.06, 0.0), Vec3::new(0.55, 0.26, 0.23), Mat3::IDENTITY, STALKER_SKIN),
        part(Shape::Cube, Vec3::new(1.8, 0.0, 0.0), Vec3::new(0.55, 0.07, 0.085), Mat3::IDENTITY, STALKER_SKIN),
        // Placa morada en la punta del hocico.
        part(Shape::Cube, Vec3::new(2.36, -0.04, 0.0), Vec3::new(0.06, 0.3, 0.09), Mat3::IDENTITY, STALKER_PURPLE),
        // Cola.
        part(Shape::Sphere, Vec3::new(-1.15, -0.02, 0.0), Vec3::new(0.75, 0.2, 0.17), Mat3::IDENTITY, STALKER_SKIN),
    ];
    // Ojos.
    for side in [1.0f32, -1.0] {
        prims.push(part(Shape::Sphere, Vec3::new(1.05, 0.16, 0.17 * side), Vec3::splat(0.045), Mat3::IDENTITY, BEAK));
    }
    // Dientes a los lados del hocico.
    for i in 0..5 {
        for side in [1.0f32, -1.0] {
            let x = 1.45 + i as f32 * 0.19;
            let dir = Vec3::new(0.2, -1.0, 0.45 * side).normalized();
            prims.push(part(
                Shape::Cube,
                Vec3::new(x, -0.08, 0.09 * side) + dir * 0.05,
                Vec3::new(0.014, 0.07 - i as f32 * 0.006, 0.014),
                Mat3::from_y_axis(dir, 0.0),
                TEETH,
            ));
        }
    }
    // Espinas moradas del lomo (cada vez más pequeñas hacia la cola).
    for i in 0..6 {
        let x = 0.75 - i as f32 * 0.32;
        let h = 0.45 - i as f32 * 0.05;
        let body_top = if x > -0.8 { 0.36 } else { 0.18 };
        prims.push(part(
            Shape::Cube,
            Vec3::new(x, body_top + h * 0.45, 0.0),
            Vec3::new(0.06, h * 0.5, 0.03),
            Mat3::from_euler(0.0, 0.0, 0.18),
            STALKER_PURPLE,
        ));
    }
    // Aletas-patas delanteras y traseras.
    for side in [1.0f32, -1.0] {
        let front = Vec3::new(0.35, -1.0, 0.35 * side).normalized();
        prims.push(
            part(Shape::Sphere, Vec3::new(0.45, -0.3, 0.2 * side) + front * 0.4, Vec3::new(0.11, 0.45, 0.09), Mat3::from_y_axis(front, 0.0), STALKER_SKIN)
                .with_cutout(Cutout::None, [0.0, 1.0])
                .with_sway(Vec3::new(1.0, 0.0, 0.0), 0.06, side),
        );
        let back = Vec3::new(-0.6, -1.0, 0.4 * side).normalized();
        prims.push(
            part(Shape::Sphere, Vec3::new(-0.55, -0.2, 0.2 * side) + back * 0.25, Vec3::new(0.08, 0.28, 0.07), Mat3::from_y_axis(back, 0.0), STALKER_SKIN),
        );
    }
    // Aleta de la cola.
    let tail_rot = Mat3 {
        c0: Vec3::new(0.0, 1.0, 0.0),
        c1: Vec3::new(-1.0, 0.0, 0.0),
        c2: Vec3::new(0.0, 0.0, -1.0),
    };
    prims.push(
        part(Shape::Cube, Vec3::new(-2.15, 0.0, 0.0), Vec3::new(0.3, 0.28, 0.012), tail_rot, STALKER_PURPLE)
            .with_cutout(Cutout::Tail, [0.4, 1.0])
            .with_sway(Vec3::new(0.0, 0.0, 1.0), 0.12, 0.0),
    );
    // Un poco más pequeño que antes.
    let k = 0.78;
    Species {
        prims: prims.into_iter().map(|p| p.scaled(k)).collect(),
        radius: 2.7 * k,
        wag: 2.5,
        lights: Vec::new(),
        upright: false,
    }
}

/// Centro del círculo que patrulla el stalker (encima de los restos metálicos).
pub const STALKER_HOME: Vec3 = Vec3::new(-9.6, 3.0, -11.2);

/// Leviatán: cabeza con cresta roja, mandíbulas en forma de brazos, cuatro ojos,
/// boca con dientes y un cuerpo largo que ondula al nadar.
pub fn leviathan() -> Species {
    let mut prims = Vec::new();
    // Cabeza puntiaguda: cráneo alargado y un hocico largo que termina en punta.
    prims.push(part(Shape::Sphere, Vec3::new(0.0, 0.1, 0.0), Vec3::new(1.1, 0.6, 0.55), Mat3::IDENTITY, REAPER_SKIN));
    prims.push(part(
        Shape::Sphere,
        Vec3::new(1.25, 0.12, 0.0),
        Vec3::new(0.32, 1.05, 0.36),
        Mat3::from_y_axis(Vec3::new(1.0, 0.05, 0.0), 0.0),
        REAPER_SKIN,
    ));
    // Cuerno rojo que sale de la frente hacia adelante (base gruesa y punta fina).
    prims.push(part(
        Shape::Sphere,
        Vec3::new(1.2, 0.5, 0.0),
        Vec3::new(0.14, 0.8, 0.16),
        Mat3::from_y_axis(Vec3::new(1.0, 0.2, 0.0), 0.0),
        REAPER_RED,
    ));
    prims.push(part(
        Shape::Sphere,
        Vec3::new(2.1, 0.66, 0.0),
        Vec3::new(0.07, 0.6, 0.08),
        Mat3::from_y_axis(Vec3::new(1.0, 0.12, 0.0), 0.0),
        REAPER_RED,
    ));
    // Cresta roja hacia atrás.
    prims.push(part(
        Shape::Sphere,
        Vec3::new(-0.7, 0.85, 0.0),
        Vec3::new(0.15, 0.8, 0.22),
        Mat3::from_y_axis(Vec3::new(-0.8, 1.0, 0.0), 0.0),
        REAPER_RED,
    ));
    // Mandíbula inferior larga y en punta, con la barbilla roja.
    prims.push(part(
        Shape::Sphere,
        Vec3::new(1.05, -0.42, 0.0),
        Vec3::new(0.26, 1.05, 0.3),
        Mat3::from_y_axis(Vec3::new(1.0, -0.12, 0.0), 0.0),
        REAPER_SKIN,
    ));
    prims.push(part(
        Shape::Sphere,
        Vec3::new(2.05, -0.55, 0.0),
        Vec3::new(0.06, 0.28, 0.07),
        Mat3::from_y_axis(Vec3::new(1.0, -0.1, 0.0), 0.0),
        REAPER_RED,
    ));
    // Boca casi "humana": una abertura ancha entre labios, con dos filas de dientes.
    prims.push(part(Shape::Sphere, Vec3::new(1.45, -0.17, 0.0), Vec3::new(0.4, 0.1, 0.31), Mat3::IDENTITY, MOUTH));
    prims.push(part(Shape::Sphere, Vec3::new(1.47, -0.04, 0.0), Vec3::new(0.4, 0.055, 0.33), Mat3::IDENTITY, REAPER_RED));
    prims.push(part(Shape::Sphere, Vec3::new(1.45, -0.3, 0.0), Vec3::new(0.4, 0.055, 0.31), Mat3::IDENTITY, REAPER_RED));
    for i in 0..9 {
        let z = (i as f32 / 8.0 - 0.5) * 0.48;
        let back = 0.35 * (z / 0.3).powi(2);
        prims.push(part(Shape::Cube, Vec3::new(1.8 - back, -0.1, z), Vec3::new(0.025, 0.05, 0.022), Mat3::IDENTITY, TEETH));
        prims.push(part(Shape::Cube, Vec3::new(1.78 - back, -0.24, z * 0.95), Vec3::new(0.025, 0.045, 0.022), Mat3::IDENTITY, TEETH));
    }
    // Cuatro ojos: dos por lado, sobre el cráneo.
    for side in [1.0f32, -1.0] {
        for (x, y, z) in [(0.72, 0.3, 0.4), (0.36, 0.38, 0.47)] {
            prims.push(
                part(Shape::Sphere, Vec3::new(x, y, z * side), Vec3::splat(0.09), Mat3::IDENTITY, BEAK)
                    .with_color(Vec3::splat(0.08)),
            );
        }
    }
    // Mandíbulas: dos "brazos" rojos por lado que salen hacia adelante y se curvan.
    for side in [1.0f32, -1.0] {
        for up in [0.35f32, -0.45] {
            let base = Vec3::new(0.4, up * 0.6, 0.55 * side);
            let d1 = Vec3::new(0.75, up, 0.65 * side).normalized();
            let l1 = 1.5;
            let mid = base + d1 * l1;
            prims.push(part(Shape::Sphere, base + d1 * (l1 * 0.5), Vec3::new(0.17, l1 * 0.55, 0.15), Mat3::from_y_axis(d1, 0.0), REAPER_RED));
            let d2 = Vec3::new(0.7, up * 0.3, -0.75 * side).normalized();
            let l2 = 1.0;
            prims.push(part(Shape::Sphere, mid + d2 * (l2 * 0.5), Vec3::new(0.12, l2 * 0.55, 0.11), Mat3::from_y_axis(d2, 0.0), REAPER_RED));
            // Garra oscura en la punta.
            prims.push(
                part(Shape::Sphere, mid + d2 * l2, Vec3::new(0.06, 0.22, 0.06), Mat3::from_y_axis(d2 + Vec3::new(-0.6, 0.0, 0.0), 0.0), BEAK)
                    .with_color(Vec3::splat(0.15)),
            );
        }
    }
    // Cuerpo: segmentos que se afinan hacia la cola; el vaivén sube en amplitud
    // hacia atrás y forma una onda que recorre el cuerpo.
    let back = Mat3::from_y_axis(Vec3::new(-1.0, 0.0, 0.0), 0.0);
    let segments = 12;
    let total = 10.5;
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let x = -0.6 - (t0 + t1) * 0.5 * total;
        let thick = 0.82 * (1.0 - 0.72 * t0) + 0.12;
        let len = total / segments as f32 * 1.3;
        prims.push(
            part(Shape::Sphere, Vec3::new(x, -0.05, 0.0), Vec3::new(thick * 0.85, len, thick), back, REAPER_SKIN)
                .with_cutout(Cutout::None, [t0, t1])
                .with_sway(Vec3::new(0.0, 0.0, 1.0), SNAKE_AMP, 0.0)
                .with_wave(SNAKE_K),
        );
        // Franja roja del lomo.
        prims.push(
            part(Shape::Sphere, Vec3::new(x, -0.05 + thick * 0.62, 0.0), Vec3::new(thick * 0.28, len * 0.85, thick * 0.25), back, REAPER_RED)
                .with_cutout(Cutout::None, [t0, t1])
                .with_sway(Vec3::new(0.0, 0.0, 1.0), SNAKE_AMP, 0.0)
                .with_wave(SNAKE_K),
        );
    }
    // Aletas laterales delanteras y traseras.
    for side in [1.0f32, -1.0] {
        for (x, size, h) in [(-2.0f32, 1.0f32, 0.15f32), (-8.9, 0.6, 0.78)] {
            let dir = Vec3::new(-0.75, -0.35, 0.7 * side).normalized();
            prims.push(
                part(Shape::Sphere, Vec3::new(x, -0.25, 0.45 * side * (1.0 - h * 0.6)) + dir * (size * 0.6), Vec3::new(0.05, size * 0.7, 0.2 * size), Mat3::from_y_axis(dir, 0.0), REAPER_RED)
                    .with_cutout(Cutout::None, [h, h + 0.02])
                    .with_sway(Vec3::new(0.0, 0.0, 1.0), SNAKE_AMP, 0.0)
                    .with_wave(SNAKE_K),
            );
        }
    }
    // Aleta de la cola en media luna.
    let tail_rot = Mat3 {
        c0: Vec3::new(0.0, 1.0, 0.0),
        c1: Vec3::new(-1.0, 0.0, 0.0),
        c2: Vec3::new(0.0, 0.0, -1.0),
    };
    prims.push(
        part(Shape::Cube, Vec3::new(-0.6 - total - 0.6, -0.05, 0.0), Vec3::new(1.2, 0.7, 0.03), tail_rot, REAPER_RED)
            .with_cutout(Cutout::Tail, [0.97, 1.1])
            .with_sway(Vec3::new(0.0, 0.0, 1.0), SNAKE_AMP, 0.0)
            .with_wave(SNAKE_K),
    );
    // Centrar la plantilla para que la esfera envolvente sea pequeña.
    let shift = Vec3::new(5.3, 0.0, 0.0);
    for p in &mut prims {
        p.center += shift;
    }
    Species {
        prims,
        radius: 9.0,
        wag: 2.2,
        lights: Vec::new(),
        upright: false,
    }
}

/// Amplitud (m) y número de onda del nado de serpiente del leviatán.
const SNAKE_AMP: f32 = 1.3;
const SNAKE_K: f32 = 7.0;

/// Índice de la especie del leviatán.
pub const LEVIATHAN: usize = 4;
/// Duración total del evento (segundos).
pub const EVENT_LENGTH: f32 = 17.0;
/// Momento en que empieza a aparecer y momento en que pasa por el centro.
pub const EVENT_APPEAR: f32 = 6.0;
pub const EVENT_CENTER: f32 = 10.2;
const EVENT_SPEED: f32 = 12.0;

/// Posición del leviatán durante el evento (None mientras no se ve).
pub fn leviathan_pose(e: f32) -> Option<Creature> {
    if !(EVENT_APPEAR..EVENT_LENGTH - 1.0).contains(&e) {
        return None;
    }
    let dir = Vec3::new(1.0, -0.03, -0.38).normalized();
    let side = dir.cross(Vec3::UP).normalized();
    let s = (e - EVENT_CENTER) * EVENT_SPEED;
    // La cabeza también se balancea con la onda del cuerpo.
    let wave = e * 2.2 * 1.3;
    let pos = Vec3::new(0.0, 5.3, 2.6) + dir * s + side * (wave.sin() * 0.5);
    let heading = (dir + side * (wave.cos() * 0.22)).normalized();
    Some(Creature {
        species: LEVIATHAN as u16,
        // El origen de la plantilla está a la mitad del cuerpo.
        pos: pos - heading * 5.3,
        rot: basis_x(heading),
        time: e * 2.2,
    })
}

/// Los tipos de animales y sus recorridos.
pub fn build() -> (Vec<Species>, Vec<Swimmer>) {
    let species = vec![peeper(), bladderfish(), jellyfish(), stalker(), leviathan()];
    let mut swimmers = Vec::new();
    // Tres cardúmenes de tres peepers.
    let schools = [
        Path { center: Vec3::new(0.0, 3.8, 0.0), rx: 8.5, rz: 8.5, bob: 0.5, speed: 0.20, phase: 0.0 },
        // Este pasa por dentro del arco.
        Path { center: Vec3::new(0.0, 2.4, 1.0), rx: 0.8, rz: 9.0, bob: 0.3, speed: 0.17, phase: 1.5 },
        Path { center: Vec3::new(0.0, 5.8, 0.0), rx: 11.0, rz: 10.5, bob: 0.6, speed: -0.14, phase: 3.0 },
    ];
    let formation = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(-0.75, 0.2, 0.6),
        Vec3::new(-0.65, -0.18, -0.65),
    ];
    for (si, path) in schools.iter().enumerate() {
        for (fi, off) in formation.iter().enumerate() {
            swimmers.push(Swimmer {
                species: 0,
                path: *path,
                offset: *off,
                phase: si as f32 * 2.1 + fi as f32 * 1.3,
            });
        }
    }
    // Tres bladderfish solitarios y lentos.
    for (center, r, speed, phase) in [
        (Vec3::new(5.0, 2.3, 6.0), 2.2, 0.12, 0.0),
        (Vec3::new(-6.5, 2.7, -4.0), 2.8, -0.10, 2.0),
        (Vec3::new(-2.5, 2.0, 8.5), 1.8, 0.14, 4.0),
    ] {
        swimmers.push(Swimmer {
            species: 1,
            path: Path { center, rx: r, rz: r * 0.8, bob: 0.25, speed, phase },
            offset: Vec3::ZERO,
            phase,
        });
    }
    // Dos medusas debajo del arco, casi quietas.
    for (center, phase) in [(Vec3::new(-1.3, 3.5, 0.7), 0.0), (Vec3::new(1.1, 3.6, -0.8), 2.5)] {
        swimmers.push(Swimmer {
            species: 2,
            path: Path { center, rx: 0.25, rz: 0.25, bob: 0.25, speed: 0.12, phase },
            offset: Vec3::ZERO,
            phase,
        });
    }
    // Stalker dando vueltas despacio, "cuidando" el metal de abajo.
    swimmers.push(Swimmer {
        species: 3,
        path: Path { center: STALKER_HOME, rx: 2.0, rz: 1.8, bob: 0.2, speed: 0.25, phase: 0.0 },
        offset: Vec3::ZERO,
        phase: 0.0,
    });
    (species, swimmers)
}
