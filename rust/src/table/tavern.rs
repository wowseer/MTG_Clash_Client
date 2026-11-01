//! Tavernen-Kulisse: Holztisch mit eingelassener Messing-Trennlinie, Kerzen,
//! Krüge, Münzen sowie der Gastraum (Dielenboden, Fachwerkwände, Kamin mit Feuer,
//! Bank, Hocker, Fässer, Kisten, Regale mit Flaschen).
//!
//! Alles wird in `ready()` prozedural erzeugt. Maßstab: 1 Einheit = 10 cm
//! (eine Karte ist 0,63 × 0,88). Die Tischoberfläche liegt bei y = 0, die
//! Trennlinie bei z = 0: Spielerhälfte z > 0, Gegnerhälfte z < 0.

use godot::classes::base_material_3d::{Feature, Flags, ShadingMode, TextureParam};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::light_3d::Param;
use godot::classes::{
    BoxMesh, CylinderMesh, INode3D, IOmniLight3D, Material, Mesh, MeshInstance3D, OmniLight3D,
    PlaneMesh, SphereMesh, StandardMaterial3D, Texture2D, TorusMesh,
};
use godot::prelude::*;

use super::textures;

// ---------------------------------------------------------------- FlickerLight

/// Punktlicht, das wie eine Kerzen-/Kaminflamme flackert.
#[derive(GodotClass)]
#[class(base=OmniLight3D)]
pub struct FlickerLight {
    /// Grundhelligkeit des Lichts.
    #[export]
    base_energy: f32,
    /// Relative Stärke des Flackerns (0 = ruhig).
    #[export]
    flicker_strength: f32,
    /// Geschwindigkeit des Flackerns (1 = normal).
    #[export]
    flicker_speed: f32,
    time: f32,
    phase: f32,
    base: Base<OmniLight3D>,
}

#[godot_api]
impl IOmniLight3D for FlickerLight {
    fn init(base: Base<OmniLight3D>) -> Self {
        Self {
            base_energy: 1.2,
            flicker_strength: 0.25,
            flicker_speed: 1.0,
            time: 0.0,
            phase: 0.0,
            base,
        }
    }

    fn ready(&mut self) {
        // Unterschiedliche Phase je Licht, damit nicht alle synchron flackern.
        let p = self.base().get_global_position();
        self.phase = (p.x * 12.9898 + p.z * 78.233).sin().abs() * 100.0;
    }

    fn process(&mut self, delta: f64) {
        self.time += delta as f32 * self.flicker_speed;
        let t = self.time + self.phase;
        let n = 0.5 * (t * 7.3).sin() + 0.3 * (t * 13.7 + 1.3).sin() + 0.2 * (t * 23.1 + 4.1).sin();
        let e = self.base_energy * (1.0 + self.flicker_strength * n);
        self.base_mut().set_param(Param::ENERGY, e);
    }
}

// ---------------------------------------------------------------- Tavern

/// Tavernen-Kulisse: Holztisch mit Messing-Trennlinie, Kerzen, Krüge, Münzen sowie Gastraum
/// mit Kamin, Fachwerk, Regalen, Fässern, Bank und Hockern – alles prozedural erzeugt.
/// Maßstab: 1 Einheit = 10 cm; Tischoberfläche bei y = 0, Trennlinie bei z = 0.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct Tavern {
    /// Größe der Tischplatte (x = Breite, y = Tiefe).
    #[export]
    table_size: Vector2,
    /// Höhe der Tischplatte über dem Boden.
    #[export]
    table_height: f32,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for Tavern {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            table_size: Vector2::new(14.0, 9.6),
            table_height: 7.5,
            base,
        }
    }

    fn ready(&mut self) {
        let m = Materials::new();
        self.build_table(&m);
        self.build_room(&m);
        self.build_furniture(&m);
        self.build_table_props(&m);
    }
}

