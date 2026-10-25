// A self-gravitating fluid as SPH particles, on the GPU in single
// precision: worldline-core's sph.rs, the same equations, in the fluid's
// own units (G = 1). Each step is a chain of compute passes:
//   kick_drift   half a kick, then a drift;
//   clear, count, scan_blocks, scan_sums, scan_add, scatter, gather
//                particles sorted by a hashed grid of cells (Teschner et
//                al. 2003's hash), two of the smallest smoothing lengths
//                across, or a quarter of two of the largest, whichever is
//                more; what neighbours need of each copied out in that
//                order, so neighbours sit together in memory, and the
//                passes below work through the particles in that order;
//   density      each smoothing length and density together, by Newton's
//                method on Σ m W(h) = m (η/h)³, and Ω (Price 2012 eqs. 10,
//                27), searching the cells within its own 2h; and each
//                bucket's largest h;
//   predict      velocities and pressures at the step's end;
//   gravity      every particle's pull as from a point, from every other,
//                in tiles, two particles to an invocation;
//   forces       pressure (eq. 30), viscosity with Morris & Monaghan's
//                switch (eqs. 98, 105), heating (eq. 35), with every
//                particle within 2 max(h_a, h_b): the cells within twice
//                the largest h, skipping those whose largest h can't reach;
//                and gravity: the pull as from points, softened between
//                neighbours, close enough for each other's kernels;
//   kick         the other half kick (and, while settling, damping and
//                the entropy set);
//   finish       the clock, the next step and the next grid.

const PI: f32 = 3.14159265358979;
const ETA: f32 = 1.2;
const ALPHA_MIN: f32 = 0.1;
const ALPHA_MAX: f32 = 1.0;
const SIGMA_DECAY: f32 = 0.1;
const EPSILON: f32 = 0.01;
const COURANT: f32 = 0.3;
const FORCE: f32 = 0.25;
// Counts summed per workgroup in the scan, and the workgroup's size.
const BLOCK: u32 = 1024u;
const GROUP: u32 = 256u;
// Particles shared by a workgroup in the gravity pass: two for each of its
// 64 invocations.
const TILE: u32 = 128u;

struct Params {
    count: u32,
    table: u32,
    gamma: f32,
    // 1 while settling: motion damped at rate `damping`, entropy held at
    // `entropy` (P = Kρ^γ).
    settling: f32,
    entropy: f32,
    damping: f32,
    _padding0: f32,
    _padding1: f32,
}

struct State {
    time: f32,
    dt: f32,
    cell: f32,
    _padding: f32,
    // The next step and the largest and smallest smoothing lengths, as
    // bits: for positive floats their order is their bits'.
    dt_next: atomic<u32>,
    h_max: atomic<u32>,
    h_min: atomic<u32>,
    _padding1: u32,
}

