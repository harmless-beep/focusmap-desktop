// Appolis: sky, ground and instanced buildings in one offscreen pass.
// Colours are authored display-referred; the offscreen target matches the
// egui surface format so the composite is a straight copy.
//
// The city is a toy diorama seen from above: painted pastel blocks, big
// billboards, a dotted plaza and a ring road. At night the same geometry
// switches to lit windows and neon signage.

struct Globals {
    view_proj: mat4x4<f32>,
    // x: seconds, y: mood (0 deep, 1 steady, 2 scattered, 3 standby),
    // z: focus intensity 0..1, w: ambient
    params: vec4<f32>,
    // x: local day fraction 0..1, y: highlighted instance (-1 none),
    // z: highlight strength, w: synthwave toggle
    world: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

const TAU: f32 = 6.2831853;

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash11(n: f32) -> f32 {
    return fract(sin(n * 127.1) * 43758.5453);
}

fn neon_palette(i: f32) -> vec3<f32> {
    let slot = i - floor(i / 5.0) * 5.0;
    if (slot < 0.5) {
        return vec3<f32>(0.10, 0.88, 1.00);
    }
    if (slot < 1.5) {
        return vec3<f32>(1.00, 0.30, 0.62);
    }
    if (slot < 2.5) {
        return vec3<f32>(0.55, 0.92, 0.35);
    }
    if (slot < 3.5) {
        return vec3<f32>(1.00, 0.68, 0.22);
    }
    return vec3<f32>(0.62, 0.42, 1.00);
}

fn sun_height() -> f32 {
    return sin((g.world.x - 0.25) * TAU);
}

/// 0 at night, 1 in full daylight, through a short dawn/dusk ramp.
fn daylight() -> f32 {
    return smoothstep(-0.10, 0.30, sun_height());
}

/// How close the local clock is to dawn or dusk.
fn day_warmth() -> f32 {
    let h = g.world.x;
    let dawn = exp(-pow((h - 0.25) * 9.0, 2.0));
    let dusk = exp(-pow((h - 0.79) * 9.0, 2.0));
    return max(dawn, dusk);
}

fn haze() -> vec3<f32> {
    return mix(
        vec3<f32>(0.035, 0.030, 0.080),
        vec3<f32>(0.69, 0.79, 0.88),
        daylight(),
    );
}

fn mood_glow() -> f32 {
    // 1 for deep focus, down to 0.35 in standby.
    return mix(1.0, 0.35, smoothstep(1.5, 3.0, g.params.y)) * (0.55 + 0.45 * g.params.z);
}

/// Blueprint is a separate theme, not a colour filter: the whole city is
/// redrawn as a cyan schematic. The toggle rides in `world.w`.
fn blueprint() -> bool {
    return g.world.w > 0.5;
}

/// Screen-space rain: two layers of thin falling dashes. Only shown at night.
fn rain(uv: vec2<f32>, t: f32) -> f32 {
    var acc = 0.0;
    for (var i: i32 = 0; i < 2; i = i + 1) {
        let fi = f32(i);
        let cols = 130.0 + fi * 80.0;
        let x = uv.x * cols + uv.y * 5.0;
        let col = floor(x);
        let h = hash12(vec2<f32>(col, fi * 13.0));
        if (h > 0.55) {
            let speed = 1.1 + h * 1.6;
            let phase = fract(uv.y + t * speed * 0.12 + h);
            let seg = fract(phase * (7.0 + h * 6.0));
            let drop = smoothstep(0.0, 0.10, seg)
                * (1.0 - smoothstep(0.15, 0.45, seg));
            let line = 1.0 - smoothstep(0.0, 0.10, abs(fract(x) - 0.5));
            acc += drop * line * (0.30 + 0.50 * h) * (0.6 + 0.4 * fi);
        }
    }
    return acc;
}

/// A strike every so often: two quick flickers, then a soft tail. Same clock
/// for every pass, so sky and towers flash together.
fn lightning(t: f32) -> f32 {
    let slot = floor(t * 0.45);
    let h = hash12(vec2<f32>(slot, 11.7));
    let phase = fract(t * 0.45);
    let strike = step(0.88, h);
    let first = exp(-phase * 26.0);
    let second = exp(-max(phase - 0.16, 0.0) * 18.0) * step(0.16, phase);
    return strike * max(first, second * 0.7);
}

// ---------------------------------------------------------------- sky

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = corners[index];
    var out: SkyOut;
    out.clip = vec4<f32>(p, 0.0, 1.0);
    out.uv = p * 0.5 + 0.5;
    return out;
}

