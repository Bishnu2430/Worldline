use glam::DVec3;

use super::{EinsteinInfeldHoffmann, Gravity};
use crate::Body;
use crate::constants::C;

/// The parts of one body's acceleration, caused by another, beyond first
/// post-Newtonian order, in m/s².
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairTerms {
    /// Second post-Newtonian order (2PN): conservative, of relative size
    /// (GM/rc²)² next to Newton's pull.
    pub second: DVec3,
    /// Radiation reaction (2.5PN): the pull that drains the pair's energy
    /// into gravitational waves, of relative size (GM/rc²)^(5/2).
    pub reaction: DVec3,
    /// The radiation reaction's first correction (3.5PN), GM/rc² of it,
    /// which makes the energy drain match the waves' power to that order.
    pub reaction_correction: DVec3,
}

/// The 2PN, 2.5PN and 3.5PN parts of body 1's acceleration due to body 2,
/// from their gravitational parameters `gm1`, `gm2` (m³/s²), positions
/// `y1`, `y2` and velocities `v1`, `v2`, in harmonic coordinates, in any
/// frame.
///
/// The c⁻⁴, c⁻⁵ and c⁻⁷ terms of the two-body equations of motion: Blanchet
/// (2024), *Living Rev. Relativ.* 27, 4, section "The 3.5PN acceleration
/// and 3PN energy" (accelerations already reduced to positions and
/// velocities); the c⁻⁷ terms in this form from Nissanke & Blanchet (2005),
/// *Class. Quantum Grav.* 22, 1007. Both print four of the G⁴m⁴ terms over
/// r⁶; their units, and the center-of-mass equations, require r⁵, as the
/// others have. Body 2's come from swapping the labels.
pub fn pair_terms(gm1: f64, gm2: f64, y1: DVec3, y2: DVec3, v1: DVec3, v2: DVec3) -> PairTerms {
    let d = y1 - y2;
    let r = d.length();
    let n = d / r;
    let v12 = v1 - v2;
    let (nv1, nv2, nv12) = (n.dot(v1), n.dot(v2), n.dot(v12));
    let (v1s, v2s, v1v2, v12s) = (
        v1.length_squared(),
        v2.length_squared(),
        v1.dot(v2),
        v12.length_squared(),
    );
    let (r2, r3, r4) = (r * r, r * r * r, r * r * r * r);
    // Products of the parameters: G²m₁m₂ = gm₁ gm₂ and so on.
    let (m12, m22, m112, m122, m222) = (
        gm1 * gm2,
        gm2 * gm2,
        gm1 * gm1 * gm2,
        gm1 * gm2 * gm2,
        gm2 * gm2 * gm2,
    );

    // 2PN, along n₁₂ and along v₁₂.
    let along_n = -57.0 / 4.0 * m112 / r4 - 69.0 / 2.0 * m122 / r4 - 9.0 * m222 / r4
        + gm2 / r2
            * (-15.0 / 8.0 * nv2.powi(4) + 1.5 * nv2 * nv2 * v1s
                - 6.0 * nv2 * nv2 * v1v2
                - 2.0 * v1v2 * v1v2
                + 4.5 * nv2 * nv2 * v2s
                + 4.0 * v1v2 * v2s
                - 2.0 * v2s * v2s)
        + m12 / r3
            * (19.5 * nv1 * nv1 - 39.0 * nv1 * nv2 + 8.5 * nv2 * nv2 - 3.75 * v1s - 2.5 * v1v2
                + 1.25 * v2s)
        + m22 / r3 * (2.0 * nv1 * nv1 - 4.0 * nv1 * nv2 - 6.0 * nv2 * nv2 - 8.0 * v1v2 + 4.0 * v2s);
    let along_v = m22 / r3 * (-2.0 * nv1 - 2.0 * nv2)
        + m12 / r3 * (-63.0 / 4.0 * nv1 + 55.0 / 4.0 * nv2)
        + gm2 / r2
            * (-6.0 * nv1 * nv2 * nv2 + 4.5 * nv2.powi(3) + nv2 * v1s - 4.0 * nv1 * v1v2
                + 4.0 * nv2 * v1v2
                + 4.0 * nv1 * v2s
                - 5.0 * nv2 * v2s);
    let second = (n * along_n + v12 * along_v) / C.powi(4);

    // 2.5PN.
    let along_n =
        (208.0 / 15.0 * m122 / r4 - 24.0 / 5.0 * m112 / r4 + 12.0 / 5.0 * m12 / r3 * v12s) * nv12;
    let along_v = 8.0 / 5.0 * m112 / r4 - 32.0 / 5.0 * m122 / r4 - 4.0 / 5.0 * m12 / r3 * v12s;
    let reaction = (n * along_n + v12 * along_v) / C.powi(5);

    // 3.5PN.
    let r5 = r4 * r;
    let (m1112, m1122, m1222) = (m112 * gm1, m112 * gm2, m122 * gm2);
    let along_n = m1112 / r5 * (3992.0 / 105.0 * nv1 - 4328.0 / 105.0 * nv2)
        + m1122 / r5 * (-13576.0 / 105.0 * nv1 + 2872.0 / 21.0 * nv2)
        - 3172.0 / 21.0 * m1222 / r5 * nv12
        + m112 / r4
            * (48.0 * nv1.powi(3) - 696.0 / 5.0 * nv1 * nv1 * nv2 + 744.0 / 5.0 * nv1 * nv2 * nv2
                - 288.0 / 5.0 * nv2.powi(3)
                - 4888.0 / 105.0 * nv1 * v1s
                + 5056.0 / 105.0 * nv2 * v1s
                + 2056.0 / 21.0 * nv1 * v1v2
                - 2224.0 / 21.0 * nv2 * v1v2
                - 1028.0 / 21.0 * nv1 * v2s
                + 5812.0 / 105.0 * nv2 * v2s)
        + m122 / r4
            * (-582.0 / 5.0 * nv1.powi(3) + 1746.0 / 5.0 * nv1 * nv1 * nv2
                - 1954.0 / 5.0 * nv1 * nv2 * nv2
                + 158.0 * nv2.powi(3)
                + 3568.0 / 105.0 * nv12 * v1s
                - 2864.0 / 35.0 * nv1 * v1v2
                + 10048.0 / 105.0 * nv2 * v1v2
                + 1432.0 / 35.0 * nv1 * v2s
                - 5752.0 / 105.0 * nv2 * v2s)
        + m12 / r3
            * (-56.0 * nv12.powi(5) + 60.0 * nv1.powi(3) * v12s - 180.0 * nv1 * nv1 * nv2 * v12s
                + 174.0 * nv1 * nv2 * nv2 * v12s
                - 54.0 * nv2.powi(3) * v12s
                - 246.0 / 35.0 * nv12 * v1s * v1s
                + 1068.0 / 35.0 * nv1 * v1s * v1v2
                - 984.0 / 35.0 * nv2 * v1s * v1v2
                - 1068.0 / 35.0 * nv1 * v1v2 * v1v2
                + 180.0 / 7.0 * nv2 * v1v2 * v1v2
                - 534.0 / 35.0 * nv1 * v1s * v2s
                + 90.0 / 7.0 * nv2 * v1s * v2s
                + 984.0 / 35.0 * nv1 * v1v2 * v2s
                - 732.0 / 35.0 * nv2 * v1v2 * v2s
                - 204.0 / 35.0 * nv1 * v2s * v2s
                + 24.0 / 7.0 * nv2 * v2s * v2s);
    let along_v = -184.0 / 21.0 * m1112 / r5
        + 6224.0 / 105.0 * m1122 / r5
        + 6388.0 / 105.0 * m1222 / r5
        + m112 / r4
            * (52.0 / 15.0 * nv1 * nv1
                - 56.0 / 15.0 * nv1 * nv2
                - 44.0 / 15.0 * nv2 * nv2
                - 132.0 / 35.0 * v1s
                + 152.0 / 35.0 * v1v2
                - 48.0 / 35.0 * v2s)
        + m122 / r4
            * (454.0 / 15.0 * nv1 * nv1 - 372.0 / 5.0 * nv1 * nv2 + 854.0 / 15.0 * nv2 * nv2
                - 152.0 / 21.0 * v1s
                + 2864.0 / 105.0 * v1v2
                - 1768.0 / 105.0 * v2s)
        + m12 / r3
            * (60.0 * nv12.powi(4) - 348.0 / 5.0 * nv1 * nv1 * v12s
                + 684.0 / 5.0 * nv1 * nv2 * v12s
                - 66.0 * nv2 * nv2 * v12s
                + 334.0 / 35.0 * v1s * v1s
                - 1336.0 / 35.0 * v1s * v1v2
                + 1308.0 / 35.0 * v1v2 * v1v2
                + 654.0 / 35.0 * v1s * v2s
                - 1252.0 / 35.0 * v1v2 * v2s
                + 292.0 / 35.0 * v2s * v2s);
    let reaction_correction = (n * along_n + v12 * along_v) / C.powi(7);

    PairTerms {
        second,
        reaction,
        reaction_correction,
    }
}