struct Particle {
    // Position, and mass.
    position: vec4<f32>,
    // Velocity, and internal energy per unit mass.
    velocity: vec4<f32>,
    // Smoothing length, density, Ω, viscosity parameter α.
    fields: vec4<f32>,
    // Acceleration, and heating du/dt.
    force: vec4<f32>,
    // Velocity predicted at the step's end, and pressure there.
    predicted: vec4<f32>,
    // Sound speed, dα/dt, the largest |μ| of the viscosity, hash key bits.
    extra: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> state: State;
@group(0) @binding(2) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(3) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> starts: array<u32>;
@group(0) @binding(5) var<storage, read_write> sorted: array<u32>;
@group(0) @binding(6) var<storage, read_write> blocks: array<u32>;
// The largest smoothing length in each bucket, as bits.
@group(0) @binding(7) var<storage, read_write> bucket_h: array<atomic<u32>>;

// What neighbours need of a particle, in the grid's order, as four
// arrays one after another, so each is read in a stretch:
//   position and mass;
//   smoothing length, density, Ω, viscosity parameter α;
//   velocity predicted at the step's end, and pressure there;
//   sound speed, and the pull of every particle as from a point.
@group(0) @binding(8) var<storage, read_write> near: array<vec4<f32>>;

struct Neighbour {
    position: vec4<f32>,
    fields: vec4<f32>,
    predicted: vec4<f32>,
    sound: vec4<f32>,
}

fn neighbour(i: u32) -> Neighbour {
    let n = params.count;
    return Neighbour(near[i], near[n + i], near[2u * n + i], near[3u * n + i]);
}

fn cube(x: f32) -> f32 {
    return x * x * x;
}

// The M4 cubic spline, w(q), normalized in 3D.
fn kernel(q: f32) -> f32 {
    if q < 1.0 {
        return (0.25 * cube(2.0 - q) - cube(1.0 - q)) / PI;
    }
    if q < 2.0 {
        return 0.25 * cube(2.0 - q) / PI;
    }
    return 0.0;
}

// dw/dq.
fn kernel_slope(q: f32) -> f32 {
    if q < 1.0 {
        return (-0.75 * (2.0 - q) * (2.0 - q) + 3.0 * (1.0 - q) * (1.0 - q)) / PI;
    }
    if q < 2.0 {
        return -0.75 * (2.0 - q) * (2.0 - q) / PI;
    }
    return 0.0;
}

// The kernel's mass within q.
fn kernel_mass(q: f32) -> f32 {
    if q < 1.0 {
        return q * q * q * (4.0 / 3.0 - 1.2 * q * q + 0.5 * q * q * q);
    }
    if q < 2.0 {
        let q3 = q * q * q;
        return 8.0 / 3.0 * q3 - 3.0 * q3 * q + 1.2 * q3 * q * q - q3 * q3 / 6.0 - 1.0 / 15.0;
    }
    return 1.0;
}

fn cell_of(x: vec3<f32>) -> vec3<i32> {
    return vec3<i32>(floor(x / state.cell));
}

fn hash(c: vec3<i32>) -> u32 {
    let h = (bitcast<u32>(c.x) * 73856093u) ^ (bitcast<u32>(c.y) * 19349663u)
        ^ (bitcast<u32>(c.z) * 83492791u);
    return h % params.table;
}

// How far the point x is from the cell c's box.
fn cell_distance(x: vec3<f32>, c: vec3<i32>) -> f32 {
    let low = vec3<f32>(c) * state.cell;
    return distance(x, clamp(x, low, low + vec3<f32>(state.cell)));
}

@compute @workgroup_size(64)
fn kick_drift(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = id.x;
    if a >= params.count {
        return;
    }
    var p = particles[a];
    let half = 0.5 * state.dt;
    let v = p.velocity.xyz + half * p.force.xyz;
    p.velocity = vec4<f32>(v, max(p.velocity.w + half * p.force.w, 0.0));
    p.fields.w = clamp(p.fields.w + half * p.extra.y, ALPHA_MIN, ALPHA_MAX);
    p.position = vec4<f32>(p.position.xyz + state.dt * v, p.position.w);
    particles[a] = p;
}

@compute @workgroup_size(256)
fn clear(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < params.table {
        atomicStore(&counts[id.x], 0u);
        atomicStore(&bucket_h[id.x], 0u);
    }
}

@compute @workgroup_size(64)
fn count(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = id.x;
    if a >= params.count {
        return;
    }
    let key = hash(cell_of(particles[a].position.xyz));
    particles[a].extra.w = bitcast<f32>(key);
    atomicAdd(&counts[key], 1u);
}

var<workgroup> scratch: array<u32, 256>;

// An inclusive scan of `scratch` (Hillis & Steele).
fn scan_scratch(lane: u32) {
    for (var offset = 1u; offset < GROUP; offset *= 2u) {
        var add = 0u;
        if lane >= offset {
            add = scratch[lane - offset];
        }
        workgroupBarrier();
        scratch[lane] += add;
        workgroupBarrier();
    }
}

// Each workgroup: the exclusive prefix sums of its BLOCK counts, and their
// total.
@compute @workgroup_size(256)
fn scan_blocks(
    @builtin(local_invocation_id) local: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let base = group.x * BLOCK + local.x * 4u;
    var own: array<u32, 4>;
    var sum = 0u;
    for (var k = 0u; k < 4u; k++) {
        own[k] = sum;
        sum += atomicLoad(&counts[base + k]);
    }
    scratch[local.x] = sum;
    workgroupBarrier();
    scan_scratch(local.x);
    let before = scratch[local.x] - sum;
    for (var k = 0u; k < 4u; k++) {
        starts[base + k] = before + own[k];
    }
    if local.x == GROUP - 1u {
        blocks[group.x] = scratch[local.x];
    }
}

// The blocks' totals, scanned (there are at most GROUP of them).
@compute @workgroup_size(256)
fn scan_sums(@builtin(local_invocation_id) local: vec3<u32>) {
    let blocks_used = params.table / BLOCK;
    var own = 0u;
    if local.x < blocks_used {
        own = blocks[local.x];
    }
    scratch[local.x] = own;
    workgroupBarrier();
    scan_scratch(local.x);
    if local.x < blocks_used {
        blocks[local.x] = scratch[local.x] - own;
    }
}

// Each bucket's start, and its count cleared to fill it again.
@compute @workgroup_size(256)
fn scan_add(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i < params.table {
        starts[i] += blocks[i / BLOCK];
        atomicStore(&counts[i], 0u);
    }
    if i == 0u {
        starts[params.table] = params.count;
    }
}

@compute @workgroup_size(64)
fn scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = id.x;
    if a >= params.count {
        return;
    }
    let key = bitcast<u32>(particles[a].extra.w);
    sorted[starts[key] + atomicAdd(&counts[key], 1u)] = a;
}

