//! What gravity does at a point, for the view's overlays: how fast a clock
//! at rest there runs, how it stretches and squeezes things, and the shape
//! of space around a body. See `docs/physics/overlays.md`.

use glam::{DMat3, DVec3};

use crate::Body;
use crate::compact::{horizon_spin, is_black_hole};
use crate::constants::C;
use crate::gravity::kerr::field;

/// One body's share of how slowly a clock at rest at `point` runs, U/c²:
/// - outside it, GM/(rc²);
/// - inside, as a sphere of even density, GM(3R² − r²)/(2R³c²);
/// - near a black hole, its exact f/2 = GMr/(Σc²), the Kerr–Schild
///   metric's g_tt = −(1 − f), for its spin.
fn potential(body: &Body, point: DVec3) -> f64 {
    let d = point - body.position;
    let m = body.gm / (C * C);
    if is_black_hole(body) {
        let spin = horizon_spin(body.gm, body.radius);
        let (f, _) = field(d, m, spin * m);
        return 0.5 * f.value;
    }
    let r = d.length();
    let big = body.radius;
    if r >= big {
        m / r
    } else {
        m * (3.0 * big * big - r * r) / (2.0 * big.powi(3))
    }
}

/// How much more slowly a clock at rest at `point` runs than one at rest
/// far from all of `bodies`: 1 − dτ/dt, with dτ/dt = √(1 − 2U/c²) and U the
/// sum of their potentials, written 2U/(1 + √(1 − 2U)) so that tiny
/// slowdowns keep their digits.
///
/// For one body, outside it, this is exact: Schwarzschild's (or, for a
/// spinning hole, Kerr's) static clocks. For several it is the sum of
/// their weak fields, right to first order in U/c² (Einstein 1911; the
/// first post-Newtonian metric, g_tt = −(1 − 2U/c²)). The Milky Way's own
/// pull, the same across the solar system, is left out.
///
/// `None` where nothing can stay at rest: inside a horizon, or a spinning
/// hole's ergosphere.
pub fn clock_slowdown(bodies: &[Body], point: DVec3) -> Option<f64> {
    let u: f64 = bodies
        .iter()
        .filter(|b| b.gm > 0.0)
        .map(|b| potential(b, point))
        .sum();
    let rest = 1.0 - 2.0 * u;
    (rest > 0.0).then(|| 2.0 * u / (1.0 + rest.sqrt()))
}

/// The tidal field at `point` from `bodies`, leaving out the one at
/// `skip`: the tensor T with which the pull at point + δ, less the pull at
/// point, is T δ (1/s²). Each body adds GM/d³ (3n̂n̂ᵀ − I), n̂ the unit
/// vector from it to the point at distance d: a stretch of 2GM/d³ along
/// n̂ and a squeeze of GM/d³ across it. Inside a body of even density, its
/// pull falls linearly toward its center, and it adds −GM/R³ I.
///
/// This is Newton's. For a black hole that doesn't spin it is also exactly
/// what a clock at rest there measures: in its frame the Riemann tensor
/// stretches by 2GM/r³ along r and squeezes by GM/r³ across, with r its
/// areal radius (Misner, Thorne & Wheeler 1973, §31.2).
pub fn tidal_tensor(bodies: &[Body], point: DVec3, skip: Option<usize>) -> DMat3 {
    let mut tensor = DMat3::ZERO;
    for (i, body) in bodies.iter().enumerate() {
        if Some(i) == skip || body.gm <= 0.0 {
            continue;
        }
        let d = point - body.position;
        let r = d.length();
        if r < body.radius {
            tensor -= DMat3::IDENTITY * (body.gm / body.radius.powi(3));
            continue;
        }
        let n = d / r;
        let outer = DMat3::from_cols(n * n.x, n * n.y, n * n.z);
        tensor += (outer * 3.0 - DMat3::IDENTITY) * (body.gm / r.powi(3));
    }
    tensor
}

