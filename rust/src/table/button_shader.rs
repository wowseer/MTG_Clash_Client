//! Shader der 3D-Knöpfe ([`Button3D`](super::button3d::Button3D)): Nachbau der Blender-Materialien
//! aus `Assets/Buttons/*.blend` (Knopf „Button_Feuer“/„Button_Blau“ und „Button_Glow“), inklusive
//! ihrer Treiber, damit sich die Oberfläche bewegt wie in Blender.
//!
//! Abbildung der Blender-Knoten (Objektkoordinaten des Knopfes, Bildausschnitt 5,3 × 1,4832):
//! - Form: abgerundetes Rechteck, `d = |max(|p| − (1,795; 0,045), 0)| + min(max(q.x, q.y), 0) − 0,405`;
//!   Deckkraft `clamp(−d / 0,012)` (weiche Kante).
//! - Farbe: senkrechter Verlauf (Color Ramp über `y·1,1111 + 0,5`), darüber zwei 4D-fBm-Rauschen
//!   (Skala 2,2 / Detail 4 / Rauheit 0,55 / Verzerrung 1,2 und 3,5 / 2 / 0,5 / 0,8) an
//!   `p·(0,55; 1,6; 1) + (0; 0; z)`, heller Rand `clamp(1 + d/0,06)^1,8 · (0,35 + 0,75·(1 − t))`.
//! - Treiber (30 fps, Schleife 120 Frames = 4 s): `z = 0,7·sin φ`, `w₁ = 0,7·cos φ`,
//!   `w₂ = 5 + 0,5·sin φ`, Leuchten `0,85 + 0,15·sin 2φ` mit `φ = 2π·frame/120`.
//! - Leuchten: `g = clamp(1 − d/0,32)^2,2`, Farbe `mix(A, B, g)`, Deckkraft `g · Puls · 0,9`.
//!
//! Das Rauschen ist klassisches 4D-Perlin-Rauschen (Gustavson) im fBm-Schema von Blender; es sieht
//! gleich aus, ist aber nicht bitgleich mit Blenders Rauschen.

/// Shader-Code (Godot Shading Language, `spatial`, unbeleuchtet, transparent).
pub const BUTTON_SHADER: &str = r#"
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

const vec2 CANVAS = vec2(5.3, 1.4832);