@compute @workgroup_size(64)
fn gather(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let p = particles[sorted[i]];
    near[i] = p.position;
    near[params.count + i] = p.fields;
}

// Σ m W(h) and Σ m ∂W/∂h at x, over the cells within 2h.
fn density_sums(x: vec3<f32>, h: f32) -> vec2<f32> {
    let c = cell_of(x);
    let reach = i32(ceil(2.0 * h / state.cell));
    var sums = vec2<f32>(0.0);
    for (var dz = -reach; dz <= reach; dz++) {
        for (var dy = -reach; dy <= reach; dy++) {
            for (var dx = -reach; dx <= reach; dx++) {
                let cc = c + vec3<i32>(dx, dy, dz);
                if cell_distance(x, cc) >= 2.0 * h {
                    continue;
                }
                let key = hash(cc);
                for (var s = starts[key]; s < starts[key + 1u]; s++) {
                    let o = near[s];
                    // Another cell may share the bucket.
                    if any(cell_of(o.xyz) != cc) {
                        continue;
                    }
                    let q = length(x - o.xyz) / h;
                    if q < 2.0 {
                        sums.x += o.w * kernel(q) / cube(h);
                        sums.y -= o.w * (3.0 * kernel(q) + q * kernel_slope(q)) / (cube(h) * h);
                    }
                }
            }
        }
    }
    return sums;
}

@compute @workgroup_size(64)
fn density(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let a = sorted[i];
    let own = neighbour(i);
    let x = own.position.xyz;
    var h = own.fields.x;
    for (var iteration = 0; iteration < 40; iteration++) {
        let sums = density_sums(x, h);
        let wanted = own.position.w * cube(ETA / h);
        let slope = sums.y + 3.0 * wanted / h;
        let next = clamp(h - (sums.x - wanted) / slope, 0.5 * h, 2.0 * h);
        let done = abs(next - h) < 1e-5 * h;
        h = next;
        if done {
            break;
        }
    }
    let sums = density_sums(x, h);
    let fields = vec4<f32>(h, sums.x, 1.0 + h / (3.0 * sums.x) * sums.y, own.fields.w);
    particles[a].fields = fields;
    near[params.count + i] = fields;
    atomicMax(&bucket_h[bitcast<u32>(particles[a].extra.w)], bitcast<u32>(h));
    atomicMax(&state.h_max, bitcast<u32>(h));
    atomicMin(&state.h_min, bitcast<u32>(h));
}