/// Gemeinsam genutzte Materialien. Holz/Stein/Putz sind triplanar in
/// Weltkoordinaten projiziert (Skalierung = Wiederholungen pro Einheit).
struct Materials {
    oak: Gd<Material>,
    dark_wood: Gd<Material>,
    barrel_wood: Gd<Material>,
    floor: Gd<Material>,
    plaster: Gd<Material>,
    stone: Gd<Material>,
    crate_wood: Gd<Material>,
    iron: Gd<Material>,
    brass: Gd<Material>,
    /// Mattes, angelaufenes Messing für die dezente Trennlinie.
    brass_dull: Gd<Material>,
    wax: Gd<Material>,
    pewter: Gd<Material>,
    foam: Gd<Material>,
    gold: Gd<Material>,
}

impl Materials {
    fn new() -> Self {
        // Tischbretter laufen in Spielrichtung (z), damit die Trennlinie (x) heraussticht.
        let oak_tex = textures::wood_planks_oriented(
            5,
            Color::from_rgb(0.52, 0.33, 0.18),
            Color::from_rgb(0.2, 0.11, 0.05),
            1,
            true,
        );
        let floor_tex = textures::wood_planks(
            8,
            Color::from_rgb(0.3, 0.2, 0.12),
            Color::from_rgb(0.09, 0.055, 0.03),
            9,
        );
        let crate_tex = textures::wood_planks(
            4,
            Color::from_rgb(0.5, 0.36, 0.2),
            Color::from_rgb(0.22, 0.14, 0.07),
            5,
        );
        Self {
            oak: textured(&oak_tex, 0.09, 0.6),
            dark_wood: solid(Color::from_rgb(0.13, 0.075, 0.035), 0.75, 0.0),
            barrel_wood: solid(Color::from_rgb(0.32, 0.19, 0.09), 0.8, 0.0),
            floor: textured(&floor_tex, 0.06, 0.85),
            plaster: textured(&textures::plaster(), 0.04, 1.0),
            stone: textured(&textures::stone_wall(), 0.05, 0.95),
            crate_wood: textured(&crate_tex, 0.2, 0.85),
            iron: solid(Color::from_rgb(0.12, 0.12, 0.12), 0.5, 0.8),
            brass: solid(Color::from_rgb(0.78, 0.58, 0.26), 0.3, 0.95),
            brass_dull: solid(Color::from_rgb(0.42, 0.32, 0.17), 0.55, 0.6),
            wax: solid(Color::from_rgb(0.93, 0.87, 0.7), 0.6, 0.0),
            pewter: solid(Color::from_rgb(0.5, 0.48, 0.45), 0.35, 0.85),
            foam: solid(Color::from_rgb(0.95, 0.9, 0.78), 0.9, 0.0),
            gold: solid(Color::from_rgb(0.9, 0.7, 0.25), 0.3, 1.0),
        }
    }
}

impl Tavern {
    fn floor_y(&self) -> f32 {
        -self.table_height
    }

    fn add_mesh<T: Inherits<Mesh>>(&mut self, name: &str, mesh: Gd<T>, pos: Vector3) -> Gd<MeshInstance3D> {
        let mut mi = MeshInstance3D::new_alloc();
        mi.set_name(name);
        mi.set_mesh(&mesh.upcast::<Mesh>());
        mi.set_position(pos);
        self.base_mut().add_child(&mi);
        mi
    }

    fn add_box(&mut self, name: &str, size: Vector3, pos: Vector3, mat: &Gd<Material>) -> Gd<MeshInstance3D> {
        let mut b = BoxMesh::new_gd();
        b.set_size(size);
        b.set_material(mat);
        self.add_mesh(name, b, pos)
    }

    fn add_cylinder(
        &mut self,
        name: &str,
        radius: (f32, f32),
        height: f32,
        pos: Vector3,
        mat: &Gd<Material>,
    ) -> Gd<MeshInstance3D> {
        let mut c = CylinderMesh::new_gd();
        c.set_top_radius(radius.0);
        c.set_bottom_radius(radius.1);
        c.set_height(height);
        c.set_material(mat);
        self.add_mesh(name, c, pos)
    }

