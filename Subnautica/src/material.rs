//! Materiales del arrecife: cada uno tiene su propia textura procedural y sus
//! parámetros de albedo, especular, transparencia, reflectividad e índice de refracción.

use crate::{
    math::{Vec3, fract, lerp, smoothstep},
    noise::{fbm2, hash2, voronoi2},
    texture::Texture,
};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Standard,
    Sand,
    Rock,
    Sponge,
    TableCoral,
    Brain,
    ShellPlate,
    Glass,
    Bubble,
    Water,
    Metal,
}

/// Cómo se obtienen las coordenadas de textura de un impacto.
#[derive(Clone, Copy, Debug)]
pub enum Mapping {
    /// Coordenadas UV propias de la primitiva (caras del cubo, cilindro, esfera).
    Uv { su: f32, sv: f32 },
    /// Proyección desde arriba en coordenadas del mundo (metros * escala).
    WorldTop(f32),
    /// Proyección triplanar en el mundo (rocas: los estratos siguen la altura real).
    WorldTriplanar(f32),
    /// Proyección triplanar en el espacio del objeto (sin costuras en esferas).
    ObjectTriplanar(f32),
}

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub name: &'static str,
    pub kind: Kind,
    pub texture: usize,
    pub overlay: Option<usize>,
    pub mapping: Mapping,
    pub albedo: Vec3,
    pub specular: f32,
    pub shininess: f32,
    pub reflectivity: f32,
    pub transparency: f32,
    pub ior: f32,
    pub emission: Vec3,
    pub translucency: f32,
    pub bump: f32,
    /// Brillo propio: las zonas claras de la textura emiten luz (ojos, venas...).
    pub glow: f32,
    pub casts_shadow: bool,
}

impl Material {
    const fn base(name: &'static str, kind: Kind, texture: usize, mapping: Mapping) -> Self {
        Self {
            name,
            kind,
            texture,
            overlay: None,
            mapping,
            albedo: Vec3::ONE,
            specular: 0.1,
            shininess: 20.0,
            reflectivity: 0.0,
            transparency: 0.0,
            ior: 1.0,
            emission: Vec3::ZERO,
            translucency: 0.0,
            bump: 0.0,
            glow: 0.0,
            casts_shadow: true,
        }
    }
}

pub const SAND: u16 = 0;
pub const ROCK: u16 = 1;
pub const TABLE_CORAL: u16 = 2;
pub const SPONGE: u16 = 3;
pub const BRAIN: u16 = 4;
pub const BRANCH: u16 = 5;
pub const FAN: u16 = 6;
pub const GRASS: u16 = 7;
pub const RED_PLANT: u16 = 8;
pub const CRYSTAL: u16 = 9;
pub const BUBBLE: u16 = 10;
pub const WATER: u16 = 11;
pub const METAL: u16 = 12;
pub const PEEPER_BODY: u16 = 13;
pub const PEEPER_EYE: u16 = 14;
pub const PEEPER_FIN: u16 = 15;
pub const BEAK: u16 = 16;
pub const BLADDER: u16 = 17;
pub const BLADDER_SPINE: u16 = 18;
pub const EYE_WHITE: u16 = 19;
pub const MUSHROOM: u16 = 20;
pub const SHELL_PLATE: u16 = 21;
pub const RED_GRASS: u16 = 22;
pub const JELLY_BELL: u16 = 23;
pub const JELLY_TENTACLE: u16 = 24;
pub const JELLY_RIBBON: u16 = 25;
pub const STALKER_SKIN: u16 = 26;
pub const STALKER_PURPLE: u16 = 27;
pub const TEETH: u16 = 28;
pub const REAPER_SKIN: u16 = 29;
pub const REAPER_RED: u16 = 30;
pub const MOUTH: u16 = 31;

pub struct Library {
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub names: Vec<&'static str>,
}