fn aurora(uv: vec2<f32>, t: f32) -> vec3<f32> {
    let cyan = vec3<f32>(0.05, 0.95, 1.0);
    let magenta = vec3<f32>(1.0, 0.12, 0.72);
    let violet = vec3<f32>(0.45, 0.2, 1.0);
    let calm = 1.0 - step(1.5, g.params.y);

    var glow = vec3<f32>(0.0);
    for (var i: i32 = 0; i < 3; i = i + 1) {
        let fi = f32(i);
        let speed = mix(0.05, 0.16, calm);
        let y = 0.62 + 0.09 * sin(uv.x * (4.5 + fi) + t * (speed + fi * 0.04))
            + 0.03 * sin(uv.x * (15.0 - fi * 2.0) - t * speed * 0.7);
        let d = uv.y - y;
        let band = exp(-d * d * (110.0 - fi * 20.0));
        let tint = mix(cyan, mix(magenta, violet, 0.5 + 0.5 * sin(uv.x * 3.0 + fi)), 0.30 + fi * 0.18);
        glow += band * tint * (0.22 - fi * 0.05);
    }

    let star_grid = uv * vec2<f32>(220.0, 124.0);
    let star_cell = floor(star_grid);
    let star_f = fract(star_grid) - 0.5;
    let dot = 1.0 - smoothstep(0.02, 0.22, length(star_f));
    let star = step(0.988, hash12(star_cell))
        * dot
        * (0.35 + 0.65 * sin(t * 1.1 + hash12(star_cell + 4.0) * TAU));
    glow += vec3<f32>(0.62, 0.78, 1.0) * star * smoothstep(0.30, 0.9, uv.y);
    return glow;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = g.params.x;

    if (blueprint()) {
        // Drafting paper: deep navy, a faint graph grid and a horizon glow.
        var c = mix(
            vec3<f32>(0.030, 0.070, 0.135),
            vec3<f32>(0.008, 0.020, 0.045),
            smoothstep(0.0, 1.0, uv.y),
        );
        c += vec3<f32>(0.06, 0.20, 0.30) * exp(-pow((uv.y - 0.34) * 5.0, 2.0)) * 0.5;
        let grid = uv * vec2<f32>(36.0, 24.0);
        let gl = abs(fract(grid) - 0.5);
        let line = 1.0 - smoothstep(0.455, 0.5, max(gl.x, gl.y));
        c += vec3<f32>(0.10, 0.34, 0.45) * line * 0.16;
        return vec4<f32>(c, 1.0);
    }

    let day = daylight();
    let night = 1.0 - day;
    let warm = day_warmth();

    let day_top = vec3<f32>(0.26, 0.55, 0.92);
    let day_low = vec3<f32>(0.70, 0.86, 0.99);
    let dusk_top = vec3<f32>(0.32, 0.30, 0.66);
    let dusk_low = vec3<f32>(1.00, 0.62, 0.44);
    let night_top = vec3<f32>(0.010, 0.007, 0.035);
    let night_low = vec3<f32>(0.055, 0.012, 0.13);

    let top = mix(night_top, mix(day_top, dusk_top, warm * 0.75), day);
    let low = mix(night_low, mix(day_low, dusk_low, warm * 0.80), day);
    var color = mix(low, top, smoothstep(0.08, 0.95, uv.y));

    // Sun: a soft disc and a wide warm bloom. Static in the sky because the
    // diorama camera moves, not the world.
    if (day > 0.01) {
        let sun_uv = vec2<f32>(0.66, 0.80);
        let sd = uv - sun_uv;
        color += vec3<f32>(1.00, 0.90, 0.65) * exp(-dot(sd, sd) * 260.0) * 0.9 * day;
        color += vec3<f32>(1.00, 0.72, 0.42) * exp(-length(sd) * 9.0) * 0.20 * day;
    }

    // Stars, aurora and weather belong to the night city.
    color += aurora(uv, t) * night;

    let flash = lightning(t) * night;
    color += vec3<f32>(0.10, 0.15, 0.22) * rain(uv, t) * night * 0.7;
    color += vec3<f32>(0.34, 0.40, 0.62) * flash
        * (0.18 + 0.82 * smoothstep(0.25, 0.95, uv.y));

    // Horizon haze picks up the city's light from below.
    let h = uv.y - 0.22;
    let band = exp(-h * h * 70.0);
    color += mix(
        vec3<f32>(0.06, 0.16, 0.30) * mood_glow(),
        vec3<f32>(0.95, 0.88, 0.72) * 0.16,
        day,
    ) * band;

    let scan = mix(0.96 + 0.04 * sin(uv.y * 950.0 + t * 1.7), 1.0, day);
    color *= scan;

    return vec4<f32>(color, 1.0);
}

// ---------------------------------------------------------------- ground

struct GroundOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec2<f32>,
};

@vertex
fn vs_ground(@location(0) pos: vec2<f32>) -> GroundOut {
    var out: GroundOut;
    out.clip = g.view_proj * vec4<f32>(pos.x, 0.0, pos.y, 1.0);
    out.world = pos;
    return out;
}

/// Soft band around a radius, in world units.
fn ring_mask(d: f32, r: f32, w: f32) -> f32 {
    return 1.0 - smoothstep(w, w * 2.0, abs(d - r));
}

/// Five coloured pools under the district blocks.
fn district_pools(world: vec2<f32>) -> vec3<f32> {
    let c0 = vec2<f32>(0.0, 26.0);
    let c1 = vec2<f32>(24.73, 8.03);
    let c2 = vec2<f32>(15.28, -21.03);
    let c3 = vec2<f32>(-15.28, -21.03);
    let c4 = vec2<f32>(-24.73, 8.03);
    let k = 0.0018;
    var acc = vec3<f32>(0.0);
    acc += vec3<f32>(1.00, 0.66, 0.18) * exp(-dot(world - c0, world - c0) * k);
    acc += vec3<f32>(0.62, 0.36, 1.00) * exp(-dot(world - c1, world - c1) * k);
    acc += vec3<f32>(1.00, 0.18, 0.72) * exp(-dot(world - c2, world - c2) * k);
    acc += vec3<f32>(0.55, 1.00, 0.35) * exp(-dot(world - c3, world - c3) * k);
    acc += vec3<f32>(0.10, 0.95, 1.00) * exp(-dot(world - c4, world - c4) * k);
    return acc;
}

