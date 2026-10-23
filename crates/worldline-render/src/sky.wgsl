// The sky behind everything: an equirectangular map of the sky (in the
// map's own axes, galactic for Worldline), seen along each pixel's ray.
//
// One lens may bend the rays first. For a ray at angle ψ from the
// direction to the lens, the table holds how much more than a straight
// line, δ, it sweeps around the lens before heading off: it ends up swept
// π − ψ + δ(ψ), measured around the lens from the camera's side. Rays
// within the lens's edge angle see black. The table is sampled evenly in
// x = ln((ψ − edge)/(π − edge)), from x_min to 0.

const PI: f32 = 3.14159265358979;

struct Sky {
    right: vec3<f32>,
    tan_half_fov: f32,
    up: vec3<f32>,
    aspect: f32,
    forward: vec3<f32>,
    lensing: f32,
    // World directions into the map's axes, row by row.
    map_x: vec3<f32>,
    edge: f32,
    map_y: vec3<f32>,
    x_min: f32,
    map_z: vec3<f32>,
    samples: f32,
    // Unit vector from the camera toward the lens.
    toward: vec3<f32>,
    // The angle one texel of the map spans, in rad.
    texel: f32,
    exposure: f32,
    // Three floats, not a vec3, which would start on a 16-byte boundary.
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
}

@group(0) @binding(0) var<uniform> sky: Sky;
@group(0) @binding(1) var map: texture_2d<f32>;
@group(0) @binding(2) var map_sampler: sampler;
@group(0) @binding(3) var<storage, read> bending: array<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> VertexOut {
    // One triangle that covers the whole screen.
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    let ndc = corner * 2.0 - 1.0;
    var out: VertexOut;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.ndc = ndc;
    return out;
}

@fragment
fn fs_sky(in: VertexOut) -> @location(0) vec4<f32> {
    let d = normalize(
        sky.forward
            + in.ndc.x * sky.tan_half_fov * sky.aspect * sky.right
            + in.ndc.y * sky.tan_half_fov * sky.up
    );

    // The ray bent by the lens: chosen with select, not branches, so the
    // derivatives below stay in uniform control flow.
    let toward = sky.toward;
    let psi = atan2(length(cross(d, toward)), dot(d, toward));
    let x = max(log(max(psi - sky.edge, 1e-30) / (PI - sky.edge)), sky.x_min);
    let f = (1.0 - x / sky.x_min) * (sky.samples - 1.0);
    let i = min(u32(max(f, 0.0)), u32(sky.samples) - 2u);
    let t = f - f32(i);
    let delta = mix(bending[i], bending[i + 1u], t);
    let swept = PI - psi + delta;
    let away = -toward;
    let side = d - dot(d, away) * away;
    let side_length = length(side);
    let lensed = away * cos(swept) + side / max(side_length, 1e-20) * sin(swept);
    let bent = sky.lensing > 0.5 && side_length > 1e-7;
    let dir = select(d, lensed, bent);
    let captured = sky.lensing > 0.5 && psi <= sky.edge;

    // Where on the map, and how much of it the pixel covers there, for
    // the level of detail: without the map's seam showing.
    let g = vec3<f32>(dot(sky.map_x, dir), dot(sky.map_y, dir), dot(sky.map_z, dir));
    let uv = vec2<f32>(
        0.5 - atan2(g.y, g.x) / (2.0 * PI),
        0.5 - asin(clamp(g.z, -1.0, 1.0)) / PI
    );
    let footprint = max(length(dpdx(dir)), length(dpdy(dir)));
    let lod = log2(max(footprint / sky.texel, 1e-6));
    let color = textureSampleLevel(map, map_sampler, uv, lod).rgb * sky.exposure;
    return select(vec4<f32>(color, 1.0), vec4<f32>(0.0, 0.0, 0.0, 1.0), captured);
}

// Or the rays are traced one by one past a spinning (Kerr) black hole, in
// the hole's Kerr–Schild Cartesian coordinates: units of GM/c², the hole
// at the origin spinning about +z, with the world's axes. The metric is
// g = η + f l⊗l, so with p_t = −E a ray's Hamiltonian is
// 2H = |p|² − E² − f (E + l·p)², and
//   dx/dλ = p − f (E + l·p) l,
//   dp/dλ = ½ (E + l·p)² ∇f + f (E + l·p) ∇(l·p),
// with r from (x² + y²)/(r² + a²) + z²/r² = 1, f = 2r³/(r⁴ + a²z²) and
// l = ((rx + ay)/(r² + a²), (ry − ax)/(r² + a²), z/r). Fourth-order
// Runge–Kutta, back in time, in single precision; worldline-core's
// kerr_light.rs does the same in double precision with error control.
//
// A thin disk in the equatorial plane may stop the rays. For each radius
// the table gives its temperature (as ln T), angular velocity Ω and u^t;
// light from it is shifted by g = 1/(u^t (E − ΩL)), and looks like the
// table's colors at ln T + ln g. Its outer part, from 60% of its outer
// radius out, fades to clear, so it has no hard edge.

