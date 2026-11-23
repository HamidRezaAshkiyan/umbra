struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    theme_id: u32,
    mode: u32,        // 0 = built-in animated theme, 1 = custom image wallpaper
    image_aspect: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

@group(0) @binding(1)
var wall_tex: texture_2d<f32>;

@group(0) @binding(2)
var wall_sampler: sampler;

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle (3 vertices, zero vertex buffers)
@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    let p = positions[vid];
    var out: VertexOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    // uv in [0, 1], y=0 at top
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

// Interleaved Gradient Noise for anti-banding dithering on dark 8-bit displays
fn ign_dither(pixel: vec2<f32>) -> f32 {
    let n = fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
    return (n - 0.5) / 255.0;
}

fn hash21(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i + vec2<f32>(0.0, 0.0));
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var val = 0.0;
    var amp = 0.5;
    let rot = mat2x2<f32>(0.8, 0.6, -0.6, 0.8);
    for (var i = 0; i < 4; i = i + 1) {
        val += amp * value_noise(p);
        p = rot * p * 2.02;
        amp *= 0.5;
    }
    return val;
}

// Theme 0: Obsidian Drift — ultra-minimal, silky moving dark slate gradient
fn theme_obsidian_drift(uv: vec2<f32>, aspect: f32, t: f32) -> vec3<f32> {
    let p = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);

    let c1 = vec2<f32>(sin(t * 0.11) * 0.48 * aspect, cos(t * 0.08) * 0.34);
    let c2 = vec2<f32>(cos(t * 0.09 + 2.1) * 0.52 * aspect, sin(t * 0.12 + 1.3) * 0.38);
    let c3 = vec2<f32>(sin(t * 0.06 + 4.2) * 0.30 * aspect, cos(t * 0.07 + 3.0) * 0.25);

    let g1 = exp(-dot(p - c1, p - c1) * 1.45);
    let g2 = exp(-dot(p - c2, p - c2) * 1.25);
    let g3 = exp(-dot(p - c3, p - c3) * 2.10);

    // Base deep obsidian
    var col = vec3<f32>(0.024, 0.027, 0.036);
    // Subtle moving gradient washes
    col += vec3<f32>(0.038, 0.052, 0.078) * g1;
    col += vec3<f32>(0.030, 0.034, 0.058) * g2;
    col += vec3<f32>(0.018, 0.028, 0.040) * g3;

    // Gentle diagonal depth & edge vignette
    let diag = 1.0 - 0.22 * length(uv - vec2<f32>(0.5, 0.5));
    return col * diag;
}

// Theme 1: Midnight Aurora — dark obsidian sky with subtle slow indigo/teal wave
fn theme_midnight_aurora(uv: vec2<f32>, aspect: f32, t: f32) -> vec3<f32> {
    let p = vec2<f32>(uv.x * aspect, uv.y);

    let wave1 = sin(p.x * 1.15 + t * 0.14) * 0.14 + cos(p.x * 0.65 - t * 0.09) * 0.09;
    let band1 = exp(-pow((uv.y - 0.42 - wave1) * 2.8, 2.0));

    let wave2 = cos(p.x * 0.90 - t * 0.11 + 1.7) * 0.16;
    let band2 = exp(-pow((uv.y - 0.58 - wave2) * 2.4, 2.0));

    let drift = fbm(p * 1.3 + vec2<f32>(t * 0.03, -t * 0.02));

    var col = vec3<f32>(0.018, 0.024, 0.038);
    col += vec3<f32>(0.022, 0.058, 0.074) * band1 * (0.55 + 0.45 * drift);
    col += vec3<f32>(0.036, 0.032, 0.078) * band2 * (0.60 + 0.40 * (1.0 - drift));

    let vig = 1.0 - 0.28 * dot(uv - 0.5, uv - 0.5);
    return col * vig;
}

// Theme 2: Carbon Ember — dark charcoal with barely-there warm umber glow
fn theme_carbon_ember(uv: vec2<f32>, aspect: f32, t: f32) -> vec3<f32> {
    let p = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);

    let c1 = vec2<f32>(sin(t * 0.09 + 0.5) * 0.45 * aspect, cos(t * 0.07) * 0.30 + 0.08);
    let c2 = vec2<f32>(cos(t * 0.08 + 2.8) * 0.42 * aspect, sin(t * 0.10 + 1.0) * 0.28 - 0.05);

    let g1 = exp(-dot(p - c1, p - c1) * 1.55);
    let g2 = exp(-dot(p - c2, p - c2) * 1.35);

    var col = vec3<f32>(0.029, 0.026, 0.027);
    col += vec3<f32>(0.070, 0.036, 0.028) * g1;
    col += vec3<f32>(0.044, 0.026, 0.038) * g2;

    let vig = 1.0 - 0.25 * length(uv - 0.5);
    return col * vig;
}

// Theme 3: Void Mist — near-OLED black with whisper-soft slow monochrome fog
fn theme_void_mist(uv: vec2<f32>, aspect: f32, t: f32) -> vec3<f32> {
    let p = vec2<f32>(uv.x * aspect, uv.y) * 1.65;
    let q = vec2<f32>(
        fbm(p + vec2<f32>(t * 0.025, t * 0.015)),
        fbm(p + vec2<f32>(-t * 0.020, t * 0.022) + vec2<f32>(5.2, 1.3))
    );
    let f = fbm(p + q * 1.35 + vec2<f32>(t * 0.012, -t * 0.010));

    let base = vec3<f32>(0.010, 0.012, 0.016);
    let mist = vec3<f32>(0.046, 0.052, 0.066);
    var col = mix(base, mist, smoothstep(0.28, 0.85, f));

    let vig = 1.0 - 0.35 * dot(uv - 0.5, uv - 0.5);
    return col * vig;
}

// Custom image wallpaper with aspect-fill (cover) scaling
fn sample_custom_image(uv: vec2<f32>, screen_aspect: f32, img_aspect: f32) -> vec3<f32> {
    var tex_uv = uv;
    if (screen_aspect > img_aspect) {
        let scale = img_aspect / screen_aspect;
        tex_uv.y = (uv.y - 0.5) * scale + 0.5;
    } else {
        let scale = screen_aspect / img_aspect;
        tex_uv.x = (uv.x - 0.5) * scale + 0.5;
    }
    return textureSample(wall_tex, wall_sampler, tex_uv).rgb;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let aspect = max(u.resolution.x, 1.0) / max(u.resolution.y, 1.0);

    var col: vec3<f32>;
    if (u.mode == 1u) {
        col = sample_custom_image(in.uv, aspect, max(u.image_aspect, 0.01));
    } else if (u.theme_id == 1u) {
        col = theme_midnight_aurora(in.uv, aspect, u.time);
    } else if (u.theme_id == 2u) {
        col = theme_carbon_ember(in.uv, aspect, u.time);
    } else if (u.theme_id == 3u) {
        col = theme_void_mist(in.uv, aspect, u.time);
    } else {
        col = theme_obsidian_drift(in.uv, aspect, u.time);
    }

    // Dither built-in dark gradients to prevent 8-bit banding
    if (u.mode == 0u) {
        col = clamp(col + vec3<f32>(ign_dither(in.pos.xy)), vec3<f32>(0.0), vec3<f32>(1.0));
    }

    return vec4<f32>(col, 1.0);
}