    fn add_flame(&mut self, pos: Vector3, radius: f32, height: f32, color: Color, energy: f32) {
        let mut s = SphereMesh::new_gd();
        s.set_radius(radius);
        s.set_height(height);
        s.set_material(&glow(color, energy));
        let mut f = self.add_mesh("Flame", s, pos);
        f.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    }

    fn add_flicker(&mut self, pos: Vector3, color: Color, energy: f32, range: f32, strength: f32, shadow: bool) {
        let mut light = FlickerLight::new_alloc();
        light.set_color(color);
        light.set_param(Param::RANGE, range);
        light.set_param(Param::ATTENUATION, 1.2);
        light.set_shadow(shadow);
        light.set_position(pos);
        {
            let mut l = light.bind_mut();
            l.base_energy = energy;
            l.flicker_strength = strength;
        }
        self.base_mut().add_child(&light);
    }

    // ------------------------------------------------------------ Tisch

    fn build_table(&mut self, m: &Materials) {
        let ts = self.table_size;
        let fy = self.floor_y();

        // Platte mit dunklerem Rahmen
        self.add_box("TableTop", Vector3::new(ts.x, 0.6, ts.y), Vector3::new(0.0, -0.3, 0.0), &m.oak);
        let rim = 0.35;
        for (size, pos) in [
            (Vector3::new(ts.x + rim * 2.0, 0.7, rim), Vector3::new(0.0, -0.32, ts.y * 0.5 + rim * 0.5)),
            (Vector3::new(ts.x + rim * 2.0, 0.7, rim), Vector3::new(0.0, -0.32, -ts.y * 0.5 - rim * 0.5)),
            (Vector3::new(rim, 0.7, ts.y), Vector3::new(ts.x * 0.5 + rim * 0.5, -0.32, 0.0)),
            (Vector3::new(rim, 0.7, ts.y), Vector3::new(-ts.x * 0.5 - rim * 0.5, -0.32, 0.0)),
        ] {
            self.add_box("TableRim", size, pos, &m.dark_wood);
        }
        // Zarge und Beine
        self.add_box(
            "Apron",
            Vector3::new(ts.x - 1.0, 1.2, ts.y - 1.0),
            Vector3::new(0.0, -1.2, 0.0),
            &m.dark_wood,
        );
        let leg_h = self.table_height - 0.6;
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let pos = Vector3::new(sx * (ts.x * 0.5 - 1.0), fy + leg_h * 0.5, sz * (ts.y * 0.5 - 1.0));
            self.add_box("Leg", Vector3::new(0.9, leg_h, 0.9), pos, &m.dark_wood);
        }

