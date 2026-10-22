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