/// Whether a pair's radiation reaction is still converging: its first
/// correction (3.5PN) smaller than its leading term (2.5PN), in the pair's
/// relative acceleration. For a circular orbit the correction is about −9
/// GM/rc² times the leading term, so it overtakes it near GM/rc² ≈ 0.11, a
/// few orbits before a merger. Past that the post-Newtonian series has
/// given out: run on, it can pump the orbit eccentric or fling the pair
/// apart instead of merging it.
pub fn reaction_converges(a: &Body, b: &Body) -> bool {
    let on_a = pair_terms(a.gm, b.gm, a.position, b.position, a.velocity, b.velocity);
    let on_b = pair_terms(b.gm, a.gm, b.position, a.position, b.velocity, a.velocity);
    let leading = on_a.reaction - on_b.reaction;
    let correction = on_a.reaction_correction - on_b.reaction_correction;
    correction.length() < leading.length()
}

/// Where two bodies sit relative to their center of mass beyond Newton,
/// and how fast that offset changes: body 1 is at X₂ x + δ and body 2 at
/// −X₁ x + δ from the center, with δ = ν Δ (𝒫 x + 𝒬 v), where x and v are
/// the relative position and velocity (body 1 minus body 2), X₁ = m₁/m,
/// X₂ = m₂/m and Δ = X₁ − X₂. Blanchet (2024), *Living Rev. Relativ.* 27,
/// 4, section "Equations of motion in the frame of the center of mass":
/// 𝒫 and 𝒬 through 2.5PN (𝒬's 2.5PN part is the radiation reaction's
/// share), as far as Worldline's conservative equations of motion go.
/// Returns (δ, dδ/dt), the rate from the relative acceleration through
/// 2PN. Equal masses have none.
fn center_offset(gm1: f64, gm2: f64, x: DVec3, v: DVec3) -> (DVec3, DVec3) {
    let gm = gm1 + gm2;
    let (x1, x2) = (gm1 / gm, gm2 / gm);
    let (nu, delta) = (x1 * x2, x1 - x2);
    let r = x.length();
    let n = x / r;
    let rdot = n.dot(v);
    let v2 = v.length_squared();
    let a = relative_acceleration(gm, nu, x, v);
    let a = a.newtonian + a.first + a.second;
    let va = v.dot(a);
    let rddot = (v2 - rdot * rdot) / r + n.dot(a);
    let u = gm / r;
    let du = -gm * rdot / (r * r);
    let (c2, c4) = (C * C, C.powi(4));
    // 𝒫 and its rate of change.
    let p1 = 0.5 * v2 - 0.5 * u;
    let dp1 = va - 0.5 * du;
    let inner = -rdot * rdot / 8.0 + 0.75 * nu * rdot * rdot + 19.0 / 8.0 * v2 + 1.5 * nu * v2;
    let dinner = (-0.25 + 1.5 * nu) * rdot * rddot + (19.0 / 4.0 + 3.0 * nu) * va;
    let p2 = (3.0 / 8.0 - 1.5 * nu) * v2 * v2 + u * inner + u * u * (1.75 - 0.5 * nu);
    let dp2 =
        (1.5 - 6.0 * nu) * v2 * va + du * inner + u * dinner + 2.0 * u * du * (1.75 - 0.5 * nu);
    let p = p1 / c2 + p2 / c4;
    let dp = dp1 / c2 + dp2 / c4;
    // 𝒬 = −(7/4) G m ṙ/c⁴ + (4/5) G m (v² − 2Gm/r)/c⁵ and its rate of
    // change.
    let q = -1.75 * gm * rdot / c4 + 0.8 * gm * (v2 - 2.0 * u) / C.powi(5);
    let dq = -1.75 * gm * rddot / c4 + 0.8 * gm * (2.0 * va - 2.0 * du) / C.powi(5);
    let k = nu * delta;
    (k * (p * x + q * v), k * (dp * x + p * v + dq * v + q * a))
}