@compute @workgroup_size(64)
fn predict(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let p = particles[sorted[i]];
    let half = 0.5 * state.dt;
    let u = max(p.velocity.w + half * p.force.w, 0.0);
    let pressure = (params.gamma - 1.0) * p.fields.y * u;
    near[2u * params.count + i] = vec4<f32>(p.velocity.xyz + half * p.force.xyz, pressure);
    near[3u * params.count + i] = vec4<f32>(sqrt(params.gamma * (params.gamma - 1.0) * u), 0.0, 0.0, 0.0);
}

var<workgroup> sources: array<vec4<f32>, TILE>;

@compute @workgroup_size(64)
fn gravity(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
) {
    let n = params.count;
    let first = 2u * id.x;
    let x = near[min(first, n - 1u)].xyz;
    let y = near[min(first + 1u, n - 1u)].xyz;
    var gx = vec3<f32>(0.0);
    var gy = vec3<f32>(0.0);
    let tiles = (n + TILE - 1u) / TILE;
    for (var t = 0u; t < tiles; t++) {
        for (var j = 0u; j < 2u; j++) {
            let slot = j * 64u + local.x;
            let b = t * TILE + slot;
            if b < n {
                sources[slot] = near[b];
            } else {
                sources[slot] = vec4<f32>(0.0);
            }
        }
        workgroupBarrier();
        for (var k = 0u; k < TILE; k++) {
            let o = sources[k];
            // A particle adds nothing to itself: d = 0. Not 1e-30: its
            // cube would overflow, and 0 × ∞ is no number.
            let d = x - o.xyz;
            let e = y - o.xyz;
            let p = inverseSqrt(max(dot(d, d), 1e-20));
            let q = inverseSqrt(max(dot(e, e), 1e-20));
            gx -= d * (o.w * p * p * p);
            gy -= e * (o.w * q * q * q);
        }
        workgroupBarrier();
    }
    if first < n {
        near[3u * n + first] = vec4<f32>(near[3u * n + first].x, gx);
    }
    if first + 1u < n {
        near[3u * n + first + 1u] = vec4<f32>(near[3u * n + first + 1u].x, gy);
    }
}

