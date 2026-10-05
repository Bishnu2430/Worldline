// Draws one body as a textured, lit, rotating sphere.
//
// Positions are relative to the camera. The depth buffer is reversed
// (1 at the near plane, 0 at infinity) for even precision from a few
// kilometers to billions.

struct Globals {
    view_proj: mat4x4<f32>,
};

struct Instance {
    // Rotation from the body's own frame (z = north pole, x = prime
    // meridian) to world axes.
    orientation: mat3x3<f32>,
    // Center relative to the camera, in meters.
    center: vec3<f32>,
    radius: f32,
    // Unit vector from the body toward the Sun.
    sun_direction: vec3<f32>,
    // 1 for bodies that shine by themselves (the Sun), 0 otherwise.
    emissive: f32,
    // Brightness of the night-side texture (Earth's city lights), 0 for none.
    night_glow: f32,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> instance: Instance;
@group(2) @binding(0) var day_texture: texture_2d<f32>;
@group(2) @binding(1) var night_texture: texture_2d<f32>;
@group(2) @binding(2) var surface_sampler: sampler;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    // Direction from the center in the body's own frame.
    @location(0) local: vec3<f32>,
    // Surface normal in world axes.
    @location(1) normal: vec3<f32>,
    // Position relative to the camera.
    @location(2) world: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VertexOut {
    let normal = instance.orientation * position;
    let world = instance.center + instance.radius * normal;
    var out: VertexOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.local = position;
    out.normal = normal;
    out.world = world;
    return out;
}

const PI: f32 = 3.14159265358979;

// A faint fill light so night sides don't vanish completely. A visual aid,
// not physics.
const FILL_LIGHT: f32 = 0.01;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Longitude and latitude on the body give the spot on its map
    // (equirectangular, longitude 0° in the middle, east to the right).
    let p = normalize(in.local);
    let longitude = atan2(p.y, p.x);
    let latitude = asin(clamp(p.z, -1.0, 1.0));
    let u = longitude / (2.0 * PI) + 0.5;
    let v = 0.5 - latitude / PI;

    // The map wraps around at ±180°, where u jumps from 1 to 0. Take the
    // sampling rate from a copy of u whose jump is on the opposite side, so
    // the seam doesn't show (Tarini 2012).
    let u_shifted = fract(u + 0.5);
    let du = vec2<f32>(dpdx(u), dpdy(u));
    let du_shifted = vec2<f32>(dpdx(u_shifted), dpdy(u_shifted));
    let seam_safe = select(du, du_shifted, abs(du_shifted.x) + abs(du_shifted.y) < abs(du.x) + abs(du.y));
    let ddx = vec2<f32>(seam_safe.x, dpdx(v));
    let ddy = vec2<f32>(seam_safe.y, dpdy(v));
    let uv = vec2<f32>(u, v);

    let surface = textureSampleGrad(day_texture, surface_sampler, uv, ddx, ddy).rgb;
    let night = textureSampleGrad(night_texture, surface_sampler, uv, ddx, ddy).rgb;
    let n = normalize(in.normal);

    if instance.emissive > 0.5 {
        // Limb darkening: near the Sun's edge we see higher, cooler gas, so
        // it looks dimmer. Eddington approximation: I(μ) / I(1) = 0.4 + 0.6 μ,
        // where μ is the cosine of the angle from the line of sight.
        let mu = max(dot(n, normalize(-in.world)), 0.0);
        return vec4<f32>(surface * (0.4 + 0.6 * mu), 1.0);
    }

    // Sunlight falls off with the cosine of the angle from overhead (Lambert).
    let sunlight = dot(n, instance.sun_direction);
    var color = surface * (max(sunlight, 0.0) + FILL_LIGHT);
    // Night lights fade in just past the terminator.
    color += night * instance.night_glow * (1.0 - smoothstep(-0.15, 0.05, sunlight));
    return vec4<f32>(color, 1.0);
}