/// The principal stretches of a symmetric tensor: its eigenvalues, largest
/// first, and their unit axes, by Jacobi's rotations (e.g. Press et al.,
/// *Numerical Recipes*, §11.1).
pub fn principal_axes(tensor: DMat3) -> [(f64, DVec3); 3] {
    let mut a = tensor.to_cols_array_2d();
    let mut v = DMat3::IDENTITY.to_cols_array_2d();
    for _ in 0..50 {
        // The largest off-diagonal element.
        let (p, q) = [(0, 1), (0, 2), (1, 2)]
            .into_iter()
            .max_by(|&(i, j), &(k, l)| a[i][j].abs().total_cmp(&a[k][l].abs()))
            .unwrap_or((0, 1));
        let scale = a[0][0].abs() + a[1][1].abs() + a[2][2].abs();
        if a[p][q].abs() <= 1e-15 * scale {
            break;
        }
        // The rotation in the p–q plane that zeroes it.
        let theta = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
        let (s, c) = theta.sin_cos();
        let mut rotation = DMat3::IDENTITY.to_cols_array_2d();
        rotation[p][p] = c;
        rotation[q][q] = c;
        // Column q, row p: R_pq = s.
        rotation[q][p] = s;
        rotation[p][q] = -s;
        let r = DMat3::from_cols_array_2d(&rotation);
        let m = DMat3::from_cols_array_2d(&a);
        a = (r.transpose() * m * r).to_cols_array_2d();
        v = (DMat3::from_cols_array_2d(&v) * r).to_cols_array_2d();
    }
    let mut axes = [0, 1, 2].map(|k| (a[k][k], DVec3::from_array(v[k])));
    axes.sort_by(|x, y| y.0.total_cmp(&x.0));
    axes
}

/// The height of a body's equatorial plane at radius `r` (m), embedded in
/// flat space so that distances within it are true (Flamm 1916): measured
/// up from where its center (or a black hole's horizon) sits.
/// - Outside the body, Flamm's paraboloid: z = 2√(r_s(r − r_s)), with
///   r_s = 2GM/c².
/// - Inside a body of even density (Schwarzschild's interior solution), a
///   cap of a sphere of radius √K, K = R³/r_s:
///   z = z(R) + √(K − R²) − √(K − r²), written
///   z(R) − (R² − r²)/(√(K − R²) + √(K − r²)) so that it keeps its digits.
///
/// For a spinning hole it is drawn as for one that doesn't spin, from r_s.
pub fn embedding_height(gm: f64, radius: f64, r: f64) -> f64 {
    let rs = 2.0 * gm / (C * C);
    let outside = |r: f64| 2.0 * (rs * (r - rs).max(0.0)).sqrt();
    if r >= radius || radius <= rs {
        return outside(r.max(rs));
    }
    let k = radius.powi(3) / rs;
    outside(radius)
        - (radius * radius - r * r) / ((k - radius * radius).sqrt() + (k - r * r).sqrt())
}

/// How much longer a ruler laid straight out from radius `from` to `to`
/// (m, both outside the body) is than to − from: the radial distance is
/// ∫ dr/√(1 − r_s/r) = [√(r(r − r_s)) + r_s ln(√r + √(r − r_s))]. With
/// √(r(r − r_s)) − r written −r_s r/(√(r(r − r_s)) + r), the tiny excess of
/// a weak field keeps its digits.
pub fn ruler_excess(gm: f64, from: f64, to: f64) -> f64 {
    let rs = 2.0 * gm / (C * C);
    let part = |r: f64| {
        let root = (r * (r - rs)).sqrt();
        -rs * r / (root + r) + rs * (r.sqrt() + (r - rs).sqrt()).ln()
    };
    part(to) - part(from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn principal_axes_are_eigenvectors() {
        let m = DMat3::from_cols(
            DVec3::new(4.0, 1.0, -2.0),
            DVec3::new(1.0, 2.0, 0.5),
            DVec3::new(-2.0, 0.5, -3.0),
        );
        let axes = principal_axes(m);
        for (value, axis) in axes {
            assert!((m * axis - axis * value).length() < 1e-12);
            assert!((axis.length() - 1.0).abs() < 1e-12);
        }
        assert!(axes[0].0 >= axes[1].0 && axes[1].0 >= axes[2].0);
        let trace: f64 = axes.iter().map(|a| a.0).sum();
        assert!((trace - 3.0).abs() < 1e-12);
    }
}