@compute @workgroup_size(64)
fn forces(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let p = neighbour(i);
    let x = p.position.xyz;
    let ha = p.fields.x;
    let rho_a = p.fields.y;
    let own = p.predicted.w / (p.fields.z * rho_a * rho_a);
    // Gravity, as from points; softened below between neighbours.
    var acceleration = p.sound.yzw;
    var heating = 0.0;
    var compression = 0.0;
    var mu_max = 0.0;
    {
        let c = cell_of(x);
        let largest = bitcast<f32>(atomicLoad(&state.h_max));
        let reach = i32(ceil(2.0 * largest / state.cell));
        for (var dz = -reach; dz <= reach; dz++) {
            for (var dy = -reach; dy <= reach; dy++) {
                for (var dx = -reach; dx <= reach; dx++) {
                    let cc = c + vec3<i32>(dx, dy, dz);
                    let gap = cell_distance(x, cc);
                    if gap >= 2.0 * largest {
                        continue;
                    }
                    let key = hash(cc);
                    // Skip a cell nothing in which can reach this far.
                    let widest = bitcast<f32>(atomicLoad(&bucket_h[key]));
                    if gap >= 2.0 * max(ha, widest) {
                        continue;
                    }
                    for (var s = starts[key]; s < starts[key + 1u]; s++) {
                        if s == i {
                            continue;
                        }
                        let o = neighbour(s);
                        if any(cell_of(o.position.xyz) != cc) {
                            continue;
                        }
                        let d = x - o.position.xyz;
                        let r = length(d);
                        let hb = o.fields.x;
                        if r >= 2.0 * max(ha, hb) || r == 0.0 {
                            continue;
                        }
                        // Gravity softened by the kernels: what the tiles
                        // below pull as from a point, less the share of
                        // the kernels' mass outside r.
                        let m = o.position.w;
                        let inside = 0.5 * (kernel_mass(r / ha) + kernel_mass(r / hb));
                        acceleration += d * (m * (1.0 - inside) / (r * r * r));
                        let unit = d / r;
                        let ga = unit * (kernel_slope(r / ha) / (cube(ha) * ha));
                        let gb = unit * (kernel_slope(r / hb) / (cube(hb) * hb));
                        let theirs = o.predicted.w / (o.fields.z * o.fields.y * o.fields.y);
                        acceleration -= m * (own * ga + theirs * gb);
                        let v = p.predicted.xyz - o.predicted.xyz;
                        heating += own * m * dot(v, ga);
                        compression += m * dot(v, ga);
                        let closing = dot(v, d);
                        if closing < 0.0 {
                            let h = 0.5 * (ha + hb);
                            let mu = h * closing / (r * r + EPSILON * h * h);
                            let rho = 0.5 * (rho_a + o.fields.y);
                            let sound = 0.5 * (p.sound.x + o.sound.x);
                            let alpha = 0.5 * (p.fields.w + o.fields.w);
                            let pi = (-alpha * sound * mu + 2.0 * alpha * mu * mu) / rho;
                            let mean = 0.5 * (ga + gb);
                            acceleration -= m * pi * mean;
                            heating += 0.5 * m * pi * dot(v, mean);
                            mu_max = max(mu_max, abs(mu));
                        }
                    }
                }
            }
        }
    }
    {
        let a = sorted[i];
        let c = p.sound.x;
        let converging = max(compression / (p.fields.z * rho_a), 0.0);
        let switch_rate = -(p.fields.w - ALPHA_MIN) * SIGMA_DECAY * c / ha
            + converging * (ALPHA_MAX - p.fields.w);
        particles[a].force = vec4<f32>(acceleration, heating);
        particles[a].extra.y = switch_rate;
        particles[a].extra.z = mu_max;
        particles[a].extra.x = c;
        let courant = COURANT * ha / max(c + 1.2 * p.fields.w * (c + 2.0 * mu_max), 1e-30);
        let step = min(courant, FORCE * sqrt(ha / max(length(acceleration), 1e-30)));
        atomicMin(&state.dt_next, bitcast<u32>(step));
    }
}

@compute @workgroup_size(64)
fn kick(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = id.x;
    if a >= params.count {
        return;
    }
    let p = particles[a];
    let half = 0.5 * state.dt;
    var v = p.velocity.xyz + half * p.force.xyz;
    var u = max(p.velocity.w + half * p.force.w, 0.0);
    if params.settling > 0.5 {
        v *= exp(-state.dt * params.damping);
        u = params.entropy * pow(p.fields.y, params.gamma - 1.0) / (params.gamma - 1.0);
    }
    particles[a].velocity = vec4<f32>(v, u);
    particles[a].fields.w = clamp(p.fields.w + half * p.extra.y, ALPHA_MIN, ALPHA_MAX);
}

// The step's end: the clock, the next step, the next grid's cells.
@compute @workgroup_size(1)
fn finish() {
    state.time += state.dt;
    state.dt = bitcast<f32>(atomicLoad(&state.dt_next));
    // Cells of two of the smallest h, but no more than four to the
    // largest reach, 2h.
    let largest = bitcast<f32>(atomicLoad(&state.h_max));
    let smallest = bitcast<f32>(atomicLoad(&state.h_min));
    state.cell = 2.0 * max(smallest, 0.25 * largest);
    atomicStore(&state.dt_next, 0x7f7fffffu);
    atomicStore(&state.h_max, 0u);
    atomicStore(&state.h_min, 0x7f7fffffu);
}

// Every particle at rest (after settling).
@compute @workgroup_size(64)
fn rest(@builtin(global_invocation_id) id: vec3<u32>) {
    let a = id.x;
    if a < params.count {
        particles[a].velocity = vec4<f32>(vec3<f32>(0.0), particles[a].velocity.w);
        particles[a].force = vec4<f32>(vec3<f32>(0.0), 0.0);
    }
}