        // Trennlinie: eingelassener Messingstreifen mit Ring in der Mitte
        let mut strip = self.add_box(
            "Divider",
            Vector3::new(ts.x - 1.2, 0.012, 0.05),
            Vector3::ZERO,
            &m.brass_dull,
        );
        strip.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        let mut ring = TorusMesh::new_gd();
        ring.set_inner_radius(0.26);
        ring.set_outer_radius(0.31);
        ring.set_material(&m.brass_dull);
        let mut r = self.add_mesh("DividerRing", ring, Vector3::ZERO);
        r.set_scale(Vector3::new(1.0, 0.08, 1.0));
        r.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    }

    // ------------------------------------------------------------ Raum

    fn build_room(&mut self, m: &Materials) {
        let fy = self.floor_y();
        let north = -24.0; // Wand hinter dem Gegner (mit Kamin)
        let south = 30.0; // Wand hinter dem Spieler
        let side = 34.0;
        let wall_h = 30.0;
        let mid_z = (north + south) * 0.5;
        let depth = south - north;

        // Boden
        let mut floor = PlaneMesh::new_gd();
        floor.set_size(Vector2::new(side * 2.0, depth));
        floor.set_material(&m.floor);
        self.add_mesh("Floor", floor, Vector3::new(0.0, fy, mid_z));

        // Wände (Putz)
        let cy = fy + wall_h * 0.5;
        self.add_box("WallN", Vector3::new(side * 2.0, wall_h, 1.0), Vector3::new(0.0, cy, north - 0.5), &m.plaster);
        self.add_box("WallS", Vector3::new(side * 2.0, wall_h, 1.0), Vector3::new(0.0, cy, south + 0.5), &m.plaster);
        self.add_box("WallW", Vector3::new(1.0, wall_h, depth), Vector3::new(-side - 0.5, cy, mid_z), &m.plaster);
        self.add_box("WallE", Vector3::new(1.0, wall_h, depth), Vector3::new(side + 0.5, cy, mid_z), &m.plaster);

        // Fachwerk an der Nordwand: Ständer, Riegel, Streben
        let z = north + 0.3;
        for i in 0..=8 {
            let x = -side + i as f32 * (side * 2.0 / 8.0);
            if x.abs() < 9.0 {
                continue; // Platz für den Kamin
            }
            self.add_box("Post", Vector3::new(1.0, wall_h, 0.8), Vector3::new(x, cy, z), &m.dark_wood);
        }
        for y in [fy + 0.6, fy + 9.0, fy + 20.0] {
            self.add_box("Rail", Vector3::new(side * 2.0, 0.9, 0.9), Vector3::new(0.0, y, z + 0.1), &m.dark_wood);
        }
        for (x0, dir) in [(-21.25, 1.0), (21.25, -1.0)] {
            let mut brace = self.add_box("Brace", Vector3::new(0.7, 12.0, 0.6), Vector3::new(x0, fy + 14.5, z + 0.2), &m.dark_wood);
            brace.set_rotation(Vector3::new(0.0, 0.0, dir * 0.6));
        }
        // Fußleisten an den Seitenwänden
        for x in [-side + 0.3, side - 0.3] {
            self.add_box("Skirting", Vector3::new(0.8, 1.2, depth), Vector3::new(x, fy + 0.6, mid_z), &m.dark_wood);
        }

        self.build_fireplace(m, north);
        self.build_shelves(m, north);
    }

    fn build_fireplace(&mut self, m: &Materials, north: f32) {
        let fy = self.floor_y();
        let z = north + 2.0;
        // Steinumrandung: Seiten, Sturz, Esse, Kaminsims
        self.add_box("HearthL", Vector3::new(4.0, 14.0, 4.0), Vector3::new(-7.0, fy + 7.0, z), &m.stone);
        self.add_box("HearthR", Vector3::new(4.0, 14.0, 4.0), Vector3::new(7.0, fy + 7.0, z), &m.stone);
        self.add_box("Lintel", Vector3::new(18.0, 3.0, 4.4), Vector3::new(0.0, fy + 12.5, z), &m.stone);
        self.add_box("Mantel", Vector3::new(20.0, 0.8, 5.2), Vector3::new(0.0, fy + 14.4, z + 0.3), &m.dark_wood);
        self.add_box("Chimney", Vector3::new(14.0, 16.0, 3.0), Vector3::new(0.0, fy + 22.8, z - 0.5), &m.stone);
        self.add_box("FireBack", Vector3::new(10.0, 11.0, 0.6), Vector3::new(0.0, fy + 5.5, z - 1.6), &m.iron);
        self.add_box("HearthFloor", Vector3::new(18.0, 0.5, 7.0), Vector3::new(0.0, fy + 0.25, z + 1.5), &m.stone);

        // Holzscheite
        for (x, rot, y) in [(-1.6, 0.3, 1.1), (1.4, -0.25, 1.1), (0.0, 0.05, 2.0)] {
            let mut log = self.add_cylinder("Log", (0.7, 0.75), 7.0, Vector3::new(x, fy + y, z), &m.dark_wood);
            log.set_rotation(Vector3::new(0.0, rot, std::f32::consts::FRAC_PI_2));
        }
        // Flammen und Glut
        let cols = [
            Color::from_rgb(1.0, 0.35, 0.08),
            Color::from_rgb(1.0, 0.55, 0.15),
            Color::from_rgb(1.0, 0.75, 0.3),
        ];
        for (k, (x, h, r)) in [
            (-2.6, 2.2, 0.5),
            (-1.7, 3.4, 0.6),
            (-0.8, 2.6, 0.45),
            (0.0, 4.0, 0.65),
            (0.9, 3.0, 0.5),
            (1.8, 3.6, 0.55),
            (2.7, 2.0, 0.45),
        ]
        .into_iter()
        .enumerate()
        {
            let zz = z + 0.2 + (k % 2) as f32 * 0.6;
            self.add_flame(Vector3::new(x, fy + 2.0 + h * 0.4, zz), r, h, cols[k % 3], 3.0);
        }
        let mut embers = self.add_box(
            "Embers",
            Vector3::new(8.0, 0.4, 3.0),
            Vector3::new(0.0, fy + 0.7, z),
            &glow(Color::from_rgb(1.0, 0.25, 0.05), 3.0),
        );
        embers.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        self.add_flicker(Vector3::new(0.0, fy + 5.0, z + 4.0), Color::from_rgb(1.0, 0.5, 0.2), 8.0, 45.0, 0.3, true);

        // Kesselhaken mit Topf
        self.add_box("Hook", Vector3::new(0.25, 4.0, 0.25), Vector3::new(4.0, fy + 9.0, z), &m.iron);
        let mut pot = SphereMesh::new_gd();
        pot.set_radius(1.6);
        pot.set_height(2.6);
        pot.set_material(&m.iron);
        self.add_mesh("Pot", pot, Vector3::new(4.0, fy + 6.2, z));
    }

    fn build_shelves(&mut self, m: &Materials, north: f32) {
        let fy = self.floor_y();
        let z = north + 1.4;
        let bottle_cols = [
            Color::from_rgb(0.1, 0.3, 0.12),
            Color::from_rgb(0.35, 0.12, 0.05),
            Color::from_rgb(0.12, 0.15, 0.3),
            Color::from_rgb(0.5, 0.4, 0.15),
        ];
        for side in [-1.0f32, 1.0] {
            let cx = side * 21.0;
            for (row, y) in [fy + 11.0, fy + 16.0].into_iter().enumerate() {
                self.add_box("Shelf", Vector3::new(10.0, 0.5, 2.4), Vector3::new(cx, y, z), &m.dark_wood);
                for i in 0..6usize {
                    let x = cx - 4.0 + i as f32 * 1.6;
                    let k = (i + row * 2 + usize::from(side > 0.0)) % 4;
                    if (i + row) % 3 == 2 {
                        self.add_cylinder("Mug", (0.55, 0.6), 1.6, Vector3::new(x, y + 1.05, z), &m.pewter);
                    } else {
                        let glass = solid(bottle_cols[k], 0.15, 0.2);
                        self.add_cylinder("Bottle", (0.45, 0.5), 2.4, Vector3::new(x, y + 1.45, z), &glass);
                        self.add_cylinder("Neck", (0.15, 0.2), 1.0, Vector3::new(x, y + 3.1, z), &glass);
                    }
                }
            }
            // Wandleuchter zwischen Kamin und Regal
            let sconce = Vector3::new(side * 13.0, fy + 15.0, north + 1.0);
            self.add_box("Sconce", Vector3::new(0.4, 2.0, 1.2), sconce, &m.iron);
            self.add_cylinder("SconceCandle", (0.3, 0.3), 1.6, sconce + Vector3::new(0.0, 1.5, 0.3), &m.wax);
            self.add_flame(sconce + Vector3::new(0.0, 2.6, 0.3), 0.18, 0.6, Color::from_rgb(1.0, 0.72, 0.3), 6.0);
            self.add_flicker(sconce + Vector3::new(0.0, 3.0, 1.5), Color::from_rgb(1.0, 0.65, 0.3), 2.5, 18.0, 0.2, false);
        }
    }

    // ------------------------------------------------------------ Möbel

    fn build_furniture(&mut self, m: &Materials) {
        let fy = self.floor_y();
        let ts = self.table_size;

        // Bank auf der Gegnerseite
        let bench_z = -ts.y * 0.5 - 4.0;
        self.add_box("Bench", Vector3::new(16.0, 0.8, 3.2), Vector3::new(0.0, fy + 4.6, bench_z), &m.oak);
        for x in [-6.5, 6.5] {
            self.add_box("BenchLeg", Vector3::new(0.8, 4.2, 2.6), Vector3::new(x, fy + 2.1, bench_z), &m.dark_wood);
        }

        // Hocker links und rechts vom Tisch
        for (x, z) in [(-ts.x * 0.5 - 4.5, 1.5), (ts.x * 0.5 + 4.5, -1.0)] {
            self.add_cylinder("Stool", (1.8, 1.8), 0.6, Vector3::new(x, fy + 4.6, z), &m.oak);
            for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let mut leg = self.add_cylinder(
                    "StoolLeg",
                    (0.22, 0.25),
                    4.5,
                    Vector3::new(x + dx * 0.95, fy + 2.2, z + dz * 0.95),
                    &m.dark_wood,
                );
                leg.set_rotation(Vector3::new(dz * 0.12, 0.0, -dx * 0.12));
            }
        }

        // Fässer (stehend und liegend) mit Eisenringen
        for (x, z, r, lying) in [
            (-22.0, -18.0, 3.4, false),
            (-15.5, -19.5, 3.0, false),
            (24.0, -16.0, 3.4, false),
            (17.0, -19.0, 3.2, true),
        ] {
            let h = r * 2.6;
            let (pos, rot) = if lying {
                (Vector3::new(x, fy + r, z), Vector3::new(0.0, 0.4, std::f32::consts::FRAC_PI_2))
            } else {
                (Vector3::new(x, fy + h * 0.5, z), Vector3::ZERO)
            };
            let mut b = self.add_cylinder("Barrel", (r * 0.9, r * 0.9), h, pos, &m.barrel_wood);
            b.set_rotation(rot);
            // Bauch + Ringe als Kinder, damit sie mitrotieren
            for (y, rr, mat) in [
                (-h * 0.36, r * 0.92, &m.iron),
                (h * 0.36, r * 0.92, &m.iron),
                (0.0, r, &m.barrel_wood),
            ] {
                let mut ring = TorusMesh::new_gd();
                ring.set_inner_radius(rr - 0.15);
                ring.set_outer_radius(rr + 0.12);
                ring.set_material(mat);
                let mut mi = MeshInstance3D::new_alloc();
                mi.set_mesh(&ring);
                mi.set_position(Vector3::new(0.0, y, 0.0));
                b.add_child(&mi);
            }
        }
        // Kisten
        for (x, z, s, y_off, rot) in [
            (-26.0, -10.0, 5.0, 0.0, 0.3),
            (-26.3, -10.2, 3.6, 5.0, -0.2),
            (27.0, -6.0, 4.6, 0.0, 0.6),
        ] {
            let mut c = self.add_box("Crate", Vector3::splat(s), Vector3::new(x, fy + y_off + s * 0.5, z), &m.crate_wood);
            c.set_rotation(Vector3::new(0.0, rot, 0.0));
        }
    }

    // ------------------------------------------------------------ Tisch-Requisiten

    fn build_table_props(&mut self, m: &Materials) {
        let ts = self.table_size;
        let cx = ts.x * 0.5 - 0.5;
        let cz = ts.y * 0.5 - 0.5;

        // Kerzen in den Ecken der Gegnerseite, eine beim Spieler links
        for (x, z, h) in [(-cx, -cz, 1.6), (cx, -cz, 1.2), (-cx, cz - 0.2, 1.0)] {
            self.add_candle(m, Vector3::new(x, 0.0, z), h);
        }
        // Krüge: beim Spieler auf dem rechten Hocker, beim Gegner am Tischrand
        self.add_tankard(m, Vector3::new(ts.x * 0.5 + 4.5, -self.table_height + 4.9, -1.0));
        self.add_tankard(m, Vector3::new(-cx + 1.2, 0.0, -cz + 0.4));

        // Münzen beim Gegner
        for (k, (dx, dz, n)) in [(0.0, 0.0, 6), (0.25, 0.15, 3), (-0.12, 0.28, 2)].into_iter().enumerate() {
            for i in 0..n {
                let jitter = ((i * 7 + k * 3) % 5) as f32 * 0.008;
                let pos = Vector3::new(cx - 1.4 + dx + jitter, 0.012 + i as f32 * 0.025, -cz + 0.2 + dz);
                self.add_cylinder("Coin", (0.11, 0.11), 0.024, pos, &m.gold);
            }
        }
    }

    fn add_candle(&mut self, m: &Materials, pos: Vector3, height: f32) {
        self.add_cylinder("CandlePlate", (0.32, 0.36), 0.08, pos + Vector3::new(0.0, 0.04, 0.0), &m.brass);
        self.add_cylinder("Candle", (0.14, 0.15), height, pos + Vector3::new(0.0, 0.08 + height * 0.5, 0.0), &m.wax);
        let flame_pos = pos + Vector3::new(0.0, 0.08 + height + 0.14, 0.0);
        self.add_flame(flame_pos, 0.06, 0.24, Color::from_rgb(1.0, 0.72, 0.3), 6.0);
        self.add_flicker(flame_pos + Vector3::new(0.0, 0.3, 0.0), Color::from_rgb(1.0, 0.66, 0.34), 1.3, 9.0, 0.18, false);
    }

    fn add_tankard(&mut self, m: &Materials, pos: Vector3) {
        self.add_cylinder("Tankard", (0.45, 0.5), 1.4, pos + Vector3::new(0.0, 0.7, 0.0), &m.pewter);
        self.add_cylinder("Foam", (0.43, 0.43), 0.08, pos + Vector3::new(0.0, 1.38, 0.0), &m.foam);
        let mut handle = TorusMesh::new_gd();
        handle.set_inner_radius(0.24);
        handle.set_outer_radius(0.34);
        handle.set_material(&m.pewter);
        let mut h = self.add_mesh("Handle", handle, pos + Vector3::new(0.55, 0.75, 0.0));
        h.set_rotation(Vector3::new(std::f32::consts::FRAC_PI_2, 0.0, 0.0));
    }
}

