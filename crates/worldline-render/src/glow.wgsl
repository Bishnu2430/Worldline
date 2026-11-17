// The galaxy's stars as a glow (see glow.rs). Each star is a 3D Gaussian of
// mass, standard deviation h, and a pixel shows the column of mass along its
// ray, from the camera outward:
//
//   Σ = w/(4πh²) · exp(−b²/2h²) · (1 + erf(s/(√2 h)))
//
// with w the star's weight, b the ray's closest approach to the star and s
// how far along the ray that is (negative if the star is behind). It's
// exact from anywhere, inside a star's spread too. Seen from far off, it's
// the projected Gaussian w/(2πh²) exp(−b²/2h²). Lengths are in any one
// unit; the column is weight per unit length squared.
//
// fs_star adds each star's column into a float target; fs_tone maps the sum
// to something to show.

struct View {
    right: vec3<f32>,
    tan_half_fov: f32,
    up: vec3<f32>,
    aspect: f32,
    forward: vec3<f32>,
    // A star is drawn out to this many standard deviations.
    cutoff: f32,
    // What the column is multiplied by before it's tone mapped.
    exposure: f32,
    // The half-angle of a cone around `forward` that holds the whole view.
    frustum: f32,
    padding: vec2<f32>,
};

struct Star {
    // Where it is relative to the camera, and its standard deviation h.
    place: vec4<f32>,
    // Its linear color times its weight.
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<storage, read> stars: array<Star>;
@group(0) @binding(2) var column: texture_2d<f32>;

const PI: f32 = 3.14159265358979;

struct Out {
    @builtin(position) clip: vec4<f32>,
    // Where on the screen, -1 to 1 across and up.
    @location(0) ndc: vec2<f32>,
    @location(1) @interpolate(flat) index: u32,
};

// Where direction d lands on the screen.
fn project(d: vec3<f32>) -> vec2<f32> {
    let w = dot(d, view.forward);
    return vec2<f32>(
        dot(d, view.right) / (w * view.tan_half_fov * view.aspect),
        dot(d, view.up) / (w * view.tan_half_fov),
    );
}

// The ray through a point on the screen.
fn ray(ndc: vec2<f32>) -> vec3<f32> {
    return normalize(
        view.forward + view.right * (ndc.x * view.tan_half_fov * view.aspect)
            + view.up * (ndc.y * view.tan_half_fov),
    );
}

// Each star is a square around the cone it can light, or, close up, the
// whole screen.
@vertex
fn vs_star(
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
    let p = stars[index].place.xyz;
    let reach = view.cutoff * stars[index].place.w;
    let d = length(p);
    var out: Out;
    out.index = index;
    // Hidden: a square of no area, off the screen.
    out.clip = vec4<f32>(2.0, 2.0, 0.5, 1.0);
    out.ndc = vec2<f32>(2.0, 2.0);
    // Farther than its reach by more than 2/√3, it lights a cone narrower
    // than 60°: a square one unit along it, as wide as the cone there,
    // covers it on the screen, if all its corners are in front.
    if d > 1.155 * reach {
        let toward = p / d;
        let half = asin(reach / d);
        let off = acos(clamp(dot(toward, view.forward), -1.0, 1.0));
        if off > half + view.frustum {
            return out;
        }
        let pole = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(toward.z) > 0.9);
        let side = normalize(cross(toward, pole));
        let other = cross(toward, side);
        let wide = tan(half);
        var front = true;
        for (var k = 0u; k < 4u; k++) {
            let c = vec2<f32>(select(-1.0, 1.0, (k & 1u) != 0u), select(-1.0, 1.0, (k & 2u) != 0u));
            let q = toward + (side * c.x + other * c.y) * wide;
            if dot(q, view.forward) < 1e-3 {
                front = false;
            }
        }
        if front {
            let s = project(toward + (side * corner.x + other * corner.y) * wide);
            out.clip = vec4<f32>(s, 0.5, 1.0);
            out.ndc = s;
            return out;
        }
    }
    out.clip = vec4<f32>(corner, 0.5, 1.0);
    out.ndc = corner;
    return out;
}

// erf, to 1.5 × 10⁻⁷ (Abramowitz & Stegun 1964, 7.1.26).
fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.3275911 * abs(x));
    let y = 1.0 - ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t
        + 0.254829592) * t * exp(-x * x);
    return select(-y, y, x >= 0.0);
}

@fragment
fn fs_star(in: Out) -> @location(0) vec4<f32> {
    let star = stars[in.index];
    let p = star.place.xyz;
    let h = star.place.w;
    let look = ray(in.ndc);
    // Along the ray to the closest approach, and how close: the cross
    // product keeps that exact for a star far off and nearly straight ahead.
    let s = dot(look, p);
    let b = length(cross(look, p)) / h;
    let sigma = exp(-0.5 * b * b) * (1.0 + erf(s / (sqrt(2.0) * h))) / (4.0 * PI * h * h);
    return vec4<f32>(star.color.rgb * sigma, 0.0);
}

// A triangle covering the screen.
@vertex
fn vs_tone(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    let x = f32(vertex & 1u) * 4.0 - 1.0;
    let y = f32(vertex >> 1u) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// The column, times the exposure, as light: its luminance L shown as
// L/(1 + L) (Reinhard et al. 2002), colors kept, and as transparent as it is
// dark, premultiplied, to lie over the sky.
@fragment
fn fs_tone(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(column, vec2<i32>(pixel.xy), 0).rgb * view.exposure;
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    if l <= 0.0 {
        return vec4<f32>(0.0);
    }
    let shown = l / (1.0 + l);
    return vec4<f32>(c * (shown / l), shown);
}