@fragment
fn fs_ground(in: GroundOut) -> @location(0) vec4<f32> {
    let t = g.params.x;
    let day = daylight();
    let night = 1.0 - day;
    let d = length(in.world);
    let ang = atan2(in.world.y, in.world.x);

    // Paving stones: light warm concrete by day, blue-black by night.
    let cell = in.world / 2.0;
    let gline = abs(fract(cell - 0.5) - 0.5)
        / max(fwidth(cell), vec2<f32>(0.0001));
    let grid = 1.0 - min(min(gline.x, gline.y), 1.0);

    let day_base = vec3<f32>(0.70, 0.69, 0.72);
    let night_base = vec3<f32>(0.055, 0.060, 0.095);
    var color = mix(night_base, day_base, day);
    color -= vec3<f32>(0.045, 0.045, 0.045) * grid * (0.30 + 0.70 * day) * 0.6;

    // The central plaza: warmer floor, glowing concentric rings and a field
    // of small dots, like a festival square seen from above.
    let plaza = 1.0 - smoothstep(12.5, 13.1, d);
    color = mix(color, mix(vec3<f32>(0.08, 0.08, 0.12), vec3<f32>(0.80, 0.77, 0.80), day), plaza * 0.55);

    let ring_m = ring_mask(d, 4.8, 0.14) + ring_mask(d, 6.8, 0.14)
        + ring_mask(d, 8.8, 0.14) + ring_mask(d, 10.8, 0.14);
    let ring_col = vec3<f32>(1.00, 0.35, 0.62) * ring_mask(d, 4.8, 0.14)
        + vec3<f32>(1.00, 0.72, 0.24) * ring_mask(d, 6.8, 0.14)
        + vec3<f32>(0.20, 0.85, 0.90) * ring_mask(d, 8.8, 0.14)
        + vec3<f32>(0.58, 0.42, 0.98) * ring_mask(d, 10.8, 0.14);
    color = mix(color, ring_col, clamp(ring_m, 0.0, 1.0) * (0.48 + 0.44 * night));

    let polar = vec2<f32>((ang / TAU + 0.5) * 44.0, d * 0.9);
    let dot_cell = floor(polar);
    let dot_f = fract(polar) - 0.5;
    let dot_r = length(dot_f * vec2<f32>(1.0, 0.8));
    let dot = step(0.52, hash12(dot_cell)) * (1.0 - smoothstep(0.19, 0.32, dot_r));
    let dot_area = smoothstep(2.8, 3.6, d) * (1.0 - smoothstep(11.6, 12.4, d));
    let dot_col = neon_palette(floor(hash12(dot_cell + 7.0) * 5.0));
    color = mix(color, dot_col, dot * dot_area * (0.38 + 0.36 * night));

    if (blueprint()) {
        // Graph-paper ground with the plaza, rings and road drawn in ink.
        var c = vec3<f32>(0.020, 0.055, 0.110);
        c += vec3<f32>(0.10, 0.38, 0.50) * grid * 0.5;
        c = mix(c, vec3<f32>(0.030, 0.095, 0.170), plaza * 0.6);
        c += vec3<f32>(0.42, 0.90, 1.00) * clamp(ring_m, 0.0, 1.0) * 0.35;
        c += vec3<f32>(0.22, 0.60, 0.75) * dot * dot_area * 0.30;

        let road_edge = ring_mask(d, 13.15, 0.085) + ring_mask(d, 16.05, 0.085);
        c += vec3<f32>(0.30, 0.75, 0.90) * clamp(road_edge, 0.0, 1.0) * 0.9;
        c += vec3<f32>(0.18, 0.50, 0.64) * ring_mask(d, 14.65, 0.04) * 0.9;
        c += vec3<f32>(0.10, 0.30, 0.42) * ring_mask(d, 34.0, 0.12) * 0.8;

        let fog = 1.0 - exp(-max(d - 30.0, 0.0) * 0.025);
        c = mix(c, vec3<f32>(0.020, 0.050, 0.095), fog);
        return vec4<f32>(c, 1.0);
    }

    // Ring road between plaza and districts.
    let road = smoothstep(13.0, 13.3, d) * (1.0 - smoothstep(15.9, 16.2, d));
    let asphalt = mix(vec3<f32>(0.055, 0.058, 0.080), vec3<f32>(0.28, 0.29, 0.33), day);
    color = mix(color, asphalt, road);
    let lane = ring_mask(d, 14.65, 0.05) * step(0.5, fract((ang / TAU + 0.5) * 56.0));
    color = mix(color, mix(vec3<f32>(0.55, 0.55, 0.58), vec3<f32>(0.96, 0.94, 0.86), day), lane * road);

    // District colour bleeding onto the surrounding blocks.
    color += district_pools(in.world) * (0.10 + 0.10 * day);

    // Pool of light under the core.
    color += vec3<f32>(0.05, 0.18, 0.26) * exp(-d * 0.16) * 0.55 * night * mood_glow();
    color += vec3<f32>(0.035, 0.045, 0.07) * lightning(t) * night;

    let fog = 1.0 - exp(-max(d - 18.0, 0.0) * 0.035);
    color = mix(color, haze(), fog * 0.92);
    return vec4<f32>(color, 1.0);
}

