//! TargetArrows: gebogene, leuchtende Zielpfeile über dem Spieltisch.
//!
//! - Angreifer: Pfeil nach vorn (Richtung Gegner), orange.
//! - Blocker: Pfeil vom Blocker auf seinen Angreifer, blau.
//! - Zielwahl („wähle …“, z. B. Blocker zuweisen, Ausrüstung anlegen): goldener Pfeil von der
//!   wählenden Karte zum Mauszeiger bzw. zur Karte darunter – beim Ziehen oder nach einem Klick.
//!
//! Jeder Pfeil ist eine quadratische Bézier-Kurve, die sich über den Tisch wölbt: ein flaches
//! Band (Breite [`SHAFT_WIDTH`]) mit Pfeilspitze, darauf laufen Chevrons zur Spitze hin. Alle
//! Pfeile liegen in einem `ImmediateMesh`, das jedes Frame aus den übergebenen
//! [`ArrowSpec`]s neu gebaut wird (wenige Pfeile – günstig). Der Tiefentest bleibt aktiv:
//! Gezoomte Karten vor der Kamera verdecken die Pfeile.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::mesh::PrimitiveType;
use godot::classes::{ImmediateMesh, INode3D, MeshInstance3D, Shader, ShaderMaterial};
use godot::prelude::*;

/// Breite des Pfeilschafts (Einheiten; Karte = 0,63 breit).
const SHAFT_WIDTH: f32 = 0.2;
const HEAD_WIDTH: f32 = 0.52;
const HEAD_LENGTH: f32 = 0.42;
/// Abschnitte der Kurve.
const SEGMENTS: usize = 28;
/// Höhe der Endpunkte über dem Tisch (über flach liegenden Karten).
const END_LIFT: f32 = 0.06;

/// Pfeil-Shader: Farbe aus der Vertexfarbe, hellerer Kern, laufende Chevrons Richtung Spitze,
/// weiches Einblenden am Anfang und weiche Ränder.
const SHADER: &str = r#"
shader_type spatial;
render_mode unshaded, blend_mix, cull_disabled, shadows_disabled, depth_draw_never;

void fragment() {
    float across = abs(UV.y - 0.5) * 2.0;
    bool head = UV2.x > 0.5;
    // Chevrons, die zur Spitze laufen
    float stripe = fract(UV.x * 3.0 - across * 0.6 - TIME * 1.6);
    float chevron = head ? 1.0 : 0.7 + 0.3 * smoothstep(0.35, 0.5, stripe) * smoothstep(0.85, 0.7, stripe);
    vec3 col = mix(COLOR.rgb, vec3(1.0), (1.0 - smoothstep(0.0, 0.3, across)) * 0.2);
    float edge = head ? 1.0 : smoothstep(1.0, 0.75, across);
    float start = smoothstep(0.0, 0.45, UV.x);
    ALBEDO = col * chevron * 0.85;
    ALPHA = COLOR.a * edge * start * 0.95;
}
"#;

/// Ein Pfeil: von `from` nach `to` (Weltkoordinaten, Höhe wird auf den Tisch gelegt) in `color`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowSpec {
    pub from: Vector3,
    pub to: Vector3,
    pub color: Color,
}

/// Zeichnet gebogene, leuchtende Zielpfeile über dem Tisch (Angriff, Block, Zielwahl).
/// Wird vom [TableController] erzeugt und jedes Frame mit den aktuellen Pfeilen gefüttert;
/// Höhe des Tisches = y dieses Nodes.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct TargetArrows {
    mesh: Gd<ImmediateMesh>,
    material: Gd<ShaderMaterial>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for TargetArrows {
    fn init(base: Base<Node3D>) -> Self {
        let mut shader = Shader::new_gd();
        shader.set_code(SHADER);
        let mut material = ShaderMaterial::new_gd();
        material.set_shader(&shader);
        // Über Tisch und flach liegenden Karten zeichnen
        material.set_render_priority(10);
        Self { mesh: ImmediateMesh::new_gd(), material, base }
    }

    fn ready(&mut self) {
        let mut mi = MeshInstance3D::new_alloc();
        mi.set_name("Arrows");
        mi.set_mesh(&self.mesh);
        mi.set_material_override(&self.material);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        self.base_mut().add_child(&mi);
    }
}

