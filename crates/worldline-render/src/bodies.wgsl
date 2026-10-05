// Draws bodies: textured, lit, rotating globes; thin atmospheres; rings.
//
// Positions are relative to the camera. The depth buffer is reversed
// (1 at the near plane, 0 at infinity) for even precision from a few
// kilometers to billions. Outputs are premultiplied by coverage.

struct Globals {
    view_proj: mat4x4<f32>,
};

struct Instance {
    // Rotation from the body's own frame (z = north pole, x = prime
    // meridian) to world axes.
    orientation: mat3x3<f32>,
    // Center relative to the camera, in meters.
    center: vec3<f32>,
    // 1 for bodies that shine by themselves (the Sun), 0 otherwise.
    emissive: f32,
    // Triaxial radii in the body's frame (equatorial, equatorial, polar), m.
    radii: vec3<f32>,
    // Brightness of the night-side texture (Earth's city lights), 0 for none.
    night_glow: f32,
    // Unit vector from the body toward the Sun.
    sun_direction: vec3<f32>,
    // 1 if the body has a cloud map, 0 otherwise.
    clouds: f32,
    // Radial range covered by the ring profile, m; outer = 0 for no rings.
    ring_inner: f32,
    ring_outer: f32,
    // Atmosphere: height of its top above the surface and its scale height,
    // m (height = 0 for none), and Rayleigh coefficients per m at the surface.
    atmosphere_height: f32,
    scale_height: f32,
    rayleigh: vec3<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> instance: Instance;
@group(2) @binding(0) var day_texture: texture_2d<f32>;
@group(2) @binding(1) var night_texture: texture_2d<f32>;
@group(2) @binding(2) var cloud_texture: texture_2d<f32>;
@group(2) @binding(3) var surface_sampler: sampler;
// Rings: fraction of light passing straight through, e^(−τ), versus radius.
@group(2) @binding(4) var ring_texture: texture_2d<f32>;
@group(2) @binding(5) var ring_sampler: sampler;

const PI: f32 = 3.14159265358979;

// A faint fill light so night sides don't vanish completely. A visual aid,
// not physics.
const FILL_LIGHT: f32 = 0.01;

// ---------------------------------------------------------------- helpers

fn to_body(v: vec3<f32>) -> vec3<f32> {
    return transpose(instance.orientation) * v;
}

// Position along the ring profile texture for a distance from the center.
fn ring_coordinate(radius: f32) -> f32 {
    return (radius - instance.ring_inner) / (instance.ring_outer - instance.ring_inner);
}

// Whether a ray from `point` (outside the body) toward `direction`, both in
// the body's frame, runs into the body's ellipsoid.
fn blocked_by_body(point: vec3<f32>, direction: vec3<f32>) -> bool {
    let o = point / instance.radii;
    let d = direction / instance.radii;
    let a = dot(d, d);
    let b = dot(o, d);
    let c = dot(o, o) - 1.0;
    // From outside (c > 0), both crossings lie ahead exactly when b < 0.
    return b * b - a * c > 0.0 && b < 0.0 && c > 0.0;
}

// ---------------------------------------------------------------- globes

struct GlobeOut {
    @builtin(position) clip: vec4<f32>,
    // Direction on the unit sphere in the body's frame: picks the map pixel.
    @location(0) direction: vec3<f32>,
    // Surface point in the body's frame, m.
    @location(1) local: vec3<f32>,
    // Surface normal in world axes.
    @location(2) normal: vec3<f32>,
    // Position relative to the camera.
    @location(3) world: vec3<f32>,
};

@vertex
fn vs_globe(@location(0) position: vec3<f32>) -> GlobeOut {
    // Stretch the unit sphere into the body's ellipsoid. An ellipsoid's
    // normal is the position divided by the radii, normalized.
    let local = position * instance.radii;
    let normal = normalize(position / instance.radii);
    let world = instance.center + instance.orientation * local;
    var out: GlobeOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.direction = position;
    out.local = local;
    out.normal = instance.orientation * normal;
    out.world = world;
    return out;
}

@fragment
fn fs_globe(in: GlobeOut) -> @location(0) vec4<f32> {
    // Longitude and latitude on the body give the spot on its map
    // (equirectangular, longitude 0° in the middle, east to the right).
    let p = normalize(in.direction);
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
    let cloud = textureSampleGrad(cloud_texture, surface_sampler, uv, ddx, ddy).r * instance.clouds;

    // Ring shadow: follow the sunbeam from this point back to the ring
    // plane and look up how much of it the rings let through there.
    let sun_body = to_body(instance.sun_direction);
    let beam = -in.local.z / sun_body.z;
    let crossing = in.local + beam * sun_body;
    let ring_u = ring_coordinate(length(crossing.xy));
    let ring_du = vec2<f32>(dpdx(ring_u), dpdy(ring_u));
    let ring_through = textureSampleGrad(ring_texture, ring_sampler, vec2<f32>(ring_u, 0.5), vec2<f32>(ring_du.x, 0.0), vec2<f32>(ring_du.y, 0.0)).r;

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
    var light = max(sunlight, 0.0);
    // Through the rings the beam travels a slanted path, so it keeps
    // e^(−τ/μ₀) = (e^(−τ))^(1/μ₀), where μ₀ is the Sun's height above the
    // ring plane.
    let under_rings = instance.ring_outer > 0.0 && beam > 0.0 && ring_u >= 0.0 && ring_u <= 1.0;
    let slant = 1.0 / max(abs(sun_body.z), 1e-3);
    light *= select(1.0, pow(max(ring_through, 1e-4), slant), under_rings);

    var color = surface * (light + FILL_LIGHT);
    // Clouds are white and lit like the ground; they hide what's below.
    color = mix(color, vec3<f32>(light + FILL_LIGHT), cloud);
    // Night lights fade in just past the terminator, and clouds dim them.
    color += night * instance.night_glow * (1.0 - cloud) * (1.0 - smoothstep(-0.15, 0.05, sunlight));
    return vec4<f32>(color, 1.0);
}

// ---------------------------------------------------------------- atmospheres

struct AirOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn vs_atmosphere(@location(0) position: vec3<f32>) -> AirOut {
    let top = instance.radii.x + instance.atmosphere_height;
    let world = instance.center + position * top;
    var out: AirOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    return out;
}

// Optical depth (per color) from `point` toward the Sun to the top of the
// atmosphere, or a huge value if the planet blocks the Sun. `point` is
// relative to the planet's center.
fn optical_depth_to_sun(point: vec3<f32>, ground: f32, top: f32) -> vec3<f32> {
    let s = instance.sun_direction;
    let b = dot(point, s);
    let miss = dot(point, point) - b * b;
    if b < 0.0 && miss < ground * ground {
        return vec3<f32>(1e9);
    }
    let path = -b + sqrt(max(top * top - miss, 0.0));
    let steps = 8;
    let ds = path / f32(steps);
    var density = 0.0;
    for (var i = 0; i < steps; i++) {
        let q = point + s * ((f32(i) + 0.5) * ds);
        density += exp(-(length(q) - ground) / instance.scale_height) * ds;
    }
    return instance.rayleigh * density;
}

// Single-scattering Rayleigh sky (after Nishita et al. 1993): sunlight
// scattered toward the camera by air along the line of sight, dimmed on the
// way in and on the way out. Output is in the same units as lit surfaces
// (reflectance I/F with the Sun's irradiance as 1).
@fragment
fn fs_atmosphere(in: AirOut) -> @location(0) vec4<f32> {
    let dir = normalize(in.world);
    let center = instance.center;
    let ground = instance.radii.x;
    let top = ground + instance.atmosphere_height;

    // Where the line of sight enters and leaves the atmosphere, stopping at
    // the ground if it hits the planet.
    let along = dot(dir, center);
    let nearest = along * dir - center;
    let miss = dot(nearest, nearest);
    if miss >= top * top {
        discard;
    }
    let half_chord = sqrt(top * top - miss);
    let t0 = max(along - half_chord, 0.0);
    var t1 = along + half_chord;
    if miss < ground * ground {
        t1 = min(t1, along - sqrt(ground * ground - miss));
    }

    let steps = 16;
    let dt = (t1 - t0) / f32(steps);
    var to_camera = vec3<f32>(0.0);
    var scattered = vec3<f32>(0.0);
    for (var i = 0; i < steps; i++) {
        let point = (t0 + (f32(i) + 0.5) * dt) * dir - center;
        let density = exp(-(length(point) - ground) / instance.scale_height);
        let here = instance.rayleigh * density * dt;
        let toward_sun = optical_depth_to_sun(point, ground, top);
        scattered += density * dt * exp(-(to_camera + 0.5 * here + toward_sun));
        to_camera += here;
    }
    let cos_angle = dot(dir, instance.sun_direction);
    let phase = 3.0 / (16.0 * PI) * (1.0 + cos_angle * cos_angle);
    let color = PI * phase * instance.rayleigh * scattered;
    // What's behind is dimmed by the air in between (gray average).
    let transmitted = exp(-to_camera);
    let coverage = 1.0 - (transmitted.r + transmitted.g + transmitted.b) / 3.0;
    return vec4<f32>(color, coverage);
}

// ---------------------------------------------------------------- rings

struct RingOut {
    @builtin(position) clip: vec4<f32>,
    // Point in the ring plane, body frame, m.
    @location(0) local: vec3<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_ring(@location(0) position: vec3<f32>) -> RingOut {
    // A square in the equatorial plane, as wide as the outermost ring.
    let local = vec3<f32>(position.xy * instance.ring_outer, 0.0);
    let world = instance.center + instance.orientation * local;
    var out: RingOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.local = local;
    out.world = world;
    return out;
}

// Ring particles in the A and B rings are brighter than those in the C
// ring and Cassini Division. Two levels: a visual approximation.
fn ring_albedo(radius: f32) -> f32 {
    let b_ring = radius >= 91975e3 && radius <= 117570e3;
    let a_ring = radius >= 122050e3 && radius <= 136770e3;
    return select(0.25, 0.55, b_ring || a_ring);
}

const RING_TINT: vec3<f32> = vec3<f32>(0.86, 0.78, 0.64);

@fragment
fn fs_ring(in: RingOut) -> @location(0) vec4<f32> {
    let radius = length(in.local.xy);
    let u = ring_coordinate(radius);
    let du = vec2<f32>(dpdx(u), dpdy(u));
    let straight_through = textureSampleGrad(ring_texture, ring_sampler, vec2<f32>(u, 0.5), vec2<f32>(du.x, 0.0), vec2<f32>(du.y, 0.0)).r;
    if u < 0.0 || u > 1.0 {
        discard;
    }
    // Measured normal optical depth τ.
    let tau = -log(max(straight_through, 1e-4));

    let normal = instance.orientation[2];
    let to_camera = normalize(-in.world);
    let mu = max(abs(dot(normal, to_camera)), 1e-3);
    let mu0 = max(abs(dot(normal, instance.sun_direction)), 1e-3);
    // Seen at a slant, the line of sight crosses more ring: e^(−τ/μ) gets through.
    let coverage = 1.0 - exp(-tau / mu);
    // Fraction of the slanting sunbeam the particles catch.
    let caught = 1.0 - exp(-tau / mu0);
    // From the unlit side only light that diffuses through reaches us.
    let lit_side = dot(normal, to_camera) * dot(normal, instance.sun_direction) > 0.0;
    let through = select(exp(-tau / mu), 1.0, lit_side);
    // Saturn's shadow across the rings.
    let in_shadow = blocked_by_body(in.local, to_body(instance.sun_direction));
    let sunlit = select(1.0, 0.0, in_shadow);

    let albedo = ring_albedo(radius);
    // A layer of Lambert-like particles: brightness ∝ albedo · μ₀ · caught.
    let brightness = albedo * (mu0 * caught * through * sunlit + FILL_LIGHT * coverage);
    return vec4<f32>(RING_TINT * brightness, coverage);
}