// ---------------------------------------------------------------- buildings

struct CubeIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct InstIn {
    @location(2) center: vec3<f32>,
    @location(3) half: vec3<f32>,
    @location(4) body: vec3<f32>,
    @location(5) glow: vec3<f32>,
    @location(6) seed: f32,
};

struct BuildOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) unit: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) body: vec3<f32>,
    @location(3) glow: vec3<f32>,
    @location(4) seed: f32,
    @location(5) dist: f32,
    @location(6) height: f32,
    @location(7) index: f32,
};

@vertex
fn vs_building(
    cube: CubeIn,
    inst: InstIn,
    @builtin(instance_index) instance: u32,
) -> BuildOut {
    let world = cube.pos * inst.half * 2.0 + inst.center;
    var out: BuildOut;
    out.clip = g.view_proj * vec4<f32>(world, 1.0);
    out.unit = cube.pos;
    out.normal = cube.normal;
    out.body = inst.body;
    out.glow = inst.glow;
    out.seed = inst.seed;
    out.dist = length(world.xz);
    out.height = inst.half.y * 2.0;
    out.index = f32(instance);
    return out;
}

@fragment
fn fs_building(in: BuildOut) -> @location(0) vec4<f32> {
    let t = g.params.x;
    let day = daylight();
    let night = 1.0 - day;
    let n = normalize(in.normal);
    let glow_gain = mood_glow();

    if (blueprint()) {
        // Towers become ink drawings: dark volumes, bright outlines, a bay and
        // floor grid, and a dimensioned roof.
        let u = select(in.unit.x, in.unit.z, abs(n.x) > 0.5) + 0.5;
        let v = in.unit.y + 0.5;
        var c = vec3<f32>(0.030, 0.075, 0.135)
            + vec3<f32>(0.018, 0.040, 0.070) * smoothstep(-0.5, 0.5, n.y);
        if (n.y > 0.5) {
            let ex = abs(in.unit.x);
            let ez = abs(in.unit.z);
            let border = 1.0 - smoothstep(0.0, 0.025, min(0.5 - ex, 0.5 - ez));
            let inner = 1.0 - smoothstep(0.0, 0.018, abs(max(ex, ez) - 0.33));
            let diag = 1.0 - smoothstep(
                0.0,
                0.012,
                abs(fract((in.unit.x + in.unit.z) * 6.0) - 0.5),
            );
            c += vec3<f32>(0.30, 0.78, 1.00) * border * 0.85;
            c += vec3<f32>(0.16, 0.48, 0.66) * inner * 0.55;
            c += vec3<f32>(0.10, 0.30, 0.44) * diag * 0.30;
        } else {
            let rows = clamp(floor(in.height * 0.8), 3.0, 10.0);
            let floors = 1.0 - smoothstep(0.455, 0.5, abs(fract(v * rows) - 0.5));
            let bays = 1.0 - smoothstep(0.465, 0.5, abs(fract(u * 4.0) - 0.5));
            c += vec3<f32>(0.14, 0.42, 0.58) * (floors * 0.7 + bays * 0.35);
            let edge = 1.0 - smoothstep(0.0, 0.018, min(u, 1.0 - u));
            c += vec3<f32>(0.35, 0.82, 1.00) * edge * 0.9;
            let door = (1.0 - smoothstep(0.02, 0.035, abs(u - 0.5)))
                * (1.0 - smoothstep(0.10, 0.13, v));
            c += vec3<f32>(0.30, 0.70, 0.88) * door * 0.7;
        }
        let fog = 1.0 - exp(-(in.dist * 0.010) * (in.dist * 0.010));
        c = mix(c, vec3<f32>(0.020, 0.050, 0.095), fog);
        let hot = (1.0 - step(0.5, abs(in.index - g.world.y))) * g.world.z;
        c = mix(c, c * 1.6 + vec3<f32>(0.20, 0.65, 0.85), hot);
        return vec4<f32>(c, 1.0);
    }

    // Sunlit paint. The key sits over the camera's shoulder, so both visible
    // walls carry some light.
    let key = normalize(vec3<f32>(0.50, 0.82, 0.34));
    let lambert = max(dot(n, key), 0.0);
    var color = in.body * (0.68 + 0.42 * lambert);
    color += vec3<f32>(0.03, 0.04, 0.055) * day * max(n.y, 0.0);

    if (n.y > 0.5) {
        // Roof deck in a lighter paint, with a cream parapet.
        color = in.body * 1.10 + vec3<f32>(0.04, 0.04, 0.04);
        let edge = max(abs(in.unit.x), abs(in.unit.z));
        let lip = smoothstep(0.40, 0.47, edge);
        color = mix(color, vec3<f32>(0.94, 0.92, 0.85), lip * 0.45);

        // Rooftop billboard on the larger blocks. From the high camera these
        // are the signs that give the diorama its carnival read.
        let has = step(0.30, hash12(vec2<f32>(in.seed * 97.0, in.index * 1.7)));
        let panel = (1.0 - smoothstep(0.34, 0.38, abs(in.unit.x)))
            * (1.0 - smoothstep(0.075, 0.10, abs(in.unit.z)));
        let frame = (1.0 - smoothstep(0.36, 0.41, abs(in.unit.x)))
            * (1.0 - smoothstep(0.085, 0.11, abs(in.unit.z)));
        // Some roofs turn the sign a quarter turn so both street axes get one.
        let has2 = step(0.55, hash12(vec2<f32>(in.index * 2.3, in.seed * 17.0)));
        let panel2 = (1.0 - smoothstep(0.34, 0.38, abs(in.unit.z)))
            * (1.0 - smoothstep(0.075, 0.10, abs(in.unit.x)));
        let frame2 = (1.0 - smoothstep(0.36, 0.41, abs(in.unit.z)))
            * (1.0 - smoothstep(0.085, 0.11, abs(in.unit.x)));
        let board = max(panel * has, panel2 * has2);
        let border = max((frame - panel) * has, (frame2 - panel2) * has2);
        let pick = hash12(vec2<f32>(in.seed * 13.0, in.index));
        let bg = mix(vec3<f32>(0.99, 0.97, 0.90), in.glow, step(0.45, pick));
        let avg = (bg.r + bg.g + bg.b) / 3.0;
        let ink = mix(vec3<f32>(1.0), vec3<f32>(0.10, 0.11, 0.15), step(0.62, avg));
        let axis = select(in.unit.x, in.unit.z, has2 > has);
        let gx = floor((axis + 0.34) / 0.68 * 8.0);
        let glyph = step(0.42, hash12(vec2<f32>(gx, in.seed * 31.0)));
        color = mix(color, vec3<f32>(0.98, 0.96, 0.90), border * 0.85);
        color = mix(color, bg, board * 0.95);
        color = mix(color, ink, board * glyph * 0.85);
        color += bg * board * night * 0.55 * (0.45 + 0.55 * glow_gain);
    } else {
        var u = in.unit.x + 0.5;
        let side = in.normal.x * 3.7 + in.normal.z * 5.3;
        if (abs(n.x) > 0.5) {
            u = in.unit.z + 0.5;
        }
        let v = in.unit.y + 0.5;
        let hs = hash12(vec2<f32>(in.seed * 41.0 + side, 7.0));

        // Street level: striped awnings over a dark door.
        let shop = 1.0 - smoothstep(0.10, 0.14, v);
        let awning = step(0.5, fract(u * 9.0 + hs));
        let shop_col = mix(vec3<f32>(0.96, 0.93, 0.84), in.glow, awning * 0.75);
        color = mix(color, shop_col, shop * 0.85);
        let door = (1.0 - smoothstep(0.025, 0.045, abs(u - 0.28)))
            * (1.0 - smoothstep(0.08, 0.11, v));
        color = mix(color, vec3<f32>(0.16, 0.17, 0.22), door * 0.8);

        // Window grid. One-pixel soft edges keep the panes from shimmering as
        // the camera drifts.
        let cols = 3.0 + floor(hs * 3.0);
        let rows = clamp(floor(in.height * 0.7), 2.0, 8.0);
        let grid = vec2<f32>(u * cols, (v - 0.12) * rows);
        let cell = floor(grid);
        let f = fract(grid);
        let w = max(fwidth(grid), vec2<f32>(0.0001));
        let pane = smoothstep(0.20 - w.x, 0.20 + w.x, f.x)
            * (1.0 - smoothstep(0.80 - w.x, 0.80 + w.x, f.x))
            * smoothstep(0.22 - w.y, 0.22 + w.y, f.y)
            * (1.0 - smoothstep(0.78 - w.y, 0.78 + w.y, f.y));
        let zone = smoothstep(0.13, 0.16, v)
            * (1.0 - smoothstep(0.94, 0.97, v))
            * step(2.8, in.height);
        let win = pane * zone;
        let roll = hash12(cell + vec2<f32>(in.seed * 47.0, side));
        let lit = step(0.55, roll);
        let glass = mix(in.body * 0.45, vec3<f32>(0.22, 0.34, 0.50), day);
        color = mix(color, glass, win * mix(0.60, 0.42, day));
        color += in.glow * win * lit * night * (0.45 + 0.55 * glow_gain);

        // One big painted sign per facade, with blocky lettering.
        let band = smoothstep(0.58, 0.61, v) * (1.0 - smoothstep(0.86, 0.89, v));
        let su = smoothstep(0.07, 0.11, u) * (1.0 - smoothstep(0.89, 0.93, u));
        let tall = step(3.6, in.height);
        let sign = band * su * tall;
        let outer_band = smoothstep(0.555, 0.585, v) * (1.0 - smoothstep(0.885, 0.915, v));
        let outer_u = smoothstep(0.05, 0.08, u) * (1.0 - smoothstep(0.92, 0.95, u));
        let border = (outer_band * outer_u - band * su) * tall;
        let panel_col = mix(
            vec3<f32>(0.99, 0.96, 0.88),
            in.glow,
            step(0.40, fract(hs * 5.0)),
        );
        let avg = (panel_col.r + panel_col.g + panel_col.b) / 3.0;
        let ink = mix(vec3<f32>(1.0), vec3<f32>(0.10, 0.11, 0.15), step(0.62, avg));
        let sgrid = vec2<f32>((u - 0.11) / 0.78 * 7.0, (v - 0.61) / 0.25 * 2.0);
        let s_cell = floor(sgrid);
        let s_f = fract(sgrid);
        let glyph = step(0.40, hash12(s_cell + vec2<f32>(in.seed * 23.0, side)))
            * (1.0 - smoothstep(0.28, 0.38, abs(s_f.x - 0.5)))
            * (1.0 - smoothstep(0.26, 0.36, abs(s_f.y - 0.5)));
        color = mix(color, vec3<f32>(0.98, 0.96, 0.90), border * 0.85);
        color = mix(color, panel_col, sign * 0.95);
        color = mix(color, ink, sign * glyph * 0.90);
        color += panel_col * sign * night * 0.45 * (0.45 + 0.55 * glow_gain);

        // Cream parapet and corner pilasters.
        let parapet = smoothstep(0.955, 0.975, v) * (1.0 - smoothstep(0.985, 1.0, v));
        color = mix(color, vec3<f32>(0.94, 0.92, 0.85), parapet * 0.8);
        let pilaster = 1.0 - smoothstep(0.012, 0.026, min(u, 1.0 - u));
        color = mix(color, vec3<f32>(0.94, 0.92, 0.85), pilaster * 0.30);
    }

    // Night keeps a neon rim on the silhouette, as before.
    if (night > 0.01) {
        var edge = 0.0;
        if (n.y < 0.5) {
            let uu = select(in.unit.x + 0.5, in.unit.z + 0.5, abs(n.x) > 0.5);
            edge = (1.0 - smoothstep(0.0, 0.05, min(uu, 1.0 - uu))) * 0.35;
        }
        color += in.glow * edge * night * (0.35 + 0.65 * glow_gain);
    }

    let flash = lightning(t) * night;
    color += vec3<f32>(0.05, 0.065, 0.10) * flash * (0.35 + lambert * 0.65);

    let fog = 1.0 - exp(-(in.dist * 0.013) * (in.dist * 0.013));
    color = mix(color, haze(), fog * 0.82);

    // The tower under the pointer (or pinned by a click) steps forward.
    let hot = (1.0 - step(0.5, abs(in.index - g.world.y))) * g.world.z;
    color = mix(color, color * 1.35 + in.glow * 0.22, hot);

    return vec4<f32>(color, 1.0);
}