#[godot_api]
impl TargetArrows {
    /// Setzt Pfeile aus Skripten: `points` enthält je zwei Punkte (Start, Ziel; Welt) pro Pfeil,
    /// `colors` eine Farbe pro Pfeil (fehlt sie: Gold). Der [TableController] überschreibt die
    /// Pfeile jedes Frame, solange er läuft.
    #[func]
    pub fn set_arrow_pairs(&mut self, points: PackedVector3Array, colors: PackedColorArray) {
        let pts = points.as_slice();
        let specs: Vec<ArrowSpec> = pts
            .chunks_exact(2)
            .enumerate()
            .map(|(i, p)| ArrowSpec {
                from: p[0],
                to: p[1],
                color: colors.as_slice().get(i).copied().unwrap_or(Color::from_rgb(1.0, 0.85, 0.35)),
            })
            .collect();
        self.set_arrows(&specs);
    }
}

impl TargetArrows {
    /// Ersetzt alle Pfeile durch `arrows` und baut das Mesh neu.
    pub fn set_arrows(&mut self, arrows: &[ArrowSpec]) {
        self.mesh.clear_surfaces();
        if arrows.is_empty() {
            return;
        }
        let table_y = self.base().get_global_position().y;
        let inv = self.base().get_global_transform().affine_inverse();
        self.mesh.surface_begin(PrimitiveType::TRIANGLES);
        for a in arrows {
            let from = Vector3::new(a.from.x, table_y + END_LIFT, a.from.z);
            let to = Vector3::new(a.to.x, table_y + END_LIFT, a.to.z);
            build_arrow(&mut self.mesh, inv * from, inv * to, a.color);
        }
        self.mesh.surface_end();
    }
}

/// Punkt der Bézier-Kurve (Bogen über dem Tisch, Höhe wächst mit der Länge).
fn curve(from: Vector3, to: Vector3, t: f32) -> Vector3 {
    let len = (to - from).length();
    let mid = (from + to) * 0.5 + Vector3::UP * (0.25 + 0.18 * len);
    let u = 1.0 - t;
    from * (u * u) + mid * (2.0 * u * t) + to * (t * t)
}

fn build_arrow(mesh: &mut Gd<ImmediateMesh>, from: Vector3, to: Vector3, color: Color) {
    if (to - from).length() < 0.15 {
        return;
    }
    // Kurve abtasten, Bogenlänge aufsummieren
    let pts: Vec<Vector3> = (0..=SEGMENTS).map(|i| curve(from, to, i as f32 / SEGMENTS as f32)).collect();
    let mut dist = vec![0.0f32];
    for w in pts.windows(2) {
        dist.push(dist.last().unwrap() + (w[1] - w[0]).length());
    }
    let total = *dist.last().unwrap();
    let head_start = (total - HEAD_LENGTH).max(total * 0.4);

    // Seitenvektor: waagerecht, quer zur Laufrichtung (Band liegt flach – gut sichtbar von oben)
    let side_at = |i: usize| {
        let tangent = pts[(i + 1).min(SEGMENTS)] - pts[i.saturating_sub(1)];
        let s = tangent.cross(Vector3::UP);
        if s.length_squared() < 1e-6 { Vector3::RIGHT } else { s.normalized() }
    };
    let mut vertex = |p: Vector3, u: f32, v: f32, head: bool| {
        mesh.surface_set_color(color);
        mesh.surface_set_uv(Vector2::new(u, v));
        mesh.surface_set_uv2(Vector2::new(if head { 1.0 } else { 0.0 }, 0.0));
        mesh.surface_add_vertex(p);
    };

    // Schaft bis zum Beginn der Spitze
    let mut prev: Option<(Vector3, Vector3, f32)> = None;
    let mut base = None;
    for i in 0..=SEGMENTS {
        let (p, d) = if dist[i] <= head_start {
            (pts[i], dist[i])
        } else {
            // Genau am Spitzenansatz enden (zwischen i-1 und i interpolieren)
            let f = (head_start - dist[i - 1]) / (dist[i] - dist[i - 1]).max(1e-5);
            (pts[i - 1].lerp(pts[i], f), head_start)
        };
        let half = side_at(i) * (SHAFT_WIDTH * 0.5);
        let (l, r) = (p - half, p + half);
        if let Some((pl, pr, pd)) = prev {
            vertex(pl, pd, 0.0, false);
            vertex(pr, pd, 1.0, false);
            vertex(r, d, 1.0, false);
            vertex(pl, pd, 0.0, false);
            vertex(r, d, 1.0, false);
            vertex(l, d, 0.0, false);
        }
        prev = Some((l, r, d));
        if dist[i] > head_start {
            base = Some((p, side_at(i)));
            break;
        }
    }

    // Spitze: Dreieck vom Ansatz bis zum Endpunkt
    let (bp, side) = base.unwrap_or((pts[SEGMENTS - 1], side_at(SEGMENTS - 1)));
    let half = side * (HEAD_WIDTH * 0.5);
    vertex(bp - half, head_start, 0.0, true);
    vertex(bp + half, head_start, 1.0, true);
    vertex(to, total, 0.5, true);
}
