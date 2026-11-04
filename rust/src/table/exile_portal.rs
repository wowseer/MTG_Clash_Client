//! Exil-Strudel: dunkel wirbelnder Wirbel auf dem Tisch unter dem Exil-Stapel (nach dem Video
//! des Users „Exile and Grave“). Ein Shader auf einer flachen Fläche: Rauschen in Polarkoordinaten,
//! das sich langsam dreht und nach innen gezogen wird, außen weich ausgeblendet, in der Mitte ein
//! dunkles Loch. Der Wirbel ist etwas größer als eine Karte im Exil, sodass er um sie herum
//! sichtbar bleibt; ohne Karten sieht man ihn ganz.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::{MeshInstance3D, QuadMesh, Shader, ShaderMaterial};
use godot::prelude::*;

use super::shader_noise::PERLIN_FBM;

/// Durchmesser des Wirbels (Zonen-Einheiten) und Höhe über dem Tisch.
const PORTAL_SIZE: f32 = 2.0;
const PORTAL_LIFT: f32 = 0.004;

const SHADER_HEAD: &str = r#"
shader_type spatial;
render_mode unshaded, blend_mix, cull_disabled, depth_draw_never, shadows_disabled;
"#;

const SHADER_BODY: &str = r#"
void fragment() {
    vec2 p = (UV - 0.5) * 2.0;
    float r = length(p);
    if (r > 1.0) { discard; }
    float a = atan(p.y, p.x);
    float t = TIME * 0.35;
    // Wirbel: Winkel dreht sich nach innen stärker, Muster wird zur Mitte gezogen
    float swirl = a + 2.6 / (r + 0.25) - t * 2.0;
    vec2 q = vec2(cos(swirl), sin(swirl)) * (1.2 + r * 1.8);
    float n = fbm(vec4(q * 1.4, r * 2.0 - t * 1.5, t * 0.4), 4, 0.55);
    float streak = smoothstep(0.52, 0.78, n);
    vec3 deep = vec3(0.015, 0.02, 0.035);
    vec3 mist = vec3(0.1, 0.13, 0.19);
    vec3 glint = vec3(0.42, 0.5, 0.66);
    vec3 col = mix(deep, mist, n * (0.4 + 0.6 * r));
    col = mix(col, glint, streak * 0.35 * smoothstep(0.15, 0.6, r));
    // dunkles Loch in der Mitte, weicher Rand außen
    col *= mix(0.25, 1.0, smoothstep(0.05, 0.45, r));
    float alpha = (1.0 - smoothstep(0.7, 1.0, r)) * 0.92;
    ALBEDO = col;
    ALPHA = alpha;
}
"#;

/// Legt den Exil-Strudel als Kind unter die Exil-Zone `zone` (flach auf dem Tisch, unter den
/// Karten). Mehrfaches Aufrufen legt nur einen an.
pub fn attach_exile_portal(zone: &mut Gd<Node3D>) {
    if zone.get_node_or_null("ExilePortal").is_some() {
        return;
    }
    let mut shader = Shader::new_gd();
    shader.set_code(&format!("{SHADER_HEAD}{PERLIN_FBM}{SHADER_BODY}"));
    let mut mat = ShaderMaterial::new_gd();
    mat.set_shader(&shader);
    mat.set_render_priority(-2);
    let mut quad = QuadMesh::new_gd();
    quad.set_size(Vector2::splat(PORTAL_SIZE));
    quad.set_material(&mat);
    let mut mesh = MeshInstance3D::new_alloc();
    mesh.set_name("ExilePortal");
    mesh.set_mesh(&quad);
    mesh.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    // Quad zeigt nach +Z → flach auf den Tisch legen (Normale nach oben)
    mesh.set_transform(Transform3D::new(
        Basis::from_axis_angle(Vector3::RIGHT, -std::f32::consts::FRAC_PI_2),
        Vector3::new(0.0, PORTAL_LIFT, 0.0),
    ));
    zone.add_child(&mesh);
}