vec4 mod289(vec4 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec4 permute(vec4 x) { return mod289(((x * 34.0) + 10.0) * x); }
vec4 taylor_inv_sqrt(vec4 r) { return 1.79284291400159 - 0.85373472095314 * r; }
vec4 fade(vec4 t) { return t * t * t * (t * (t * 6.0 - 15.0) + 10.0); }

// Klassisches 4D-Perlin-Rauschen (Stefan Gustavson, MIT), Bereich etwa −1 … 1
float cnoise(vec4 P) {
    vec4 Pi0 = floor(P);
    vec4 Pi1 = Pi0 + 1.0;
    Pi0 = mod289(Pi0);
    Pi1 = mod289(Pi1);
    vec4 Pf0 = fract(P);
    vec4 Pf1 = Pf0 - 1.0;
    vec4 ix = vec4(Pi0.x, Pi1.x, Pi0.x, Pi1.x);
    vec4 iy = vec4(Pi0.yy, Pi1.yy);
    vec4 iz0 = vec4(Pi0.z);
    vec4 iz1 = vec4(Pi1.z);
    vec4 iw0 = vec4(Pi0.w);
    vec4 iw1 = vec4(Pi1.w);
    vec4 ixy = permute(permute(ix) + iy);
    vec4 ixy0 = permute(ixy + iz0);
    vec4 ixy1 = permute(ixy + iz1);
    vec4 ixy00 = permute(ixy0 + iw0);
    vec4 ixy01 = permute(ixy0 + iw1);
    vec4 ixy10 = permute(ixy1 + iw0);
    vec4 ixy11 = permute(ixy1 + iw1);
    vec4 gx00 = ixy00 / 7.0;
    vec4 gy00 = floor(gx00) / 7.0;
    vec4 gz00 = floor(gy00) / 6.0;
    gx00 = fract(gx00) - 0.5;
    gy00 = fract(gy00) - 0.5;
    gz00 = fract(gz00) - 0.5;
    vec4 gw00 = vec4(0.75) - abs(gx00) - abs(gy00) - abs(gz00);
    vec4 sw00 = step(gw00, vec4(0.0));
    gx00 -= sw00 * (step(0.0, gx00) - 0.5);
    gy00 -= sw00 * (step(0.0, gy00) - 0.5);
    vec4 gx01 = ixy01 / 7.0;
    vec4 gy01 = floor(gx01) / 7.0;
    vec4 gz01 = floor(gy01) / 6.0;
    gx01 = fract(gx01) - 0.5;
    gy01 = fract(gy01) - 0.5;
    gz01 = fract(gz01) - 0.5;
    vec4 gw01 = vec4(0.75) - abs(gx01) - abs(gy01) - abs(gz01);
    vec4 sw01 = step(gw01, vec4(0.0));
    gx01 -= sw01 * (step(0.0, gx01) - 0.5);
    gy01 -= sw01 * (step(0.0, gy01) - 0.5);
    vec4 gx10 = ixy10 / 7.0;
    vec4 gy10 = floor(gx10) / 7.0;
    vec4 gz10 = floor(gy10) / 6.0;
    gx10 = fract(gx10) - 0.5;
    gy10 = fract(gy10) - 0.5;
    gz10 = fract(gz10) - 0.5;
    vec4 gw10 = vec4(0.75) - abs(gx10) - abs(gy10) - abs(gz10);
    vec4 sw10 = step(gw10, vec4(0.0));
    gx10 -= sw10 * (step(0.0, gx10) - 0.5);
    gy10 -= sw10 * (step(0.0, gy10) - 0.5);
    vec4 gx11 = ixy11 / 7.0;
    vec4 gy11 = floor(gx11) / 7.0;
    vec4 gz11 = floor(gy11) / 6.0;
    gx11 = fract(gx11) - 0.5;
    gy11 = fract(gy11) - 0.5;
    gz11 = fract(gz11) - 0.5;
    vec4 gw11 = vec4(0.75) - abs(gx11) - abs(gy11) - abs(gz11);
    vec4 sw11 = step(gw11, vec4(0.0));
    gx11 -= sw11 * (step(0.0, gx11) - 0.5);
    gy11 -= sw11 * (step(0.0, gy11) - 0.5);
    vec4 g0000 = vec4(gx00.x, gy00.x, gz00.x, gw00.x);
    vec4 g1000 = vec4(gx00.y, gy00.y, gz00.y, gw00.y);
    vec4 g0100 = vec4(gx00.z, gy00.z, gz00.z, gw00.z);
    vec4 g1100 = vec4(gx00.w, gy00.w, gz00.w, gw00.w);
    vec4 g0010 = vec4(gx10.x, gy10.x, gz10.x, gw10.x);
    vec4 g1010 = vec4(gx10.y, gy10.y, gz10.y, gw10.y);
    vec4 g0110 = vec4(gx10.z, gy10.z, gz10.z, gw10.z);
    vec4 g1110 = vec4(gx10.w, gy10.w, gz10.w, gw10.w);
    vec4 g0001 = vec4(gx01.x, gy01.x, gz01.x, gw01.x);
    vec4 g1001 = vec4(gx01.y, gy01.y, gz01.y, gw01.y);
    vec4 g0101 = vec4(gx01.z, gy01.z, gz01.z, gw01.z);
    vec4 g1101 = vec4(gx01.w, gy01.w, gz01.w, gw01.w);
    vec4 g0011 = vec4(gx11.x, gy11.x, gz11.x, gw11.x);
    vec4 g1011 = vec4(gx11.y, gy11.y, gz11.y, gw11.y);
    vec4 g0111 = vec4(gx11.z, gy11.z, gz11.z, gw11.z);
    vec4 g1111 = vec4(gx11.w, gy11.w, gz11.w, gw11.w);
    vec4 norm00 = taylor_inv_sqrt(vec4(dot(g0000, g0000), dot(g0100, g0100), dot(g1000, g1000), dot(g1100, g1100)));
    g0000 *= norm00.x; g0100 *= norm00.y; g1000 *= norm00.z; g1100 *= norm00.w;
    vec4 norm01 = taylor_inv_sqrt(vec4(dot(g0001, g0001), dot(g0101, g0101), dot(g1001, g1001), dot(g1101, g1101)));
    g0001 *= norm01.x; g0101 *= norm01.y; g1001 *= norm01.z; g1101 *= norm01.w;
    vec4 norm10 = taylor_inv_sqrt(vec4(dot(g0010, g0010), dot(g0110, g0110), dot(g1010, g1010), dot(g1110, g1110)));
    g0010 *= norm10.x; g0110 *= norm10.y; g1010 *= norm10.z; g1110 *= norm10.w;
    vec4 norm11 = taylor_inv_sqrt(vec4(dot(g0011, g0011), dot(g0111, g0111), dot(g1011, g1011), dot(g1111, g1111)));
    g0011 *= norm11.x; g0111 *= norm11.y; g1011 *= norm11.z; g1111 *= norm11.w;
    float n0000 = dot(g0000, Pf0);
    float n1000 = dot(g1000, vec4(Pf1.x, Pf0.yzw));
    float n0100 = dot(g0100, vec4(Pf0.x, Pf1.y, Pf0.zw));
    float n1100 = dot(g1100, vec4(Pf1.xy, Pf0.zw));
    float n0010 = dot(g0010, vec4(Pf0.xy, Pf1.z, Pf0.w));
    float n1010 = dot(g1010, vec4(Pf1.x, Pf0.y, Pf1.z, Pf0.w));
    float n0110 = dot(g0110, vec4(Pf0.x, Pf1.yz, Pf0.w));
    float n1110 = dot(g1110, vec4(Pf1.xyz, Pf0.w));
    float n0001 = dot(g0001, vec4(Pf0.xyz, Pf1.w));
    float n1001 = dot(g1001, vec4(Pf1.x, Pf0.yz, Pf1.w));
    float n0101 = dot(g0101, vec4(Pf0.x, Pf1.y, Pf0.z, Pf1.w));
    float n1101 = dot(g1101, vec4(Pf1.xy, Pf0.z, Pf1.w));
    float n0011 = dot(g0011, vec4(Pf0.xy, Pf1.zw));
    float n1011 = dot(g1011, vec4(Pf1.x, Pf0.y, Pf1.zw));
    float n0111 = dot(g0111, vec4(Pf0.x, Pf1.yzw));
    float n1111 = dot(g1111, Pf1);
    vec4 fade_xyzw = fade(Pf0);
    vec4 n_0w = mix(vec4(n0000, n1000, n0100, n1100), vec4(n0001, n1001, n0101, n1101), fade_xyzw.w);
    vec4 n_1w = mix(vec4(n0010, n1010, n0110, n1110), vec4(n0011, n1011, n0111, n1111), fade_xyzw.w);
    vec4 n_zw = mix(n_0w, n_1w, fade_xyzw.z);
    vec2 n_yzw = mix(n_zw.xy, n_zw.zw, fade_xyzw.y);
    float n_xyzw = mix(n_yzw.x, n_yzw.y, fade_xyzw.x);
    return 2.2 * n_xyzw;
}

// Blenders fBm (normalisiert auf 0 … 1); octaves = Detail + 1
float fbm(vec4 p, int octaves, float roughness) {
    float fscale = 1.0;
    float amp = 1.0;
    float maxamp = 0.0;
    float sum = 0.0;
    for (int i = 0; i < octaves; i++) {
        sum += cnoise(fscale * p) * amp;
        maxamp += amp;
        amp *= roughness;
        fscale *= 2.0;
    }
    return 0.5 * sum / maxamp + 0.5;
}

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
    vec2 o = vec2(UV.x - 0.5, 0.5 - UV.y) * CANVAS;
    // abgerundetes Rechteck (SDF wie in Blender)
    vec2 q = abs(o) - vec2(1.795, 0.045);
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