// ---------------------------------------------------------------- Materialien

fn solid(color: Color, roughness: f32, metallic: f32) -> Gd<Material> {
    let mut m = StandardMaterial3D::new_gd();
    m.set_albedo(color);
    m.set_roughness(roughness);
    m.set_metallic(metallic);
    m.upcast()
}

/// Triplanar in Weltkoordinaten projizierte Textur; `per_unit` = Wiederholungen pro Einheit.
fn textured(tex: &Gd<Texture2D>, per_unit: f32, roughness: f32) -> Gd<Material> {
    let mut m = StandardMaterial3D::new_gd();
    m.set_texture(TextureParam::ALBEDO, tex);
    m.set_flag(Flags::UV1_USE_TRIPLANAR, true);
    m.set_flag(Flags::UV1_USE_WORLD_TRIPLANAR, true);
    m.set_uv1_scale(Vector3::splat(per_unit));
    m.set_roughness(roughness);
    m.upcast()
}

fn glow(color: Color, energy: f32) -> Gd<Material> {
    let mut m = StandardMaterial3D::new_gd();
    m.set_shading_mode(ShadingMode::UNSHADED);
    m.set_albedo(color);
    m.set_feature(Feature::EMISSION, true);
    m.set_emission(color);
    m.set_emission_energy_multiplier(energy);
    m.upcast()
}