struct Kerr {
    // (E, p) = launch (1, look) for the ray the camera sees along look.
    launch: mat4x4<f32>,
    // Where the camera is.
    camera: vec3<f32>,
    spin: f32,
    // A ray moving inward inside this radius can't come back out.
    capture: f32,
    // One moving outward past this radius has escaped.
    far: f32,
    disk_inner: f32,
    disk_outer: f32,
    // 1 to draw the disk.
    disk: f32,
    // Samples of the disk's radii, evenly in ln r from inner to outer.
    radii: f32,
    // The colors' first ln T and spacing, and how many.
    ln_t_min: f32,
    ln_t_step: f32,
    colors: f32,
    disk_exposure: f32,
    steps: f32,
    _padding: f32,
}

@group(0) @binding(4) var<uniform> kerr: Kerr;
// The disk's radii (ln T, Ω, u^t) and then its colors (r, g, b), as
// three floats each.
@group(0) @binding(5) var<storage, read> table: array<f32>;

struct Rates {
    dx: vec3<f32>,
    dp: vec3<f32>,
}

// The Boyer–Lindquist r of the point x.
fn kerr_radius(x: vec3<f32>, a: f32) -> f32 {
    let w = dot(x, x) - a * a;
    return sqrt(0.5 * (w + sqrt(w * w + 4.0 * a * a * x.z * x.z)));
}

// Hamilton's equations for a ray of energy e. Written with
// q = 1 + a²z²/r⁴ so nothing overflows far out:
// ∇r = (x, y, z(1 + a²/r²))/(rq), f = 2/(rq),
// ∇f = f (3∇r/r − ∇ln(r⁴q)).
fn rates(x: vec3<f32>, p: vec3<f32>, e: f32, a: f32) -> Rates {
    let a2 = a * a;
    let r = kerr_radius(x, a);
    let r2 = r * r;
    let q = 1.0 + a2 * x.z * x.z / (r2 * r2);
    let grad_r = vec3<f32>(x.x, x.y, x.z * (1.0 + a2 / r2)) / (r * q);
    let f = 2.0 / (r * q);
    let grad_ln = (4.0 * grad_r / r + vec3<f32>(0.0, 0.0, 2.0 * a2 * x.z / (r2 * r2))) / q;
    let grad_f = f * (3.0 * grad_r / r - grad_ln);
    let s = r2 + a2;
    let l = vec3<f32>((r * x.x + a * x.y) / s, (r * x.y - a * x.x) / s, x.z / r);
    // l·p = (r m + a n)/s + z p_z/r, m = x p_x + y p_y, n = y p_x − x p_y.
    let m = x.x * p.x + x.y * p.y;
    let n = x.y * p.x - x.x * p.y;
    let top = r * m + a * n;
    let grad_top = grad_r * m + r * vec3<f32>(p.x, p.y, 0.0) + a * vec3<f32>(-p.y, p.x, 0.0);
    let grad_lp = (grad_top - 2.0 * r * top / s * grad_r) / s
        + p.z * (vec3<f32>(0.0, 0.0, 1.0) - x.z / r * grad_r) / r;
    let k = e + dot(l, p);
    return Rates(p - f * k * l, 0.5 * k * k * grad_f + f * k * grad_lp);
}

// The disk at radius r, seen by a ray of energy e and angular momentum l.
fn disk_light(r: f32, e: f32, l: f32) -> vec3<f32> {
    let radii = u32(kerr.radii);
    let f = log(r / kerr.disk_inner) / log(kerr.disk_outer / kerr.disk_inner) * (kerr.radii - 1.0);
    let i = min(u32(max(f, 0.0)), radii - 2u);
    let t = clamp(f - f32(i), 0.0, 1.0);
    let ring = mix(
        vec3<f32>(table[3u * i], table[3u * i + 1u], table[3u * i + 2u]),
        vec3<f32>(table[3u * i + 3u], table[3u * i + 4u], table[3u * i + 5u]),
        t
    );
    let g = 1.0 / (ring.z * (e - ring.y * l));
    let c = clamp((ring.x + log(max(g, 1e-6)) - kerr.ln_t_min) / kerr.ln_t_step, 0.0, kerr.colors - 1.0);
    let j = min(u32(c), u32(kerr.colors) - 2u);
    let base = 3u * radii + 3u * j;
    let light = mix(
        vec3<f32>(table[base], table[base + 1u], table[base + 2u]),
        vec3<f32>(table[base + 3u], table[base + 4u], table[base + 5u]),
        c - f32(j)
    ) * kerr.disk_exposure;
    // Brighter than the display shows, gently.
    return 1.0 - exp(-light);
}