/// Where two bodies' center of mass is and how fast it moves, to second
/// post-Newtonian order: the inverse of the relation in
/// [`center_offset`]. The plain mass-weighted center differs from it by
/// ν Δ 𝒫 v, which close to a merger is tens of km/s (for GW150914's holes,
/// at GM/rc² = 0.1). Valid while the center itself moves much slower than
/// light.
pub fn center_of_mass(a: &Body, b: &Body) -> (DVec3, DVec3) {
    let gm = a.gm + b.gm;
    let xb = b.gm / gm;
    let (x, v) = (a.position - b.position, a.velocity - b.velocity);
    let (offset, rate) = center_offset(a.gm, b.gm, x, v);
    (a.position - x * xb - offset, a.velocity - v * xb - rate)
}

/// Two bodies set on the circular orbit whose gravitational wave is at
/// `wave_frequency` (Hz, twice the orbit's frequency), in the x–y plane
/// around their center of mass at the origin, `a` on the +x side, turning
/// counterclockwise seen from +z. The speed is the circular speed of the
/// relativistic equations, v² = r |a·n| (a Newtonian circle would be
/// visibly eccentric this close), and the size is found with it: Kepler's
/// law first, then rescaled as r ∝ ω^(−2/3) until the angular speed is the
/// wanted one (relativity slows an orbit of a given size by about
/// (3 − ν)/2 GM/rc²). The pair already falls inward at the leading-order
/// rate, ṙ = −(64/5) G³m³ν/(r³c⁵), so the orbit stays circular as it
/// shrinks, and the bodies are placed about their center of mass to second
/// post-Newtonian order (see [`center_of_mass`]), so it stays put.
pub fn circular_pair(a: Body, b: Body, wave_frequency: f64) -> (Body, Body) {
    let gm = a.gm + b.gm;
    let (xa, xb) = (a.gm / gm, b.gm / gm);
    let nu = xa * xb;
    let omega = std::f64::consts::PI * wave_frequency;
    let mut r = (gm / (omega * omega)).cbrt();
    let mut speed = (gm / r).sqrt();
    for _ in 0..40 {
        let x = DVec3::new(r, 0.0, 0.0);
        let acceleration = relative_acceleration(gm, nu, x, DVec3::new(0.0, speed, 0.0));
        let inward = -(acceleration.newtonian + acceleration.first + acceleration.second).x;
        speed = (r * inward).sqrt();
        r *= (speed / (r * omega)).powf(2.0 / 3.0);
    }
    let x = DVec3::new(r, 0.0, 0.0);
    let rdot = -64.0 / 5.0 * gm.powi(3) * nu / (r.powi(3) * C.powi(5));
    let v = DVec3::new(rdot, speed, 0.0);
    let (offset, rate) = center_offset(a.gm, b.gm, x, v);
    (
        a.at(x * xb + offset).moving(v * xb + rate),
        b.at(-x * xa + offset).moving(-v * xa + rate),
    )
}

