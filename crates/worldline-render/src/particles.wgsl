// Draws a fluid's SPH particles (see fluid.wgsl) straight from the buffer
// the GPU steps them in: each as a small sphere facing the camera, of
// radius half its smoothing length, so neighbours just touch; lit by the
// Sun and colored by its internal energy, cool dark red to hot
// yellow-white. The half nearer the camera is cut away, to show the
// inside. Positions are relative to the camera, as in bodies.wgsl.

struct Globals {
    view_proj: mat4x4<f32>,
};

struct Draw {
    // The fluid's origin relative to the camera, m, and meters per unit of
    // its length.
    center: vec3<f32>,
    scale: f32,
    // The camera's right, and the internal energy drawn mid-scale.
    right: vec3<f32>,
    energy: f32,
    // The camera's up, and each sphere's radius in smoothing lengths.
    up: vec3<f32>,
    size: f32,
    // Unit vector toward the Sun.
    sun: vec3<f32>,
    _padding: f32,
};

struct Particle {
    position: vec4<f32>,
    velocity: vec4<f32>,
    fields: vec4<f32>,
    force: vec4<f32>,
    predicted: vec4<f32>,
    extra: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> draw: Draw;
@group(1) @binding(1) var<storage, read> particles: array<Particle>;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) color: vec3<f32>,
};

// Cool to hot, for t from 0 to 1.
fn heat(t: f32) -> vec3<f32> {
    let low = vec3<f32>(0.35, 0.05, 0.02);
    let middle = vec3<f32>(0.95, 0.45, 0.10);
    let high = vec3<f32>(1.0, 0.95, 0.80);
    if t < 0.5 {
        return mix(low, middle, 2.0 * t);
    }
    return mix(middle, high, 2.0 * t - 1.0);
}

@vertex
fn vs_particle(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) index: u32,
) -> Out {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[vertex];
    let p = particles[index];
    let radius = draw.size * p.fields.x * draw.scale;
    let world = draw.center + p.position.xyz * draw.scale
        + (draw.right * corner.x + draw.up * corner.y) * radius;
    var out: Out;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    // Cut away: particles on the camera's side of the plane through the
    // fluid's origin facing it, their quads collapsed to nothing.
    let toward_camera = -normalize(draw.center);
    if dot(p.position.xyz, toward_camera) > 0.0 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    out.corner = corner;
    // A factor of ten in energy spans the scale.
    let t = clamp(0.5 + log(max(p.velocity.w, 1e-30) / draw.energy) / log(10.0), 0.0, 1.0);
    out.color = heat(t);
    return out;
}

@fragment
fn fs_particle(in: Out) -> @location(0) vec4<f32> {
    let r2 = dot(in.corner, in.corner);
    if r2 > 1.0 {
        discard;
    }
    // The sphere's surface there, facing the camera.
    let toward_camera = normalize(cross(draw.right, draw.up));
    let normal = draw.right * in.corner.x + draw.up * in.corner.y + toward_camera * sqrt(1.0 - r2);
    let light = 0.25 + 0.75 * max(dot(normal, draw.sun), 0.0);
    return vec4<f32>(in.color * light, 1.0);
}