// ---------------------------------------------------------------- traffic
//
// One draw covers every moving thing in the city: cars, drones, monorail,
// airships, the plaza crowd, billboards, trees, lamps and wisps.

const CARS: f32 = 20.0;
const DRONES: f32 = 28.0;
const TRACK: f32 = 46.0;
const PILLARS: f32 = 52.0;
const TRAIN: f32 = 58.0;
const BLIMPS: f32 = 67.0;
const PEDS: f32 = 187.0;
const SIDEWALK: f32 = 227.0;
const BOARDS: f32 = 243.0;
const TREES: f32 = 275.0;
const LAMPS: f32 = 299.0;

struct TrafficOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) phase: f32,
    @location(2) dist: f32,
    /// 0 object, 1 airship, 2 pedestrian, 3 billboard, 4 wisp.
    @location(3) kind: f32,
    /// Cube-local position, for heads, keels and glyphs.
    @location(4) unit: vec3<f32>,
};

@vertex
fn vs_traffic(cube: CubeIn, @builtin(instance_index) instance: u32) -> TrafficOut {
    let i = f32(instance);
    let t = g.params.x;
    let mood = g.params.y;
    let seed = hash11(i + 3.7);
    let day = daylight();
    var color = neon_palette(i);
    var kind = 0.0;

    var center: vec3<f32>;
    var half: vec3<f32>;
    var tangent: vec3<f32>;
    let up = vec3<f32>(0.0, 1.0, 0.0);

    if (i < CARS) {
        // Little cars on the ring road.
        let lane = step(0.5, fract(i * 0.37));
        let dir = mix(1.0, -1.0, lane);
        let speed = mix(0.10, 0.22, seed) * dir;
        let angle = t * speed + seed * TAU + i * 0.31;
        let radius = 13.6 + 1.5 * hash11(i * 2.3);
        // Scattered minutes make traffic weave; deep focus keeps it smooth.
        let scatter = step(1.5, mood) * (1.0 - step(2.5, mood));
        let weave = scatter * sin(t * 5.0 + i) * 0.25;
        center = vec3<f32>(
            cos(angle) * (radius + weave),
            0.24,
            sin(angle) * (radius + weave),
        );
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle)) * dir;
        half = vec3<f32>(0.16, 0.11, 0.32);
    } else if (i < DRONES) {
        // Drones hang over the skyline and blink.
        let j = i - CARS;
        let angle = t * 0.05 + hash11(j * 7.1) * TAU;
        let radius = 8.0 + 12.0 * hash11(j * 3.3);
        let height = 8.0 + 6.0 * hash11(j * 5.9) + sin(t * 0.8 + j) * 0.8;
        center = vec3<f32>(cos(angle) * radius, height, sin(angle) * radius);
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle));
        half = vec3<f32>(0.09, 0.09, 0.09);
    } else if (i < TRACK) {
        // Elevated monorail ring: eighteen straight box segments.
        let j = i - DRONES;
        let angle = j / (TRACK - DRONES) * TAU;
        let radius = 16.4;
        center = vec3<f32>(cos(angle) * radius, 5.7, sin(angle) * radius);
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle));
        half = vec3<f32>(0.10, 0.08, 2.72);
        color = mix(vec3<f32>(0.12, 0.14, 0.18), vec3<f32>(0.82, 0.84, 0.88), day);
    } else if (i < PILLARS) {
        // Every third joint drops a support to the ground.
        let j = i - TRACK;
        let angle = j / (PILLARS - TRACK) * TAU + 0.17;
        let radius = 16.4;
        center = vec3<f32>(cos(angle) * radius, 2.85, sin(angle) * radius);
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle));
        half = vec3<f32>(0.10, 2.85, 0.10);
        color = mix(vec3<f32>(0.16, 0.18, 0.22), vec3<f32>(0.76, 0.77, 0.80), day);
    } else if (i < TRAIN) {
        // Six cars riding the ring.
        let j = i - PILLARS;
        let angle = t * 0.10 + 1.2 + j * 0.06;
        let radius = 16.0;
        center = vec3<f32>(cos(angle) * radius, 5.95, sin(angle) * radius);
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle));
        half = vec3<f32>(0.20, 0.26, 0.62);
        color = neon_palette(j + 1.0);
    } else if (i < BLIMPS) {
        // Airships: a hull plus two tail fins, lit from below.
        let j = i - TRAIN;
        let ship = floor(j / 3.0);
        let part = j - ship * 3.0;
        let angle = t * 0.018 + ship * 2.1;
        let radius = 22.0 + ship * 3.0;
        let origin = vec3<f32>(
            cos(angle) * radius,
            13.5 + ship * 1.4 + sin(t * 0.3 + ship) * 0.3,
            sin(angle) * radius,
        );
        let dir = vec3<f32>(-sin(angle), 0.0, cos(angle));
        tangent = dir;
        color = mix(neon_palette(ship + 2.0), vec3<f32>(0.95, 0.94, 0.90), 0.35);
        kind = 1.0;
        if (part < 0.5) {
            center = origin;
            half = vec3<f32>(0.42, 0.34, 1.55);
        } else {
            center = origin - dir * 1.25
                + vec3<f32>(0.0, (part - 1.5) * 0.9, 0.0);
            half = vec3<f32>(0.07, 0.55, 0.42);
        }
    } else if (i < PEDS) {
        // The plaza crowd: chunky little walkers with a slow bob.
        let j = i - BLIMPS;
        let radius = 3.6 + 7.2 * hash11(j * 5.7);
        let dir = mix(1.0, -1.0, step(0.5, hash11(j * 7.7)));
        let angle = t * mix(0.03, 0.07, hash11(j * 2.1)) * dir + hash11(j * 3.1) * TAU;
        let bob = abs(sin(t * mix(4.0, 7.0, seed) + j * 1.7)) * 0.035;
        center = vec3<f32>(cos(angle) * radius, 0.17 + bob, sin(angle) * radius);
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle)) * dir;
        half = vec3<f32>(0.10, 0.17, 0.10);
        color = mix(neon_palette(j + 0.5), vec3<f32>(1.0, 0.98, 0.94), 0.38);
        kind = 2.0;
    } else if (i < SIDEWALK) {
        // Walkers trickling out through the five gaps between districts.
        let j = i - PEDS;
        let gap = floor(j / 8.0);
        let lane = hash11(j * 1.7) - 0.5;
        let angle = (36.0 + gap * 72.0 + lane * 7.0) * (TAU / 360.0);
        let go = mix(1.0, -1.0, step(0.5, hash11(j * 4.3)));
        let travel = fract(hash11(j * 9.1) + t * 0.006 * go);
        let radius = 16.8 + travel * 15.0;
        let bob = abs(sin(t * 5.5 + j)) * 0.035;
        center = vec3<f32>(cos(angle) * radius, 0.17 + bob, sin(angle) * radius);
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle)) * go;
        half = vec3<f32>(0.10, 0.17, 0.10);
        color = mix(neon_palette(j + 1.5), vec3<f32>(1.0, 0.98, 0.94), 0.42);
        kind = 2.0;
    } else if (i < BOARDS) {
        // Eight big billboards around the plaza, each on a slim pole.
        let j = i - SIDEWALK;
        let board = floor(j * 0.5);
        let part = j - board * 2.0;
        let angle = board * (TAU / 8.0) + 0.38;
        let radius = 11.2;
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle));
        if (part < 0.5) {
            let h = 4.8 + hash11(board * 4.1) * 2.0;
            center = vec3<f32>(cos(angle) * radius, h, sin(angle) * radius);
            half = vec3<f32>(0.10, 1.25, 1.95);
            color = neon_palette(board + 1.0);
            kind = 3.0;
        } else {
            center = vec3<f32>(cos(angle) * radius, 2.4, sin(angle) * radius);
            half = vec3<f32>(0.09, 2.4, 0.09);
            color = mix(vec3<f32>(0.18, 0.19, 0.24), vec3<f32>(0.85, 0.84, 0.80), day);
        }
    } else if (i < TREES) {
        // Trees: a fat green canopy over a short brown trunk.
        let j = i - BOARDS;
        let tree = floor(j * 0.5);
        let part = j - tree * 2.0;
        let angle = tree * (TAU / 16.0) + hash11(tree * 3.3) * 0.18;
        let radius = 11.8 + hash11(tree * 8.1) * 0.9;
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        if (part < 0.5) {
            center = vec3<f32>(x, 1.55, z);
            half = vec3<f32>(0.42, 0.50, 0.42);
            color = mix(
                vec3<f32>(0.28, 0.52, 0.26),
                vec3<f32>(0.58, 0.76, 0.32),
                hash11(tree * 2.7),
            );
        } else {
            center = vec3<f32>(x, 0.55, z);
            half = vec3<f32>(0.10, 0.55, 0.10);
            color = vec3<f32>(0.38, 0.27, 0.19);
        }
    } else if (i < LAMPS) {
        // Street lamps on the plaza rim: pole plus a warm head.
        let j = i - TREES;
        let lamp = floor(j * 0.5);
        let part = j - lamp * 2.0;
        let angle = lamp * (TAU / 12.0) + 0.10;
        let radius = 12.5;
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        if (part < 0.5) {
            center = vec3<f32>(x, 1.25, z);
            half = vec3<f32>(0.06, 1.25, 0.06);
            color = mix(vec3<f32>(0.20, 0.21, 0.26), vec3<f32>(0.88, 0.86, 0.80), day);
        } else {
            center = vec3<f32>(x, 2.52, z);
            half = vec3<f32>(0.14, 0.10, 0.14);
            color = vec3<f32>(1.0, 0.86, 0.52) * mix(1.0, 0.6, day);
        }
    } else {
        // Glowing wisps drift between the towers and pulse slowly.
        let j = i - LAMPS;
        let angle = t * 0.07 + j * 1.05;
        let radius = 5.0 + 2.5 * sin(t * 0.4 + j * 2.0)
            + 2.0 * hash11(j * 4.4);
        let height = 2.0 + 3.5 * hash11(j * 6.2) + sin(t * 0.8 + j) * 0.6;
        center = vec3<f32>(cos(angle) * radius, height, sin(angle) * radius);
        tangent = vec3<f32>(-sin(angle), 0.0, cos(angle));
        half = vec3<f32>(0.13, 0.17, 0.13);
        color = mix(neon_palette(j + 3.0), vec3<f32>(0.80, 0.95, 1.0), 0.55);
        kind = 4.0;
    }

    let side = normalize(cross(up, tangent));
    let world = center
        + side * cube.pos.x * half.x * 2.0
        + up * cube.pos.y * half.y * 2.0
        + tangent * cube.pos.z * half.z * 2.0;

    var out: TrafficOut;
    out.clip = g.view_proj * vec4<f32>(world, 1.0);
    out.color = color;
    out.phase = seed;
    out.dist = length(world.xz);
    out.kind = kind;
    out.unit = cube.pos;
    return out;
}