/// Relativistic N-body gravity for close pairs, such as two neutron stars:
/// the Einstein–Infeld–Hoffmann equations (first post-Newtonian order) for
/// every body, plus each pair's second-order (2PN) terms and its radiation
/// reaction (2.5PN, with its first correction at 3.5PN), through which a
/// binary loses energy to gravitational waves and spirals in.
///
/// The pairwise terms are exact for two bodies. With more, the 2PN terms
/// that couple three bodies are left out; they matter only when a third
/// body sits close to a compact pair. Adding pairwise terms to N-body
/// gravity is how N-body codes treat compact binaries (Kupi, Amaro-Seoane
/// & Spurzem 2006, *MNRAS* 371, L45; Mikkola & Merritt 2008, *AJ* 135,
/// 2398). For ordinary orbits the terms are below double precision, so the
/// solar system moves as under the first-order equations alone.
///
/// See `docs/physics/post-newtonian.md`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PostNewtonian {
    /// Leave out the radiation reaction, keeping only the conservative
    /// part (for checks that energy is conserved).
    pub conservative: bool,
}

impl PostNewtonian {
    /// The conservative part alone: everything but the radiation reaction.
    pub fn conservative() -> Self {
        Self { conservative: true }
    }
}

impl Gravity for PostNewtonian {
    fn name(&self) -> &'static str {
        if self.conservative {
            "Einstein–Infeld–Hoffmann (1PN) with 2PN between pairs"
        } else {
            "Einstein–Infeld–Hoffmann (1PN) with 2PN and gravitational-wave losses (2.5PN and 3.5PN) between pairs"
        }
    }

    fn velocity_dependent(&self) -> bool {
        true
    }

    fn accelerations(&self, time: f64, bodies: &[Body], out: &mut [DVec3]) {
        EinsteinInfeldHoffmann.accelerations(time, bodies, out);
        for (i, bi) in bodies.iter().enumerate() {
            for (j, bj) in bodies.iter().enumerate().skip(i + 1) {
                if bi.gm == 0.0 && bj.gm == 0.0 {
                    continue;
                }
                let on_i = pair_terms(
                    bi.gm,
                    bj.gm,
                    bi.position,
                    bj.position,
                    bi.velocity,
                    bj.velocity,
                );
                let on_j = pair_terms(
                    bj.gm,
                    bi.gm,
                    bj.position,
                    bi.position,
                    bj.velocity,
                    bi.velocity,
                );
                out[i] += on_i.second;
                out[j] += on_j.second;
                if !self.conservative {
                    out[i] += on_i.reaction + on_i.reaction_correction;
                    out[j] += on_j.reaction + on_j.reaction_correction;
                }
            }
        }
    }
}

/// The relative acceleration of two bodies, a = a₁ − a₂, split by
/// post-Newtonian order, in m/s².
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativeAcceleration {
    /// Newton's: −(Gm/r²) n.
    pub newtonian: DVec3,
    /// First post-Newtonian order (1PN).
    pub first: DVec3,
    /// Second order (2PN).
    pub second: DVec3,
    /// Radiation reaction (2.5PN).
    pub reaction: DVec3,
    /// Its first correction (3.5PN).
    pub reaction_correction: DVec3,
}

impl RelativeAcceleration {
    /// All of them together.
    pub fn total(&self) -> DVec3 {
        self.newtonian + self.first + self.second + self.reaction + self.reaction_correction
    }
}