struct Traced {
    // Where on the sky the ray came from, in world directions.
    dir: vec3<f32>,
    // The disk's light it met on the way.
    color: vec3<f32>,
    // How much of the sky's light gets through: 0 if the ray was stopped.
    clear: f32,
}

// The direction error a step may make, rad.
const TOLERANCE: f32 = 1e-6;

// Traces back the ray the camera sees along world direction `look`.
fn trace_kerr(look: vec3<f32>) -> Traced {
    let start = kerr.launch * vec4<f32>(1.0, look);
    let e = start.x;
    let a = kerr.spin;
    var x = kerr.camera;
    var p = start.yzw;
    let angular = x.x * p.y - x.y * p.x;
    var r = kerr_radius(x, a);
    var color = vec3<f32>(0.0);
    var clear = 1.0;
    let steps = u32(kerr.steps);
    for (var i = 0u; i < steps; i++) {
        let k1 = rates(x, p, e, a);
        // Steps of κr. Light passing at r, moving at angle α to the line
        // from the hole, is bent at about 2 sin α/r² per unit length, and a
        // step's error is about that bending times κ⁴: κ is chosen to keep
        // it near TOLERANCE, from 3% of r near the hole to r far out.
        let speed = length(k1.dx);
        let across = length(cross(x, k1.dx)) / (length(x) * speed);
        let kappa = clamp(pow(TOLERANCE * r / (2.0 * across + 1e-3), 0.2), 0.03, 1.0);
        let h = -kappa * r / speed;
        let k2 = rates(x + 0.5 * h * k1.dx, p + 0.5 * h * k1.dp, e, a);
        let k3 = rates(x + 0.5 * h * k2.dx, p + 0.5 * h * k2.dp, e, a);
        let k4 = rates(x + h * k3.dx, p + h * k3.dp, e, a);
        let next_x = x + h / 6.0 * (k1.dx + 2.0 * k2.dx + 2.0 * k3.dx + k4.dx);
        let next_p = p + h / 6.0 * (k1.dp + 2.0 * k2.dp + 2.0 * k3.dp + k4.dp);
        if kerr.disk > 0.5 && x.z * next_x.z < 0.0 {
            let c = mix(x, next_x, x.z / (x.z - next_x.z));
            let rc = sqrt(max(dot(c.xy, c.xy) - a * a, 0.0));
            if rc >= kerr.disk_inner && rc <= kerr.disk_outer {
                let alpha = 1.0 - smoothstep(0.6 * kerr.disk_outer, kerr.disk_outer, rc);
                color += clear * alpha * disk_light(rc, e, angular);
                clear *= 1.0 - alpha;
                if clear < 1e-3 {
                    return Traced(look, color, 0.0);
                }
            }
        }
        let next_r = kerr_radius(next_x, a);
        if next_r < r && next_r < kerr.capture {
            return Traced(look, color, 0.0);
        }
        if next_r > r && next_r > kerr.far {
            return Traced(-normalize(rates(next_x, next_p, e, a).dx), color, clear);
        }
        x = next_x;
        p = next_p;
        r = next_r;
    }
    // Still circling: as good as captured.
    return Traced(look, color, 0.0);
}

@fragment
fn fs_kerr(in: VertexOut) -> @location(0) vec4<f32> {
    let d = normalize(
        sky.forward
            + in.ndc.x * sky.tan_half_fov * sky.aspect * sky.right
            + in.ndc.y * sky.tan_half_fov * sky.up
    );
    let traced = trace_kerr(d);
    let dir = traced.dir;
    let g = vec3<f32>(dot(sky.map_x, dir), dot(sky.map_y, dir), dot(sky.map_z, dir));
    let uv = vec2<f32>(
        0.5 - atan2(g.y, g.x) / (2.0 * PI),
        0.5 - asin(clamp(g.z, -1.0, 1.0)) / PI
    );
    let footprint = max(length(dpdx(dir)), length(dpdy(dir)));
    let lod = log2(max(footprint / sky.texel, 1e-6));
    let color = textureSampleLevel(map, map_sampler, uv, lod).rgb * sky.exposure;
    return vec4<f32>(traced.color + traced.clear * color, 1.0);
}

// For tests: traces the rays along each probe's direction, writing back
// where on the sky each came from and how much of its light was stopped.
@group(0) @binding(6) var<storage, read_write> probes: array<vec4<f32>>;

@compute @workgroup_size(64)
fn cs_trace(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= arrayLength(&probes) {
        return;
    }
    let traced = trace_kerr(probes[id.x].xyz);
    probes[id.x] = vec4<f32>(traced.dir, 1.0 - traced.clear);
}