pub fn build_library() -> Library {
    let mut textures = Vec::new();
    let mut names = Vec::new();
    let mut add = |name: &'static str, t: Texture| {
        textures.push(t);
        names.push(name);
        textures.len() - 1
    };
    let t_sand = add("arena", Texture::generate(512, sand_texture));
    let t_rock = add("roca", Texture::generate(512, rock_texture));
    let t_moss = add("musgo", Texture::generate(256, moss_texture));
    let t_table = add("coral_mesa", Texture::generate(256, table_coral_texture));
    let t_sponge = add("esponja", Texture::generate(256, sponge_texture));
    let t_brain = add("coral_cerebro", Texture::generate(256, brain_texture));
    let t_branch = add("coral_ramificado", Texture::generate(256, branch_texture));
    let t_fan = add("abanico", Texture::generate(256, fan_texture));
    let t_grass = add("pasto", Texture::generate(128, grass_texture));
    let t_red = add("planta_venosa", Texture::generate(128, red_plant_texture));
    let t_crystal = add("cuarzo", Texture::generate(256, crystal_texture));
    let t_bubble = add("burbuja", Texture::generate(64, bubble_texture));
    let t_water = add("agua", Texture::generate(256, water_texture));
    let t_metal = add("metal", Texture::generate(256, metal_texture));
    let t_peeper = add("peeper_cuerpo", Texture::generate(256, peeper_body_texture));
    let t_peeper_eye = add("peeper_ojo", Texture::generate(256, peeper_eye_texture));
    let t_fin = add("aleta", Texture::generate(128, fin_texture));
    let t_beak = add("pico", Texture::generate(64, beak_texture));
    let t_bladder = add("bladderfish", Texture::generate(128, bladder_texture));
    let t_spine = add("bladderfish_espina", Texture::generate(256, spine_texture));
    let t_eye_white = add("ojo_blanco", Texture::generate(128, eye_white_texture));
    let t_mushroom = add("hongo_acido", Texture::generate(256, mushroom_texture));
    let t_shell = add("placa_venosa", Texture::generate(256, shell_plate_texture));
    let t_red_grass = add("pasto_rojo", Texture::generate(128, red_grass_texture));
    let t_jelly = add("medusa", Texture::generate(256, jelly_bell_texture));
    let t_tentacle = add("tentaculo", Texture::generate(64, tentacle_texture));
    let t_ribbon = add("medusa_espiral", Texture::generate(128, ribbon_texture));
    let t_stalker = add("stalker_piel", Texture::generate(256, stalker_skin_texture));
    let t_stalker_p = add("stalker_morado", Texture::generate(128, stalker_purple_texture));
    let t_teeth = add("dientes", Texture::generate(64, teeth_texture));
    let t_reaper = add("leviatan_piel", Texture::generate(256, reaper_skin_texture));
    let t_reaper_red = add("leviatan_rojo", Texture::generate(128, reaper_red_texture));
    let t_mouth = add("boca", Texture::generate(64, mouth_texture));

    let mut materials = Vec::new();

    let mut m = Material::base("Arena", Kind::Sand, t_sand, Mapping::WorldTop(0.22));
    m.specular = 0.05;
    m.shininess = 14.0;
    m.reflectivity = 0.01;
    m.bump = 1.0;
    materials.push(m);

    let mut m = Material::base("Roca", Kind::Rock, t_rock, Mapping::WorldTriplanar(0.42));
    m.overlay = Some(t_moss);
    m.specular = 0.04;
    m.shininess = 12.0;
    m.reflectivity = 0.02;
    m.bump = 1.0;
    materials.push(m);

    let mut m = Material::base(
        "Coral de mesa",
        Kind::TableCoral,
        t_table,
        Mapping::Uv { su: 1.0, sv: 1.0 },
    );
    m.specular = 0.25;
    m.shininess = 30.0;
    m.reflectivity = 0.04;
    m.translucency = 0.25;
    m.glow = 0.12;
    materials.push(m);

    let mut m = Material::base("Esponja tubo", Kind::Sponge, t_sponge, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.12;
    m.shininess = 18.0;
    m.reflectivity = 0.03;
    materials.push(m);

    let mut m = Material::base(
        "Coral cerebro",
        Kind::Brain,
        t_brain,
        Mapping::ObjectTriplanar(0.5),
    );
    m.specular = 0.30;
    m.shininess = 36.0;
    m.reflectivity = 0.06;
    m.glow = 0.45;
    materials.push(m);

    let mut m = Material::base(
        "Coral ramificado",
        Kind::Standard,
        t_branch,
        Mapping::Uv { su: 1.0, sv: 1.0 },
    );
    m.specular = 0.32;
    m.shininess = 48.0;
    m.reflectivity = 0.08;
    m.emission = Vec3::new(0.012, 0.003, 0.008);
    materials.push(m);

    let mut m = Material::base("Abanico de mar", Kind::Standard, t_fan, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.08;
    m.reflectivity = 0.02;
    m.transparency = 0.0;
    m.translucency = 0.65;
    materials.push(m);

    let mut m = Material::base("Pasto marino", Kind::Standard, t_grass, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.22;
    m.shininess = 28.0;
    m.reflectivity = 0.05;
    m.translucency = 0.75;
    materials.push(m);

    let mut m = Material::base(
        "Planta venosa",
        Kind::Standard,
        t_red,
        Mapping::Uv { su: 1.0, sv: 1.0 },
    );
    m.specular = 0.28;
    m.shininess = 34.0;
    m.reflectivity = 0.06;
    m.translucency = 0.7;
    materials.push(m);

    let mut m = Material::base("Cuarzo", Kind::Glass, t_crystal, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 1.4;
    m.shininess = 220.0;
    m.reflectivity = 0.12;
    m.transparency = 0.82;
    m.ior = 1.52;
    m.emission = Vec3::new(0.015, 0.05, 0.06);
    m.casts_shadow = false;
    materials.push(m);

    let mut m = Material::base("Burbuja", Kind::Bubble, t_bubble, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 2.5;
    m.shininess = 500.0;
    m.reflectivity = 0.05;
    m.transparency = 0.97;
    // Aire dentro del agua: 1.0 / 1.333.
    m.ior = 0.75;
    m.casts_shadow = false;
    materials.push(m);

    let mut m = Material::base("Superficie del agua", Kind::Water, t_water, Mapping::WorldTop(0.08));
    m.specular = 3.0;
    m.shininess = 600.0;
    m.reflectivity = 0.02;
    m.transparency = 1.0;
    // Del agua hacia el aire.
    m.ior = 1.333;
    m.casts_shadow = false;
    materials.push(m);

    let mut m = Material::base("Metal", Kind::Metal, t_metal, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.9;
    m.shininess = 110.0;
    m.reflectivity = 0.42;
    materials.push(m);

    let mut m = Material::base("Peeper (cuerpo)", Kind::Standard, t_peeper, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.45;
    m.shininess = 40.0;
    m.reflectivity = 0.06;
    materials.push(m);

    let mut m = Material::base("Peeper (ojo)", Kind::Standard, t_peeper_eye, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 1.2;
    m.shininess = 120.0;
    m.reflectivity = 0.10;
    m.glow = 2.2;
    materials.push(m);

    let mut m = Material::base("Aleta", Kind::Standard, t_fin, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.2;
    m.translucency = 0.6;
    m.reflectivity = 0.03;
    materials.push(m);

    let mut m = Material::base("Pico", Kind::Standard, t_beak, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.3;
    m.shininess = 30.0;
    m.reflectivity = 0.04;
    materials.push(m);

    let mut m = Material::base("Bladderfish", Kind::Glass, t_bladder, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.9;
    m.shininess = 80.0;
    m.reflectivity = 0.05;
    m.transparency = 0.55;
    m.ior = 1.08;
    m.casts_shadow = false;
    materials.push(m);

    let mut m = Material::base("Bladderfish (espina)", Kind::Standard, t_spine, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.35;
    m.shininess = 40.0;
    m.reflectivity = 0.05;
    m.glow = 0.15;
    materials.push(m);

    let mut m = Material::base("Ojo", Kind::Standard, t_eye_white, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 1.5;
    m.shininess = 150.0;
    m.reflectivity = 0.12;
    materials.push(m);

    let mut m = Material::base("Hongo acido", Kind::Standard, t_mushroom, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.40;
    m.shininess = 40.0;
    m.reflectivity = 0.08;
    m.translucency = 0.3;
    m.glow = 0.35;
    materials.push(m);

    let mut m = Material::base("Placa venosa", Kind::ShellPlate, t_shell, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.35;
    m.shininess = 45.0;
    m.reflectivity = 0.07;
    m.glow = 0.9;
    materials.push(m);

    let mut m = Material::base("Pasto rojo", Kind::Standard, t_red_grass, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.15;
    m.shininess = 20.0;
    m.reflectivity = 0.03;
    m.translucency = 0.7;
    materials.push(m);

    let mut m = Material::base("Medusa", Kind::Standard, t_jelly, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.9;
    m.shininess = 80.0;
    m.reflectivity = 0.10;
    m.translucency = 0.6;
    m.glow = 0.55;
    materials.push(m);

    let mut m = Material::base("Tentaculo", Kind::Standard, t_tentacle, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.6;
    m.shininess = 60.0;
    m.reflectivity = 0.05;
    m.translucency = 0.6;
    m.glow = 0.35;
    materials.push(m);

    let mut m = Material::base("Medusa (espiral)", Kind::Standard, t_ribbon, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.4;
    m.reflectivity = 0.05;
    m.translucency = 0.7;
    m.glow = 0.45;
    materials.push(m);

    let mut m = Material::base("Stalker (piel)", Kind::Standard, t_stalker, Mapping::ObjectTriplanar(0.9));
    m.specular = 0.35;
    m.shininess = 30.0;
    m.reflectivity = 0.05;
    materials.push(m);

    let mut m = Material::base("Stalker (morado)", Kind::Standard, t_stalker_p, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.4;
    m.shininess = 35.0;
    m.reflectivity = 0.05;
    m.translucency = 0.3;
    materials.push(m);

    let mut m = Material::base("Dientes", Kind::Standard, t_teeth, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.8;
    m.shininess = 60.0;
    m.reflectivity = 0.08;
    materials.push(m);

    let mut m = Material::base("Leviatan (piel)", Kind::Standard, t_reaper, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.45;
    m.shininess = 40.0;
    m.reflectivity = 0.06;
    materials.push(m);

    let mut m = Material::base("Leviatan (rojo)", Kind::Standard, t_reaper_red, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.4;
    m.shininess = 35.0;
    m.reflectivity = 0.05;
    m.translucency = 0.2;
    materials.push(m);

    let mut m = Material::base("Boca", Kind::Standard, t_mouth, Mapping::Uv { su: 1.0, sv: 1.0 });
    m.specular = 0.3;
    m.reflectivity = 0.04;
    materials.push(m);

    Library {
        materials,
        textures,
        names,
    }
}

// ---------------------------------------------------------------------------
// Texturas procedurales (todas en espacio lineal).
// ---------------------------------------------------------------------------

fn sand_texture(u: f32, v: f32) -> Vec3 {
    let big = fbm2(u * 4.0, v * 4.0, 4, 4);
    let mid = fbm2(u * 16.0 + 3.1, v * 16.0 + 1.7, 16, 3);
    let gx = (u * 512.0) as i32;
    let gy = (v * 512.0) as i32;
    let grain = hash2(gx, gy);
    let speck = hash2(gx + 917, gy + 211);
    let mut c = Vec3::new(0.74, 0.55, 0.32)
        * (0.84 + 0.24 * big)
        * (0.92 + 0.14 * mid)
        * (0.93 + 0.14 * grain);
    if speck > 0.986 {
        c *= 0.55;
    } else if speck < 0.008 {
        c = c * 1.22 + Vec3::new(0.04, 0.03, 0.03);
    }
    let algae = smoothstep(0.58, 0.78, fbm2(u * 3.0 + 5.0, v * 3.0 + 1.0, 3, 4));
    c.lerp(Vec3::new(0.40, 0.44, 0.27), algae * 0.30)
}

fn rock_texture(u: f32, v: f32) -> Vec3 {
    // Roca sedimentaria: estratos horizontales deformados, cada uno con su tono.
    let warp = fbm2(u * 3.0, v * 3.0, 3, 4) - 0.5;
    let layer_coord = (v + warp * 0.12) * 11.0;
    let layer = layer_coord.floor();
    let within = layer_coord - layer;
    let band = hash2(layer as i32, 7);
    let band2 = hash2(layer as i32, 19);
    let cool = Vec3::new(0.30, 0.33, 0.34);
    let warm = Vec3::new(0.52, 0.47, 0.39);
    let mut c = cool.lerp(warm, band * 0.8 + 0.1);
    c *= 0.82 + 0.3 * band2;
    // Borde inferior de cada estrato más oscuro (sombra de la saliente).
    let lip = 1.0 - smoothstep(0.0, 0.18, within);
    c *= 1.0 - lip * 0.35;
    // Variación a varias escalas.
    let n1 = fbm2(u * 8.0, v * 8.0, 8, 5);
    let blotch = fbm2(u * 2.0 + 9.0, v * 2.0 + 2.0, 2, 4);
    c *= 0.72 + 0.5 * n1;
    c *= 0.8 + 0.4 * blotch;
    // Grietas grandes y suaves.
    let (f1, f2, _) = voronoi2(u * 3.0 + warp * 1.5, v * 3.0 + warp, 3);
    let crack = 1.0 - smoothstep(0.0, 0.035, f2 - f1);
    c *= 1.0 - crack * 0.35;
    // Poros oscuros y motas claras.
    let gx = (u * 512.0) as i32;
    let gy = (v * 512.0) as i32;
    let speck = hash2(gx, gy);
    if speck > 0.975 {
        c = c * 1.3 + Vec3::splat(0.03);
    } else if speck < 0.02 {
        c *= 0.55;
    }
    c
}

fn moss_texture(u: f32, v: f32) -> Vec3 {
    let n = fbm2(u * 6.0, v * 6.0, 6, 5);
    let d = fbm2(u * 24.0 + 2.0, v * 24.0 + 7.0, 24, 3);
    let mut c = Vec3::new(0.11, 0.17, 0.07).lerp(Vec3::new(0.30, 0.40, 0.13), n);
    c *= 0.8 + 0.4 * d;
    let (f1, _, id) = voronoi2(u * 14.0, v * 14.0, 14);
    if f1 < 0.18 && id > 0.72 {
        // Pequeñas colonias de alga rojiza.
        let k = 1.0 - f1 / 0.18;
        c = c.lerp(Vec3::new(0.62, 0.22, 0.20), k * 0.8);
    }
    c
}

fn table_coral_texture(u: f32, v: f32) -> Vec3 {
    // Coral "almohada" como en el juego: base clara (se tiñe por instancia),
    // estrías suaves y hoyuelos oscuros.
    let warp = fbm2(u * 4.0, v * 4.0, 4, 3);
    let streak = (u * TAU * 7.0 + warp * 4.0).sin() * 0.5 + 0.5;
    let (f1, _, id) = voronoi2(u * 10.0, v * 5.0, 10);
    let dimple = 1.0 - smoothstep(0.08, 0.2, f1);
    let mut c = Vec3::splat(0.82) * (0.82 + 0.18 * streak) * (0.9 + 0.2 * fbm2(u * 12.0, v * 12.0, 12, 3));
    // Borde superior un poco más claro.
    c = c * (0.85 + 0.3 * smoothstep(0.45, 0.75, v));
    let pit = if id > 0.35 { dimple } else { 0.0 };
    c.lerp(Vec3::new(0.10, 0.06, 0.08), pit * 0.85)
}

fn sponge_texture(u: f32, v: f32) -> Vec3 {
    let warp = fbm2(u * 4.0, v * 4.0, 4, 3);
    let ridges = (u * TAU * 9.0 + warp * 3.0).sin() * 0.5 + 0.5;
    let (f1, _, _) = voronoi2(u * 24.0, v * 24.0, 24);
    let pore = 1.0 - smoothstep(0.06, 0.18, f1);
    let mut c = Vec3::new(0.95, 0.70, 0.13) * (0.72 + 0.38 * v);
    c *= 0.78 + 0.28 * ridges;
    c *= 0.9 + 0.2 * fbm2(u * 12.0, v * 12.0, 12, 3);
    c.lerp(Vec3::new(0.32, 0.18, 0.04), pore * 0.75)
}

fn brain_texture(u: f32, v: f32) -> Vec3 {
    // Coral cerebro de Subnautica: domo morado lleno de pólipos con anillo verde.
    let (f1, f2, id) = voronoi2(u * 8.0, v * 8.0, 8);
    let edge = f2 - f1;
    let base = Vec3::new(0.40, 0.14, 0.55) * (0.8 + 0.3 * fbm2(u * 6.0, v * 6.0, 6, 3));
    let ring = smoothstep(0.12, 0.17, f1) * (1.0 - smoothstep(0.27, 0.33, f1));
    let center = 1.0 - smoothstep(0.10, 0.14, f1);
    let ring_color = Vec3::new(0.15, 0.85, 0.55).lerp(Vec3::new(0.35, 0.95, 0.85), id);
    let mut c = base;
    c = c.lerp(ring_color, ring);
    c = c.lerp(Vec3::new(0.05, 0.02, 0.08), center * 0.9);
    // Surco oscuro entre pólipos.
    c * (0.55 + 0.45 * smoothstep(0.0, 0.08, edge))
}

fn branch_texture(u: f32, v: f32) -> Vec3 {
    let (f1, _, id) = voronoi2(u * 14.0, v * 14.0, 14);
    let polyp = 1.0 - smoothstep(0.06, 0.24, f1);
    let n = fbm2(u * 6.0, v * 6.0, 6, 3);
    let base = Vec3::new(0.84, 0.30, 0.54) * (0.78 + 0.3 * n);
    base + Vec3::new(0.32, 0.26, 0.28) * polyp * (0.6 + 0.4 * id)
}

fn fan_texture(u: f32, v: f32) -> Vec3 {
    let n = fbm2(u * 5.0, v * 5.0, 5, 4);
    let edge = smoothstep(0.35, 1.0, ((u - 0.5) * 2.0).abs().max(((v - 0.5) * 2.0).abs()));
    Vec3::new(0.46, 0.14, 0.58) * (0.75 + 0.45 * n) + Vec3::new(0.25, 0.10, 0.18) * edge
}

fn grass_texture(u: f32, v: f32) -> Vec3 {
    let center = (-((u - 0.5) * 16.0).powi(2)).exp();
    let side = ((u - 0.5) * TAU * 5.0).cos() * 0.5 + 0.5;
    let n = fbm2(u * 8.0, v * 8.0, 8, 3);
    let mut c = Vec3::new(0.07, 0.20, 0.05).lerp(Vec3::new(0.30, 0.62, 0.15), v.powf(0.8));
    c *= 0.82 + 0.15 * side + 0.15 * n;
    c + Vec3::new(0.10, 0.16, 0.04) * center
}

fn red_plant_texture(u: f32, v: f32) -> Vec3 {
    let du = (u - 0.5).abs();
    let center = (-(du * 18.0).powi(2)).exp();
    let veins = fract(v * 7.0 - du * 3.0);
    let vein = 1.0 - smoothstep(0.0, 0.12, veins.min(1.0 - veins));
    let mut c = Vec3::new(0.58, 0.12, 0.06).lerp(Vec3::new(0.98, 0.45, 0.16), v);
    c *= 0.85 + 0.25 * fbm2(u * 8.0, v * 8.0, 8, 3);
    c + Vec3::new(0.35, 0.28, 0.08) * (vein * 0.7 + center)
}

fn crystal_texture(u: f32, v: f32) -> Vec3 {
    let streak = fbm2(u * 3.0, v * 12.0, 0, 4);
    let cloud = smoothstep(0.55, 0.8, fbm2(u * 6.0 + 4.0, v * 6.0, 0, 4));
    let mut c = Vec3::new(0.80, 0.96, 1.0) * (0.86 + 0.14 * streak);
    c = c.lerp(Vec3::new(0.95, 0.82, 1.0), lerp(0.0, 0.35, v));
    c.lerp(Vec3::ONE, cloud * 0.6)
}

fn bubble_texture(u: f32, v: f32) -> Vec3 {
    let n = fbm2(u * 4.0, v * 4.0, 4, 2);
    Vec3::new(0.90, 0.97, 1.0) * (0.94 + 0.06 * n)
}

fn water_texture(u: f32, v: f32) -> Vec3 {
    let (f1, f2, _) = voronoi2(u * 8.0, v * 8.0, 8);
    let net = 1.0 - smoothstep(0.0, 0.25, f2 - f1);
    Vec3::new(0.82, 0.96, 1.0) * (0.92 + 0.16 * net)
}

fn metal_texture(u: f32, v: f32) -> Vec3 {
    let brushed = fbm2(u * 64.0, v * 4.0, 0, 3);
    let mut c = Vec3::new(0.62, 0.65, 0.69) * (0.82 + 0.25 * brushed);
    let edge = u.min(1.0 - u).min(v).min(1.0 - v);
    if edge < 0.07 {
        c *= 0.62;
        // Remaches en el marco.
        let along = if u.min(1.0 - u) < 0.07 { v } else { u };
        let rivet_d = ((fract(along * 8.0) - 0.5) * 0.125).abs().hypot(edge - 0.035);
        if rivet_d < 0.016 {
            c = Vec3::splat(0.85);
        }
    }
    if (u - 0.5).abs() < 0.006 && edge >= 0.07 {
        c *= 0.4;
    }
    // Franja de peligro amarilla y negra.
    if (0.40..0.56).contains(&v) && edge >= 0.07 {
        let stripe = fract((u + v) * 9.0) < 0.5;
        c = if stripe {
            Vec3::new(0.92, 0.66, 0.06)
        } else {
            Vec3::splat(0.05)
        };
    }
    // Óxido y algas por llevar tiempo bajo el agua.
    let rust = smoothstep(0.58, 0.75, fbm2(u * 5.0 + 3.0, v * 5.0, 0, 5));
    c = c.lerp(Vec3::new(0.42, 0.27, 0.13), rust * 0.75);
    let algae = smoothstep(0.62, 0.80, fbm2(u * 4.0 + 11.0, v * 4.0 + 2.0, 0, 4)) * (1.0 - v);
    c.lerp(Vec3::new(0.20, 0.36, 0.12), algae * 0.8)
}

fn peeper_body_texture(u: f32, v: f32) -> Vec3 {
    let n = fbm2(u * 8.0, v * 6.0, 8, 4);
    let navy = Vec3::new(0.035, 0.06, 0.26);
    let teal = Vec3::new(0.03, 0.14, 0.20);
    let mut c = teal.lerp(navy, smoothstep(0.25, 0.6, v)) * (0.8 + 0.4 * n);
    // Pliegues finos de la piel.
    let (f1, f2, _) = voronoi2(u * 18.0, v * 9.0, 18);
    c *= 1.0 - (1.0 - smoothstep(0.0, 0.05, f2 - f1)) * 0.35;
    c
}

fn peeper_eye_texture(u: f32, v: f32) -> Vec3 {
    let x = u * 2.0 - 1.0;
    let y = v * 2.0 - 1.0;
    let r = (x * x + y * y).sqrt();
    let a = y.atan2(x);
    let fibers = (a * 60.0 + fbm2(x * 6.0 + 3.0, y * 6.0, 0, 3) * 8.0).sin() * 0.5 + 0.5;
    if r < 0.27 {
        // Pupila oscura con brillitos.
        let speck = if hash2((u * 256.0) as i32, (v * 256.0) as i32) > 0.985 { 0.25 } else { 0.0 };
        return Vec3::new(0.04, 0.07, 0.20) * (0.7 + 0.6 * (1.0 - r / 0.27)) + Vec3::splat(speck);
    }
    if r < 0.33 {
        return Vec3::new(0.30, 0.17, 0.03) * (0.7 + 0.3 * fibers);
    }
    let mut c = Vec3::new(1.0, 0.85, 0.22).lerp(Vec3::new(1.0, 0.62, 0.10), smoothstep(0.33, 0.95, r));
    c *= 0.82 + 0.25 * fibers;
    // Anillo oscuro donde el ojo toca el cuerpo.
    c.lerp(Vec3::new(0.06, 0.08, 0.18), smoothstep(0.9, 0.99, r))
}

fn fin_texture(u: f32, v: f32) -> Vec3 {
    // Radios finos que se abren desde la base de la aleta.
    let a = (u - 0.5).atan2(v + 0.4);
    let rays = (a * 40.0).sin() * 0.5 + 0.5;
    Vec3::new(0.05, 0.12, 0.20).lerp(Vec3::new(0.08, 0.24, 0.28), v) * (0.85 + 0.2 * rays)
}

fn beak_texture(u: f32, v: f32) -> Vec3 {
    Vec3::new(0.62, 0.36, 0.12) * (0.8 + 0.3 * fbm2(u * 6.0, v * 6.0, 6, 2))
}

fn bladder_texture(u: f32, v: f32) -> Vec3 {
    Vec3::new(0.68, 0.64, 0.98) * (0.88 + 0.15 * fbm2(u * 4.0, v * 4.0, 4, 3))
}

fn spine_texture(u: f32, v: f32) -> Vec3 {
    let (f1, _, _) = voronoi2(u * 6.0, v * 22.0, 0);
    let spot = 1.0 - smoothstep(0.18, 0.3, f1);
    let base = Vec3::new(0.55, 0.12, 0.30);
    let spot_color = Vec3::new(0.30, 0.80, 0.80).lerp(Vec3::new(1.0, 0.55, 0.40), smoothstep(0.75, 0.9, v));
    let mut c = base.lerp(spot_color, spot);
    // Cabeza anaranjada.
    c = c.lerp(Vec3::new(0.85, 0.45, 0.20) * (0.8 + 0.4 * spot), smoothstep(0.9, 0.97, v));
    c
}

fn eye_white_texture(u: f32, v: f32) -> Vec3 {
    // La esfera mira hacia +x local: ahí (u = 0.5, v = 0.5) va la pupila.
    let du = (u - 0.5) * 2.2;
    let dv = (v - 0.5) * 1.1;
    let d = (du * du + dv * dv).sqrt();
    if d < 0.16 {
        Vec3::splat(0.02)
    } else {
        Vec3::new(0.85, 0.88, 0.92) * (0.9 + 0.1 * smoothstep(0.16, 0.5, d))
    }
}

fn mushroom_texture(u: f32, v: f32) -> Vec3 {
    // Copa morada con gajos verticales; cerca del polo superior (v -> 1) está el
    // hueco: aro naranja/rosado que brilla y centro morado oscuro.
    let ribs = (u * TAU * 11.0).cos() * 0.5 + 0.5;
    let mut c = Vec3::new(0.30, 0.34, 0.85).lerp(Vec3::new(0.55, 0.40, 0.95), smoothstep(0.3, 0.8, v));
    c *= 0.82 + 0.22 * ribs;
    c *= 0.9 + 0.2 * fbm2(u * 8.0, v * 8.0, 8, 3);
    let top = smoothstep(0.80, 0.86, v);
    let ring = smoothstep(0.84, 0.90, v) * (1.0 - smoothstep(0.93, 0.98, v));
    let hole = smoothstep(0.95, 0.995, v);
    let inner = Vec3::new(1.0, 0.55, 0.35).lerp(Vec3::new(1.0, 0.35, 0.6), ribs);
    c = c.lerp(Vec3::new(0.6, 0.35, 0.8), top * 0.6);
    c = c.lerp(inner, ring);
    c.lerp(Vec3::new(0.18, 0.05, 0.25), hole)
}

fn red_grass_texture(u: f32, v: f32) -> Vec3 {
    let center = (-((u - 0.5) * 14.0).powi(2)).exp();
    let c = Vec3::new(0.35, 0.03, 0.05).lerp(Vec3::new(0.85, 0.08, 0.12), v.powf(0.7));
    c * (0.85 + 0.25 * fbm2(u * 8.0, v * 8.0, 8, 3)) + Vec3::new(0.15, 0.02, 0.03) * center
}

fn jelly_bell_texture(u: f32, v: f32) -> Vec3 {
    // Campana morada; abajo una franja rosada con "arcos" y puntitos claros.
    let base = Vec3::new(0.22, 0.16, 0.45) * (0.85 + 0.3 * fbm2(u * 6.0, v * 6.0, 6, 3));
    let band = 1.0 - smoothstep(0.25, 0.42, v);
    let arches = ((u * 8.0).fract() - 0.5).abs() < 0.12 && v > 0.08;
    let mut c = base.lerp(Vec3::new(0.85, 0.35, 0.55), band * if arches { 1.0 } else { 0.45 });
    let dot = hash2((u * 64.0) as i32, (v * 64.0) as i32) > 0.93 && v < 0.4;
    if dot {
        c = Vec3::new(0.7, 0.95, 1.0);
    }
    c
}

fn tentacle_texture(u: f32, v: f32) -> Vec3 {
    Vec3::new(0.45, 0.62, 0.90) * (0.85 + 0.2 * (v * 20.0 + u).sin())
}

fn ribbon_texture(u: f32, v: f32) -> Vec3 {
    let lines = (u * TAU * 6.0).sin() * 0.5 + 0.5;
    let mut c = Vec3::new(0.38, 0.40, 0.80).lerp(Vec3::new(0.62, 0.30, 0.65), u) * (0.85 + 0.2 * lines);
    if hash2((u * 48.0) as i32, (v * 48.0) as i32) > 0.95 && !(0.15..0.85).contains(&u) {
        c = Vec3::new(0.8, 1.0, 1.0);
    }
    c
}

fn shell_plate_texture(u: f32, v: f32) -> Vec3 {
    // Placa venosa: base oscura con venas amarillas que se ramifican desde abajo.
    let x = u * 2.0 - 1.0;
    let y = v * 2.0 - 1.0;
    let dx = x;
    let dy = y + 0.95;
    let r = (dx * dx + dy * dy).sqrt() * 0.55;
    let a = dx.atan2(dy);
    let warp = fbm2(x * 3.0 + 5.0, y * 3.0 + 5.0, 0, 3) - 0.5;
    let branches = 2.0 + (r * 4.0).floor() * 1.5;
    let lines = (a * branches + warp * 2.5 + r * 1.5).sin().abs();
    let width = 0.05 + 0.07 * (1.0 - r).max(0.0);
    let mut vein = 1.0 - smoothstep(0.0, width, lines);
    vein = vein.max((1.0 - smoothstep(0.0, 0.05, ((r * 6.0 + warp * 2.0) * PI).sin().abs())) * 0.45);
    vein = vein.max(1.0 - smoothstep(0.0, 0.06 * (1.3 - r), dx.abs()));
    let edge = (x * x + y * y).sqrt();
    vein *= 1.0 - smoothstep(0.85, 0.98, edge);
    let base = Vec3::new(0.06, 0.08, 0.04) * (0.7 + 0.6 * fbm2(x * 4.0 + 2.0, y * 4.0, 0, 4));
    let rim = smoothstep(0.86, 0.97, edge);
    let base = base.lerp(Vec3::new(0.20, 0.20, 0.08), rim);
    base.lerp(Vec3::new(0.98, 0.86, 0.35), vein)
}

fn stalker_skin_texture(u: f32, v: f32) -> Vec3 {
    // Verde agua grisáceo con rayas moradas (la textura se proyecta desde los lados).
    let warp = fbm2(u * 3.0, v * 3.0, 3, 3);
    let stripe = ((u * 9.0 + warp * 1.5) * TAU).sin() * 0.5 + 0.5;
    let base = Vec3::new(0.40, 0.58, 0.56) * (0.85 + 0.25 * fbm2(u * 10.0, v * 10.0, 10, 3));
    let purple = Vec3::new(0.30, 0.20, 0.55);
    base.lerp(purple, smoothstep(0.62, 0.85, stripe) * 0.8)
}

fn stalker_purple_texture(u: f32, v: f32) -> Vec3 {
    Vec3::new(0.32, 0.16, 0.62).lerp(Vec3::new(0.48, 0.25, 0.80), v) * (0.85 + 0.25 * fbm2(u * 6.0, v * 6.0, 6, 3))
}

fn teeth_texture(u: f32, v: f32) -> Vec3 {
    Vec3::new(0.80, 0.82, 0.74) * (0.9 + 0.1 * fbm2(u * 4.0, v * 4.0, 4, 2))
}

fn reaper_skin_texture(u: f32, v: f32) -> Vec3 {
    // Piel azul pálido; en el lomo (u ~ 0.25 en los segmentos) una franja roja cortada.
    let n = fbm2(u * 8.0, v * 8.0, 8, 4);
    let mut c = Vec3::new(0.55, 0.78, 0.80).lerp(Vec3::new(0.80, 0.90, 0.88), smoothstep(0.55, 0.85, (u - 0.75).abs() * -2.0 + 1.0));
    c *= 0.85 + 0.25 * n;
    let back = 1.0 - smoothstep(0.03, 0.09, (u - 0.25).abs());
    let dashes = smoothstep(-0.2, 0.4, (v * 40.0).sin());
    c.lerp(Vec3::new(0.62, 0.12, 0.08), back * dashes * 0.9)
}

fn reaper_red_texture(u: f32, v: f32) -> Vec3 {
    let n = fbm2(u * 6.0, v * 6.0, 6, 4);
    let streak = (v * 30.0 + n * 4.0).sin() * 0.5 + 0.5;
    Vec3::new(0.55, 0.12, 0.07).lerp(Vec3::new(0.75, 0.22, 0.12), streak * 0.6) * (0.85 + 0.25 * n)
}

fn mouth_texture(u: f32, v: f32) -> Vec3 {
    Vec3::new(0.30, 0.03, 0.04) * (0.7 + 0.4 * fbm2(u * 6.0, v * 6.0, 6, 3))
}