/// The relative acceleration of two bodies in their center-of-mass frame,
/// harmonic coordinates, through 3.5PN (leaving out 3PN), from their total gravitational
/// parameter `gm` (m³/s²), symmetric mass ratio `nu` = m₁m₂/(m₁+m₂)², and
/// separation `x` = y₁ − y₂ and relative velocity `v`:
///
/// a = −(Gm/r²) [(1 + 𝒜) n + ℬ v]
///
/// with the coefficients 𝒜 and ℬ of Blanchet (2024), section "Equations of
/// motion in the frame of the center of mass" (its 1PN, 2PN, 2.5PN and 3.5PN parts; the
/// 2PN ones also in Kidder 1995, *Phys. Rev. D* 52, 821). An independent
/// form of the same physics as [`pair_terms`], for checks.
pub fn relative_acceleration(gm: f64, nu: f64, x: DVec3, v: DVec3) -> RelativeAcceleration {
    let r = x.length();
    let n = x / r;
    let rdot = n.dot(v);
    let v2 = v.length_squared();
    let u = gm / r; // Gm/r
    let scale = -gm / (r * r);
    let (c2, c4, c5) = (C * C, C.powi(4), C.powi(5));

    let a1 = -1.5 * rdot * rdot * nu + v2 + 3.0 * nu * v2 - u * (4.0 + 2.0 * nu);
    let b1 = -4.0 * rdot + 2.0 * rdot * nu;
    let (rd2, rd3, rd4) = (rdot * rdot, rdot.powi(3), rdot.powi(4));
    let a2 = 15.0 / 8.0 * rd4 * nu - 45.0 / 8.0 * rd4 * nu * nu - 4.5 * rd2 * nu * v2
        + 6.0 * rd2 * nu * nu * v2
        + 3.0 * nu * v2 * v2
        - 4.0 * nu * nu * v2 * v2
        + u * (-2.0 * rd2 - 25.0 * rd2 * nu - 2.0 * rd2 * nu * nu - 6.5 * nu * v2
            + 2.0 * nu * nu * v2)
        + u * u * (9.0 + 87.0 / 4.0 * nu);
    let b2 =
        4.5 * rd3 * nu + 3.0 * rd3 * nu * nu - 7.5 * rdot * nu * v2 - 2.0 * rdot * nu * nu * v2
            + u * (2.0 * rdot + 20.5 * rdot * nu + 4.0 * rdot * nu * nu);
    let a25 = 8.0 * u * nu / 5.0 * rdot * (-17.0 / 3.0 * u - 3.0 * v2);
    let b25 = 8.0 * u * nu / 5.0 * (3.0 * u + v2);
    let a35 = u
        * nu
        * rdot
        * (u * u * (3956.0 / 35.0 + 184.0 / 5.0 * nu)
            + u * v2 * (692.0 / 35.0 - 724.0 / 15.0 * nu)
            + v2 * v2 * (366.0 / 35.0 + 12.0 * nu)
            + u * rd2 * (294.0 / 5.0 + 376.0 / 5.0 * nu)
            - v2 * rd2 * (114.0 + 12.0 * nu)
            + 112.0 * rd4);
    let b35 = u
        * nu
        * (u * u * (-1060.0 / 21.0 - 104.0 / 5.0 * nu)
            + u * v2 * (164.0 / 21.0 + 148.0 / 5.0 * nu)
            + v2 * v2 * (-626.0 / 35.0 - 12.0 / 5.0 * nu)
            + u * rd2 * (-82.0 / 3.0 - 848.0 / 15.0 * nu)
            + v2 * rd2 * (678.0 / 5.0 + 12.0 / 5.0 * nu)
            - 120.0 * rd4);

    RelativeAcceleration {
        newtonian: scale * n,
        first: scale * (a1 * n + b1 * v) / c2,
        second: scale * (a2 * n + b2 * v) / c4,
        reaction: scale * (a25 * n + b25 * v) / c5,
        reaction_correction: scale * (a35 * n + b35 * v) / C.powi(7),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::System;
    use crate::constants::GM_SUN;
    use crate::gravitational_waves::{quadrupole_power, quadrupole_power_correction};
    use crate::integrator::{Ias15, advance};
    use crate::orbit::{kepler_period, periapsis_state};

    /// Two bodies in their center-of-mass frame, with separation `x` and
    /// relative velocity `v`: the positions y₁ = X₂ x, y₂ = −X₁ x (where
    /// they sit doesn't change any force), and velocities from
    /// differentiating Blanchet (2024)'s CM relation, y₁ = [X₂ + νΔ𝒫] x +
    /// νΔ𝒬 v and likewise y₂, with its 1PN 𝒫 = (v²/2 − Gm/2r)/c² and its
    /// 2.5PN 𝒬 = (4Gm v²/5 − 8G²m²/5r)/c⁵ (whose time derivative vanishes at
    /// this order, leaving 𝒬 a).
    fn center_of_mass_pair(gm1: f64, gm2: f64, x: DVec3, v: DVec3) -> [Body; 2] {
        let gm = gm1 + gm2;
        let (x1, x2) = (gm1 / gm, gm2 / gm);
        let (nu, delta) = (x1 * x2, x1 - x2);
        let r = x.length();
        let n = x / r;
        let rdot = n.dot(v);
        let v2 = v.length_squared();
        let first = ((0.5 * v2 - 0.5 * gm / r) * v - 0.5 * gm * rdot / r * n) / (C * C);
        let q = (0.8 * gm * v2 - 1.6 * gm * gm / r) / C.powi(5);
        let reaction = -q * gm / (r * r) * n;
        let shift = nu * delta * (first + reaction);
        [
            Body::new("one", gm1, 1.0).at(x * x2).moving(v * x2 + shift),
            Body::new("two", gm2, 1.0)
                .at(-x * x1)
                .moving(-v * x1 + shift),
        ]
    }

    /// A pair whose field strength GM/(r c²) is `strength` at a separation
    /// with radial motion, so that every term counts.
    fn strong_pair(gm1: f64, gm2: f64, strength: f64) -> (DVec3, DVec3) {
        let gm = gm1 + gm2;
        let r = gm / (strength * C * C);
        let x = DVec3::new(0.8, 0.5, 0.2).normalize() * r;
        let speed = (gm / r).sqrt();
        let v = DVec3::new(-0.3, 0.9, 0.1).normalize() * (1.1 * speed);
        (x, v)
    }

    #[test]
    fn the_center_of_mass_stays_put_to_third_order() {
        // Two holes of 30 and 10 Suns on circular orbits, set up about
        // their center of mass to second order, run for 5 orbits at two
        // field strengths x = GM/rc². The plain mass-weighted center swings
        // at second order, ν Δ x² v (it leaves out ν Δ 𝒫 v, and 𝒫 ~ x² on a
        // circle), so its swing grows as x^2.5 (v grows as √x). The
        // second-order center may only move at third order, as x^3.5; a
        // mistake in its 2PN terms would leave x^2.5. The test takes the
        // midpoint: the swing must grow faster than x³.
        let (gm1, gm2) = (30.0 * GM_SUN, 10.0 * GM_SUN);
        let gm = gm1 + gm2;
        let largest = |x: f64| {
            let r = gm / (x * C * C);
            let wave = (gm / r.powi(3)).sqrt() / std::f64::consts::PI;
            let (a, b) = circular_pair(Body::new("a", gm1, 1.0), Body::new("b", gm2, 1.0), wave);
            let mut system = System::new(vec![a, b]);
            let mut ias = Ias15::new();
            let (mut plain, mut second): (f64, f64) = (0.0, 0.0);
            for _ in 0..5 * 64 {
                advance(
                    &mut system,
                    &PostNewtonian::default(),
                    &mut ias,
                    1.0 / (wave * 32.0),
                );
                let (p, q) = (&system.bodies[0], &system.bodies[1]);
                plain = plain.max(((p.velocity * p.gm + q.velocity * q.gm) / gm).length());
                second = second.max(center_of_mass(p, q).1.length());
            }
            (plain, second)
        };
        let (weak, strong) = (0.003, 0.01);
        let ((plain_weak, second_weak), (plain_strong, second_strong)) =
            (largest(weak), largest(strong));
        let power = |w: f64, s: f64| (s / w).ln() / (strong / weak).ln();
        println!(
            "center of mass swings: mass-weighted {plain_weak:.2} → {plain_strong:.1} m/s (as x^{:.2}); second-order {second_weak:.3} → {second_strong:.2} m/s (as x^{:.2})",
            power(plain_weak, plain_strong),
            power(second_weak, second_strong)
        );
        assert!(power(second_weak, second_strong) > 3.0);
    }

    #[test]
    fn the_reaction_stops_converging_a_few_orbits_before_a_merger() {
        // Two neutron stars on circular orbits: the 3.5PN correction is
        // about −9 GM/rc² times the leading reaction, overtaking it near
        // GM/rc² = 0.11.
        let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
        let gm = gm1 + gm2;
        let at = |strength: f64| {
            let r = gm / (strength * C * C);
            let v = DVec3::new(0.0, (gm / r).sqrt(), 0.0);
            center_of_mass_pair(gm1, gm2, DVec3::new(r, 0.0, 0.0), v)
        };
        for (strength, converges) in [(0.01, true), (0.08, true), (0.14, false)] {
            let [a, b] = at(strength);
            assert_eq!(reaction_converges(&a, &b), converges, "{strength}");
        }
    }

    #[test]
    fn two_bodies_follow_the_center_of_mass_equations() {
        // The general-frame equations (EIH plus the pair terms), applied to
        // two bodies in their center-of-mass frame, must give the
        // center-of-mass equations' relative acceleration. What differs is
        // of third order (GM/rc²)³: from the center of mass's own 2PN
        // shift, which neither side keeps. A mistake in any 2PN term would
        // show at second order instead, (GM/rc²)². The test passes below the
        // geometric midpoint, (GM/rc²)^(5/2).
        //
        // The radiation reaction is compared on its own: it is the part of
        // the acceleration that flips sign when every velocity flips, while
        // the conservative parts stay. What differs is then GM/rc² of the
        // 3.5PN part; a mistake in it would make it differ by about itself.
        // The test passes below √(GM/rc²) of it.
        let strength: f64 = 1e-3;
        for (gm1, gm2) in [
            (3.0 * GM_SUN, GM_SUN),
            (1.4 * GM_SUN, 1.4 * GM_SUN),
            (10.0 * GM_SUN, 1e-6 * GM_SUN),
        ] {
            let gm = gm1 + gm2;
            let nu = gm1 * gm2 / (gm * gm);
            let (x, v) = strong_pair(gm1, gm2, strength);
            let bodies = center_of_mass_pair(gm1, gm2, x, v);
            let mut out = [DVec3::ZERO; 2];
            PostNewtonian::default().accelerations(0.0, &bodies, &mut out);
            let expected = relative_acceleration(gm, nu, x, v);
            let newton = expected.newtonian.length();
            let mismatch = (out[0] - out[1] - expected.total()).length() / newton;

            let reversed = center_of_mass_pair(gm1, gm2, x, -v);
            let mut back = [DVec3::ZERO; 2];
            PostNewtonian::default().accelerations(0.0, &reversed, &mut back);
            let odd = 0.5 * ((out[0] - out[1]) - (back[0] - back[1]));
            let expected_odd = expected.reaction + expected.reaction_correction;
            let correction = expected.reaction_correction.length();
            let reaction_mismatch = (odd - expected_odd).length() / correction;
            println!(
                "m₂/m₁ = {:.0e}: mismatch {mismatch:.1e} of Newton's pull (2PN part {:.1e}, bound {:.1e}); radiation reaction (2.5PN {:.1e}, 3.5PN {:.1e} of Newton's pull) mismatch {reaction_mismatch:.1e} of the 3.5PN part (bound {:.1e})",
                gm2 / gm1,
                expected.second.length() / newton,
                strength.powf(2.5),
                expected.reaction.length() / newton,
                correction / newton,
                strength.sqrt()
            );
            assert!(mismatch < strength.powf(2.5));
            // Measurable only well above double-precision rounding: for a
            // near test particle the 3.5PN part is 10⁻¹⁶ of the pull.
            if correction / newton > 1e4 * f64::EPSILON {
                assert!(reaction_mismatch < strength.sqrt());
            }
        }
    }

    /// The pair's energy times G, in harmonic coordinates, to 2PN order,
    /// and with `schott`, the 2.5PN (Schott) term that makes the energy
    /// balance hold at every instant: Blanchet (2024), section "The 3.5PN
    /// acceleration and 3PN energy", body 1's terms plus the same with the
    /// labels swapped.
    fn energy(bodies: &[Body], schott: bool) -> f64 {
        let half = |b1: &Body, b2: &Body| {
            let (gm1, gm2) = (b1.gm, b2.gm);
            let d = b1.position - b2.position;
            let r = d.length();
            let n = d / r;
            let (v1, v2) = (b1.velocity, b2.velocity);
            let v12 = v1 - v2;
            let (nv1, nv2) = (n.dot(v1), n.dot(v2));
            let (v1s, v2s, v1v2) = (v1.length_squared(), v2.length_squared(), v1.dot(v2));
            let c2 = C * C;
            let newton = 0.5 * gm1 * v1s - 0.5 * gm1 * gm2 / r;
            let first = 0.5 * gm1 * gm1 * gm2 / (r * r)
                + 3.0 / 8.0 * gm1 * v1s * v1s
                + gm1 * gm2 / r * (-0.25 * nv1 * nv2 + 1.5 * v1s - 1.75 * v1v2);
            let second = -0.5 * gm1.powi(3) * gm2 / r.powi(3)
                - 19.0 / 8.0 * gm1 * gm1 * gm2 * gm2 / r.powi(3)
                + 5.0 / 16.0 * gm1 * v1s.powi(3)
                + gm1 * gm2 / r
                    * (3.0 / 8.0 * nv1.powi(3) * nv2 + 3.0 / 16.0 * nv1 * nv1 * nv2 * nv2
                        - 9.0 / 8.0 * nv1 * nv2 * v1s
                        - 13.0 / 8.0 * nv2 * nv2 * v1s
                        + 21.0 / 8.0 * v1s * v1s
                        + 13.0 / 8.0 * nv1 * nv1 * v1v2
                        + 0.75 * nv1 * nv2 * v1v2
                        - 55.0 / 8.0 * v1s * v1v2
                        + 17.0 / 8.0 * v1v2 * v1v2
                        + 31.0 / 16.0 * v1s * v2s)
                + gm1 * gm1 * gm2 / (r * r)
                    * (29.0 / 4.0 * nv1 * nv1 - 13.0 / 4.0 * nv1 * nv2 + 0.5 * nv2 * nv2
                        - 1.5 * v1s
                        + 1.75 * v2s);
            let schott_term = if schott {
                4.0 * gm1 * gm1 * gm2 / (5.0 * r * r)
                    * nv1
                    * (v12.length_squared() - 2.0 * (gm1 - gm2) / r)
            } else {
                0.0
            };
            newton + first / c2 + second / (c2 * c2) + schott_term / C.powi(5)
        };
        half(&bodies[0], &bodies[1]) + half(&bodies[1], &bodies[0])
    }

    /// A binary at periastron with eccentricity 0.5, whose field strength
    /// GM/(rc²) at periastron is `strength`, in its center-of-mass frame.
    fn eccentric_binary(strength: f64) -> (System, f64) {
        let (gm1, gm2) = (1.4 * GM_SUN, 1.0 * GM_SUN);
        let gm = gm1 + gm2;
        let e = 0.5;
        let a = gm / (strength * C * C) / (1.0 - e);
        let (x, v) = periapsis_state(a, e, gm);
        let bodies = center_of_mass_pair(gm1, gm2, x, v);
        (System::new(bodies.to_vec()), kepler_period(a, gm))
    }

    #[test]
    fn without_radiation_the_pair_keeps_its_energy() {
        // The 2PN energy wanders under the first-order equations alone:
        // they leave out a second-order part, (GM/rc²)² of the force. With
        // the 2PN terms, only third-order parts are left out, so it wanders
        // GM/rc² times less; a mistake in a 2PN term would leave it
        // wandering about as much. The test passes below the midpoint,
        // √(GM/rc²) times the first-order wander (the orbit's own
        // coefficient cancels in the comparison).
        let strength: f64 = 1e-3;
        let wander = |gravity: &dyn Gravity| {
            let (mut system, period) = eccentric_binary(strength);
            let start = energy(&system.bodies, false);
            let mut ias = Ias15::new();
            let mut worst: f64 = 0.0;
            for _ in 0..(10 * 64) {
                advance(&mut system, gravity, &mut ias, period / 64.0);
                worst = worst.max(((energy(&system.bodies, false) - start) / start).abs());
            }
            worst
        };
        let ours = wander(&PostNewtonian::conservative());
        let first_order = wander(&EinsteinInfeldHoffmann);
        let bound = strength.sqrt() * first_order;
        println!(
            "over 10 orbits the energy wanders by {first_order:.1e} of itself with first order alone, {ours:.1e} with 2PN (bound {bound:.1e})"
        );
        assert!(ours < bound);
    }

    #[test]
    fn radiation_carries_off_the_waves_power() {
        // With the radiation reaction, the energy (with its Schott term)
        // falls as fast as the waves carry it off. Against Einstein's
        // quadrupole formula alone it misses by the formula's own next
        // order, GM/rc² of it. Against the formula with its first
        // correction (Wagoner & Will 1976), it must miss GM/rc² times less,
        // if the 3.5PN reaction terms are right; a mistake in them would
        // leave it missing about as much. The test passes below the
        // midpoint, √(GM/rc²) times the first miss (the orbit's own
        // coefficients cancel in the comparison).
        //
        // The energy has further Schott terms, at 3.5PN, that swing back and
        // forth around the orbit. Stopping at a periastron, where the run
        // started, lets them cancel; stopping elsewhere leaves a swing that
        // doesn't shrink with GM/rc².
        for strength in [1e-2f64, 3e-3] {
            let (mut system, period) = eccentric_binary(strength);
            let gravity = PostNewtonian::default();
            let relative = |s: &System| {
                let (b1, b2) = (&s.bodies[0], &s.bodies[1]);
                (b1.position - b2.position, b1.velocity - b2.velocity)
            };
            let power = |s: &System| {
                let (x, v) = relative(s);
                let (r, rdot) = (x.length(), x.dot(v) / x.length());
                let (gm1, gm2) = (s.bodies[0].gm, s.bodies[1].gm);
                DVec3::new(
                    quadrupole_power(gm1, gm2, r, v.length(), rdot),
                    quadrupole_power_correction(gm1, gm2, r, v.length(), rdot),
                    0.0,
                )
            };
            let start = energy(&system.bodies, true);
            let mut ias = Ias15::new();
            let dt = period / 256.0;
            // Simpson's rule over the samples; the energy is E·G and the
            // power is in W, so radiated·G is compared.
            let mut radiated = DVec3::ZERO;
            let mut previous = power(&system);
            let mut passages = 0;
            while passages < 20 {
                let (x, v) = relative(&system);
                let approaching = x.dot(v) < 0.0;
                advance(&mut system, &gravity, &mut ias, dt / 2.0);
                let middle = power(&system);
                advance(&mut system, &gravity, &mut ias, dt / 2.0);
                let after = power(&system);
                radiated += dt / 6.0 * (previous + 4.0 * middle + after);
                previous = after;
                let (x, v) = relative(&system);
                if approaching && x.dot(v) >= 0.0 {
                    passages += 1;
                }
            }
            let radiated = radiated * crate::constants::G;
            let lost = start - energy(&system.bodies, true);
            let leading = radiated.x;
            let corrected = radiated.x + radiated.y;
            let miss = (lost / leading - 1.0).abs();
            let corrected_miss = (lost / corrected - 1.0).abs();
            println!(
                "GM/rc² = {strength:.0e}, over 20 orbits: lost {:.4e} of the energy; quadrupole formula {:.4e} (off by {miss:.1e}), with its correction {:.4e} (off by {corrected_miss:.1e}, bound {:.1e})",
                lost / start.abs(),
                leading / start.abs(),
                corrected / start.abs(),
                strength.sqrt() * miss
            );
            assert!(corrected_miss < strength.sqrt() * miss);
        }
    }
}