@fragment
fn fs_traffic(in: TrafficOut) -> @location(0) vec4<f32> {
    let t = g.params.x;
    let day = daylight();
    let night = 1.0 - day;
    var color = in.color;

    if (blueprint()) {
        var c = vec3<f32>(0.08, 0.30, 0.44);
        c = mix(c, vec3<f32>(0.35, 0.80, 0.95), smoothstep(-0.4, 0.4, in.unit.y) * 0.55);
        if (in.kind > 1.5) {
            c += vec3<f32>(0.20, 0.55, 0.70) * 0.4;
        }
        let fog = 1.0 - exp(-(in.dist * 0.010) * (in.dist * 0.010));
        c = mix(c, vec3<f32>(0.020, 0.050, 0.095), fog);
        return vec4<f32>(c, 1.0);
    }

    if (in.kind < 0.5) {
        // Cars, monorail, trees and lamps: a little top light so the boxes
        // read as shapes rather than flat cut-outs.
        color *= 0.84 + 0.24 * smoothstep(-0.4, 0.4, in.unit.y);
        let blink = 0.82 + 0.18 * sin(t * 6.0 + in.phase * 20.0);
        color *= mix(1.0, blink, night * 0.5);
    } else if (in.kind < 1.5) {
        // Airships are dark shapes with a lit keel, not neon slabs.
        let blink = 0.75 + 0.25 * sin(t * 6.0 + in.phase * 20.0);
        let keel = 1.0 - smoothstep(-0.25, 0.30, in.unit.y);
        color = in.color * (0.35 + 0.15 * day)
            + in.color * keel * 0.30 * (0.50 + 0.50 * blink);
    } else if (in.kind < 2.5) {
        // Pedestrians: cream head, coloured shirt, dark legs.
        if (in.unit.y > 0.22) {
            color = vec3<f32>(0.98, 0.87, 0.73);
        } else if (in.unit.y < -0.20) {
            color *= 0.30;
        }
        color *= 0.88 + 0.16 * smoothstep(-0.4, 0.4, in.unit.y);
    } else if (in.kind < 3.5) {
        // Billboards run a bright block pattern, like distant ad glyphs.
        let gx = floor((in.unit.z + 0.5) * 9.0);
        let gy = floor((in.unit.y + 0.5) * 5.0);
        let glyph = 0.30
            + 0.70 * step(0.42, hash12(vec2<f32>(gx, gy + floor(t * 0.7) * 5.0)));
        let edge = 1.0 - smoothstep(0.36, 0.45, abs(in.unit.y));
        color = in.color * glyph * 1.15 + in.color * edge * 0.5;
    } else {
        // Wisps are soft, slow pulses rather than blinking lights.
        let pulse = 0.75 + 0.35 * sin(t * 2.2 + in.phase * 12.0);
        color = mix(in.color, vec3<f32>(0.85, 0.95, 1.0), 0.55) * pulse;
    }

    let fog = 1.0 - exp(-(in.dist * 0.013) * (in.dist * 0.013));
    color = mix(color, haze(), fog * 0.8);
    return vec4<f32>(color, 1.0);
}
