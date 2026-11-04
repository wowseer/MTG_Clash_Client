//! Shader der 3D-Knöpfe ([`Button3D`](super::button3d::Button3D)): Nachbau der Blender-Materialien
//! aus `Assets/Buttons/*.blend` (Knopf „Button_Feuer“/„Button_Blau“ und „Button_Glow“), inklusive
//! ihrer Treiber, damit sich die Oberfläche bewegt wie in Blender.
//!
//! Abbildung der Blender-Knoten (Objektkoordinaten des Knopfes, Bildausschnitt 5,3 × 1,4832):
//! - Form: abgerundetes Rechteck, `d = |max(|p| − (1,795; 0,045), 0)| + min(max(q.x, q.y), 0) − 0,405`
//!   (1,795 = `body_half`, kürzer für kompakte Knöpfe; der Bildausschnitt `canvas` wächst mit);
//!   Deckkraft `clamp(−d / 0,012)` (weiche Kante).
//! - Farbe: senkrechter Verlauf (Color Ramp über `y·1,1111 + 0,5`), darüber zwei 4D-fBm-Rauschen
//!   (Skala 2,2 / Detail 4 / Rauheit 0,55 / Verzerrung 1,2 und 3,5 / 2 / 0,5 / 0,8) an
//!   `p·(0,55; 1,6; 1) + (0; 0; z)`, heller Rand `clamp(1 + d/0,06)^1,8 · (0,35 + 0,75·(1 − t))`.
//! - Treiber (30 fps, Schleife 120 Frames = 4 s): `z = 0,7·sin φ`, `w₁ = 0,7·cos φ`,
//!   `w₂ = 5 + 0,5·sin φ`, Leuchten `0,85 + 0,15·sin 2φ` mit `φ = 2π·frame/120`.
//! - Leuchten: `g = clamp(1 − d/0,32)^2,2`, Farbe `mix(A, B, g)`, Deckkraft `g · Puls · 0,9`.
//!
//! Das Rauschen ist klassisches 4D-Perlin-Rauschen (Gustavson) im fBm-Schema von Blender; es sieht
//! gleich aus, ist aber nicht bitgleich mit Blenders Rauschen ([`PERLIN_FBM`]).

use super::shader_noise::PERLIN_FBM;

/// Shader-Code (Godot Shading Language, `spatial`, unbeleuchtet, transparent).
pub fn button_shader() -> String {
    BUTTON_SHADER.replace("%NOISE%", PERLIN_FBM)
}

const BUTTON_SHADER: &str = r#"
shader_type spatial;
render_mode unshaded, blend_mix, depth_draw_never, cull_back, shadows_disabled;

// Farben je Stil (linear, wie in Blender)
uniform vec4 ramp0;
uniform vec4 ramp1;
uniform vec4 ramp2;
uniform vec4 ramp3;
uniform vec4 ramp_pos = vec4(0.0, 0.18, 0.7, 1.0);
uniform vec4 streak_color;   // Mix  B (helle Schlieren)
uniform vec4 shade_color;    // Mix.001 B (dunkle Flecken)
uniform vec4 rim_color;      // Mix.002 B (heller Rand)
uniform vec4 glow_inner;     // Glow Mix A
uniform vec4 glow_outer;     // Glow Mix B
// Bedienung
uniform float glow_boost = 1.0;   // Hover: > 1
uniform float time_offset = 0.0;  // damit nicht alle Knöpfe gleich laufen
uniform float disabled = 0.0;     // 1 = ausgegraut

// Bildausschnitt (Blender-Einheiten) und halbe Länge des geraden Knopfkörpers (1,795 = Blender)
uniform vec2 canvas = vec2(5.3, 1.4832);
uniform float body_half = 1.795;

%NOISE%
// Blenders Noise-Texture-Knoten (4D, fBm, normalisiert) mit Verzerrung
float noise_tex(vec3 co, float w, float scale, int octaves, float roughness, float distortion) {
    vec4 p = vec4(co, w) * scale;
    p += vec4(cnoise(p + vec4(131.7, 47.3, 211.9, 83.1)),
              cnoise(p + vec4(19.4, 173.2, 61.8, 147.5)),
              cnoise(p + vec4(97.6, 29.1, 157.3, 11.9)),
              cnoise(p + vec4(201.3, 113.7, 7.4, 189.2))) * distortion;
    return fbm(p, octaves, roughness);
}

vec3 ramp(float t) {
    vec3 c = ramp0.rgb;
    c = mix(c, ramp1.rgb, clamp((t - ramp_pos.x) / max(ramp_pos.y - ramp_pos.x, 1e-4), 0.0, 1.0));
    c = mix(c, ramp2.rgb, clamp((t - ramp_pos.y) / max(ramp_pos.z - ramp_pos.y, 1e-4), 0.0, 1.0));
    c = mix(c, ramp3.rgb, clamp((t - ramp_pos.z) / max(ramp_pos.w - ramp_pos.z, 1e-4), 0.0, 1.0));
    return c;
}

void fragment() {
    vec2 o = vec2(UV.x - 0.5, 0.5 - UV.y) * canvas;
    // abgerundetes Rechteck (SDF wie in Blender)
    vec2 q = abs(o) - vec2(body_half, 0.045);
    float d = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - 0.405;

    // Treiber: 30 fps, 120 Frames
    float frame = (TIME + time_offset) * 30.0 + 1.0;
    float phi = 6.28318530718 * frame / 120.0;
    float z = 0.7 * sin(phi);
    float w1 = 0.7 * cos(phi);
    float w2 = 5.0 + 0.5 * sin(phi);
    float pulse = 0.85 + 0.15 * sin(2.0 * phi);

    // Knopf
    float face_a = clamp(-d / 0.012, 0.0, 1.0);
    vec3 col = vec3(0.0);
    if (face_a > 0.0) {
        float t = o.y * 1.1111 + 0.5;
        col = ramp(t);
        vec3 pn = vec3(o.x * 0.55, o.y * 1.6, z);
        float n1 = noise_tex(pn, w1, 2.2, 5, 0.55, 1.2);
        float streak = clamp((n1 - 0.52) / 0.2, 0.0, 1.0) * 0.5;
        col = mix(col, streak_color.rgb, streak);
        float n2 = noise_tex(pn, w2, 3.5, 3, 0.5, 0.8);
        col = mix(col, shade_color.rgb, clamp(n2 - 0.5, 0.0, 1.0) * 0.9);
        float rim = pow(clamp(1.0 + d / 0.06, 0.0, 1.0), 1.8);
        float vert = clamp(1.0 - t, 0.0, 1.0) * 0.75 + 0.35;
        col = mix(col, rim_color.rgb, clamp(rim * vert, 0.0, 1.0));
    }

    // Leuchten (hinter dem Knopf)
    float g = pow(clamp(1.0 - d / 0.32, 0.0, 1.0), 2.2);
    float glow_a = clamp(g * pulse * 0.9 * glow_boost, 0.0, 1.0);
    vec3 glow_col = mix(glow_inner.rgb, glow_outer.rgb, g);

    float a = face_a + glow_a * (1.0 - face_a);
    vec3 c = (col * face_a + glow_col * glow_a * (1.0 - face_a)) / max(a, 1e-4);
    float grey = dot(c, vec3(0.299, 0.587, 0.114));
    c = mix(c, vec3(grey) * 0.6, disabled);
    ALBEDO = c;
    ALPHA = a * (1.0 - 0.6 * disabled);
}
"#;
