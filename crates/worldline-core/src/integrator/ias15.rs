use glam::DVec3;

use super::Integrator;
use crate::System;
use crate::gravity::Gravity;

/// Gauss–Radau spacings on [0, 1]: the 8 points within a step where IAS15
/// samples the acceleration. Point 0 is the start of the step. The other 7
/// are the roots of P₇(x) + P₈(x), where Pₙ are Legendre polynomials, mapped
/// from [−1, 1] to [0, 1]. A unit test checks each one.
const H: [f64; 8] = [
    0.0,
    0.056_262_560_536_922_15,
    0.180_240_691_736_892_36,
    0.352_624_717_113_169_6,
    0.547_153_626_330_555_4,
    0.734_210_177_215_410_5,
    0.885_320_946_839_095_8,
    0.977_520_613_561_287_5,
];

/// `NEWTON_TO_POWER[m][k]` is the coefficient of sᵏ in (s − h₁)(s − h₂)…(s − hₘ).
/// It converts the Newton-form coefficients `g` into power-series coefficients `b`.
const NEWTON_TO_POWER: [[f64; 7]; 7] = newton_to_power();

/// `BINOMIAL[m][k]` is the binomial coefficient C(m + 1, k + 1), used to
/// shift the acceleration polynomial forward to the next step.
const BINOMIAL: [[f64; 7]; 7] = binomial();

/// A proposed step is rejected if the error estimate says it should have
/// been 4× shorter, and steps never grow more than 4× at once.
const SAFETY: f64 = 0.25;

/// The predictor–corrector loop stops when corrections fall below this,
/// relative to the acceleration.
const CONVERGED: f64 = 1e-16;

/// Upper limit on predictor–corrector iterations per step.
const MAX_ITERATIONS: usize = 12;

/// How IAS15 chooses its next step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepCriterion {
    /// From the shortest dynamical timescale among the bodies, built from
    /// acceleration, jerk and snap (Pham, Rein & Spiegel 2024). Robust to
    /// rounding noise in close encounters. The default, as in REBOUND.
    Timescale,
    /// From the size of the acceleration polynomial's highest term relative
    /// to the acceleration (Rein & Spiegel 2015). Use it when accelerations
    /// pass through zero, as in a harmonic oscillator, where the timescale
    /// criterion asks for ever shorter steps; gravity never does.
    HighestTerm,
}

/// IAS15: an adaptive 15th-order integrator accurate to machine precision.
///
/// Within each step, every body's acceleration is modeled as a polynomial in
/// time of degree 7, fitted at the 8 Gauss–Radau points by iterating until the
/// fit stops improving. Integrating that polynomial twice gives positions and
/// velocities with 15th-order accuracy. The size of the highest-order term
/// estimates the error, which sets the next step size so the error stays
/// below double precision.
///
/// It handles accelerations that depend on velocity, so it works with
/// relativistic gravity, and adapts automatically to close encounters and
/// eccentric orbits. Source: Rein & Spiegel, MNRAS 446, 1424 (2015).
/// See `docs/physics/integrators.md`.
#[derive(Debug, Clone)]
pub struct Ias15 {
    /// Precision parameter ε. The default, 10⁻⁹, keeps the per-step error
    /// below double-precision rounding.
    pub epsilon: f64,
    /// How the next step is chosen.
    pub criterion: StepCriterion,
    /// The next step to try, in s. Zero until the first step picks one.
    dt: f64,
    /// The last accepted step, in s. Zero before the first one.
    dt_last_done: f64,
    /// Position and velocity at the start of the step.
    x0: Vec<DVec3>,
    v0: Vec<DVec3>,
    /// Compensated-summation error terms for positions and velocities.
    csx: Vec<DVec3>,
    csv: Vec<DVec3>,
    /// Acceleration at the start of the step, and at the latest substep.
    a0: Vec<DVec3>,
    at: Vec<DVec3>,
    /// Acceleration polynomial: a(s) = a₀ + b₀s + b₁s² + … + b₆s⁷, for s in [0, 1].
    b: Vec<[DVec3; 7]>,
    /// The same polynomial in Newton form on the Gauss–Radau points.
    g: Vec<[DVec3; 7]>,
    /// The values `b` was predicted to have before this step was iterated.
    e: Vec<[DVec3; 7]>,
    /// `b` and `e` from the last accepted step, kept to re-predict after a rejection.
    br: Vec<[DVec3; 7]>,
    er: Vec<[DVec3; 7]>,
}

impl Default for Ias15 {
    fn default() -> Self {
        Self {
            epsilon: 1e-9,
            criterion: StepCriterion::Timescale,
            dt: 0.0,
            dt_last_done: 0.0,
            x0: Vec::new(),
            v0: Vec::new(),
            csx: Vec::new(),
            csv: Vec::new(),
            a0: Vec::new(),
            at: Vec::new(),
            b: Vec::new(),
            g: Vec::new(),
            e: Vec::new(),
            br: Vec::new(),
            er: Vec::new(),
        }
    }
}

impl Ias15 {
    /// Creates an IAS15 integrator with the default precision. It picks its
    /// own step sizes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sizes the working arrays for `n` bodies, starting fresh if the number
    /// of bodies changed.
    fn prepare(&mut self, n: usize) {
        if self.x0.len() == n {
            return;
        }
        *self = Self {
            epsilon: self.epsilon,
            criterion: self.criterion,
            ..Self::default()
        };
        let zeros = vec![DVec3::ZERO; n];
        let polys = vec![[DVec3::ZERO; 7]; n];
        for v in [
            &mut self.x0,
            &mut self.v0,
            &mut self.csx,
            &mut self.csv,
            &mut self.a0,
            &mut self.at,
        ] {
            v.clone_from(&zeros);
        }
        for p in [
            &mut self.b,
            &mut self.g,
            &mut self.e,
            &mut self.br,
            &mut self.er,
        ] {
            p.clone_from(&polys);
        }
    }

    /// Sets every body to its predicted state at fraction `s` of a step of `dt` seconds.
    fn predict(&self, system: &mut System, s: f64, dt: f64) {
        let sdt = s * dt;
        for (i, body) in system.bodies.iter_mut().enumerate() {
            let (a0, b) = (self.a0[i], &self.b[i]);
            let dx = a0 / 2.0
                + s * (b[0] / 6.0
                    + s * (b[1] / 12.0
                        + s * (b[2] / 20.0
                            + s * (b[3] / 30.0
                                + s * (b[4] / 42.0 + s * (b[5] / 56.0 + s * b[6] / 72.0))))));
            let dv = a0
                + s * (b[0] / 2.0
                    + s * (b[1] / 3.0
                        + s * (b[2] / 4.0
                            + s * (b[3] / 5.0
                                + s * (b[4] / 6.0 + s * (b[5] / 7.0 + s * b[6] / 8.0))))));
            body.position = self.x0[i] + sdt * self.v0[i] + sdt * sdt * dx;
            body.velocity = self.v0[i] + sdt * dv;
        }
    }

    /// Iterates the acceleration polynomial for a step of `dt` seconds until
    /// it stops improving.
    fn converge(&mut self, system: &mut System, gravity: &dyn Gravity, dt: f64) {
        let mut error = f64::INFINITY;
        let mut previous_error = f64::INFINITY;
        for iteration in 0..MAX_ITERATIONS {
            if error < CONVERGED || (iteration > 2 && previous_error <= error) {
                break;
            }
            previous_error = error;
            let (mut max_change, mut max_acc): (f64, f64) = (0.0, 0.0);
            for n in 1..8 {
                let s = H[n];
                self.predict(system, s, dt);
                gravity.accelerations(system.time() + s * dt, &system.bodies, &mut self.at);
                for i in 0..self.at.len() {
                    let (g, b) = (&mut self.g[i], &mut self.b[i]);
                    // Divided differences give the newest Newton coefficient.
                    let mut newest = (self.at[i] - self.a0[i]) / s;
                    for j in 1..n {
                        newest = (newest - g[j - 1]) / (s - H[j]);
                    }
                    let change = newest - g[n - 1];
                    g[n - 1] = newest;
                    for (k, bk) in b.iter_mut().enumerate().take(n) {
                        *bk += change * NEWTON_TO_POWER[n - 1][k];
                    }
                    if n == 7 {
                        max_change = max_change.max(change.abs().max_element());
                        max_acc = max_acc.max(self.at[i].abs().max_element());
                    }
                }
            }
            error = if max_acc > 0.0 {
                max_change / max_acc
            } else {
                0.0
            };
        }
    }

    /// Relative size of the highest-order term: the original error estimate.
    fn highest_term(&self) -> f64 {
        let (mut max_b6, mut max_acc): (f64, f64) = (0.0, 0.0);
        for (b, a) in self.b.iter().zip(&self.at) {
            max_b6 = max_b6.max(b[6].abs().max_element());
            max_acc = max_acc.max(a.abs().max_element());
        }
        max_b6 / max_acc
    }

    /// The shortest dynamical timescale among the bodies at the end of the
    /// step, as a fraction of the step: τ = √(2a² / (j² + a·s)) from each
    /// body's acceleration a, jerk j and snap s (Pham, Rein & Spiegel 2024,
    /// eq. 16). `None` if no body accelerates.
    ///
    /// It replaced IAS15's original estimate as the default: the size of
    /// the polynomial's highest term can be dominated by rounding noise in a
    /// close encounter, and the integrator then keeps shrinking its steps to
    /// chase the noise (it stalled on Apophis passing Earth in 2029).
    fn shortest_timescale(&self) -> Option<f64> {
        let mut shortest: Option<f64> = None;
        for (b, a0) in self.b.iter().zip(&self.a0) {
            // The acceleration polynomial a(s) = a₀ + Σ bₖ s^(k+1) and its
            // first two derivatives at the end of the step, s = 1 (jerk and
            // snap in units of the step).
            let acceleration = *a0 + b.iter().sum::<DVec3>();
            let jerk: DVec3 = b
                .iter()
                .enumerate()
                .map(|(k, bk)| *bk * (k + 1) as f64)
                .sum();
            let snap: DVec3 = b
                .iter()
                .enumerate()
                .map(|(k, bk)| *bk * ((k + 1) * k) as f64)
                .sum();
            let (y2, y3, y4) = (
                acceleration.length_squared(),
                jerk.length_squared(),
                snap.length_squared(),
            );
            if !y2.is_normal() {
                continue;
            }
            let timescale2 = 2.0 * y2 / (y3 + (y4 * y2).sqrt());
            if timescale2.is_normal() {
                shortest = Some(shortest.map_or(timescale2, |s: f64| s.min(timescale2)));
            }
        }
        shortest.map(f64::sqrt)
    }

    /// Predicts the next step's polynomial from the last accepted step's,
    /// for a next step `ratio` times as long. The error of the previous
    /// prediction is carried forward as a correction.
    fn predict_next_polynomial(&mut self, ratio: f64) {
        for i in 0..self.b.len() {
            if self.dt_last_done == 0.0 || ratio > 20.0 {
                // No history, or too big a jump to extrapolate: start from zero.
                self.e[i] = [DVec3::ZERO; 7];
                self.b[i] = [DVec3::ZERO; 7];
                continue;
            }
            let (br, er) = (&self.br[i], &self.er[i]);
            let mut q = ratio;
            for k in 0..7 {
                let shifted: DVec3 = (k..7).map(|m| BINOMIAL[m][k] * br[m]).sum();
                self.e[i][k] = q * shifted;
                self.b[i][k] = self.e[i][k] + (br[k] - er[k]);
                q *= ratio;
            }
        }
    }
}

impl Integrator for Ias15 {
    fn name(&self) -> &'static str {
        "IAS15"
    }

    fn step(&mut self, system: &mut System, gravity: &dyn Gravity, max_dt: f64) -> f64 {
        self.prepare(system.bodies.len());
        if self.dt == 0.0 {
            self.dt = initial_step(system);
        }
        let natural_dt = self.dt;

        for (i, body) in system.bodies.iter().enumerate() {
            self.x0[i] = body.position;
            self.v0[i] = body.velocity;
        }
        gravity.accelerations(system.time(), &system.bodies, &mut self.a0);

        loop {
            let dt = self.dt.min(max_dt);
            for (g, b) in self.g.iter_mut().zip(&self.b) {
                *g = newton_from_power(b);
            }
            self.converge(system, gravity, dt);

            let dt_new = match self.criterion {
                // Equation 17: (5040 ε)^(1/7) τ, which gives the same steps
                // as the original criterion on circular orbits.
                StepCriterion::Timescale => match self.shortest_timescale() {
                    Some(timescale) => (5040.0 * self.epsilon).powf(1.0 / 7.0) * timescale * dt,
                    None => dt / SAFETY,
                },
                StepCriterion::HighestTerm => {
                    let error = self.highest_term();
                    if error.is_normal() {
                        dt * (self.epsilon / error).powf(1.0 / 7.0)
                    } else {
                        dt / SAFETY
                    }
                }
            };

            if dt_new < SAFETY * dt {
                // The step was far too long. Undo it and retry a shorter one.
                for (i, body) in system.bodies.iter_mut().enumerate() {
                    body.position = self.x0[i];
                    body.velocity = self.v0[i];
                }
                self.dt = dt_new;
                self.predict_next_polynomial(dt_new / self.dt_last_done);
                continue;
            }

            // Accept: advance to the end of the step with compensated summation.
            for (i, body) in system.bodies.iter_mut().enumerate() {
                let (a0, b) = (self.a0[i], &self.b[i]);
                let dx = dt * self.v0[i]
                    + dt * dt
                        * (a0 / 2.0
                            + b[0] / 6.0
                            + b[1] / 12.0
                            + b[2] / 20.0
                            + b[3] / 30.0
                            + b[4] / 42.0
                            + b[5] / 56.0
                            + b[6] / 72.0);
                let dv = dt
                    * (a0
                        + b[0] / 2.0
                        + b[1] / 3.0
                        + b[2] / 4.0
                        + b[3] / 5.0
                        + b[4] / 6.0
                        + b[5] / 7.0
                        + b[6] / 8.0);
                compensated_add(&mut self.x0[i], &mut self.csx[i], dx);
                compensated_add(&mut self.v0[i], &mut self.csv[i], dv);
                body.position = self.x0[i];
                body.velocity = self.v0[i];
            }
            system.tick(dt);

            // A step cut short to land on a requested time says nothing
            // about the natural step size, so don't let it shrink the next one.
            self.dt = if dt < natural_dt {
                natural_dt.min(dt_new)
            } else {
                dt_new.min(dt / SAFETY)
            };
            self.dt_last_done = dt;
            self.er.copy_from_slice(&self.e);
            self.br.copy_from_slice(&self.b);
            self.predict_next_polynomial(self.dt / dt);
            return dt;
        }
    }
}

/// Converts power-series coefficients `b` to Newton-form coefficients `g`
/// by back substitution through [`NEWTON_TO_POWER`].
fn newton_from_power(b: &[DVec3; 7]) -> [DVec3; 7] {
    let mut g = [DVec3::ZERO; 7];
    for k in (0..7).rev() {
        g[k] = b[k];
        for m in k + 1..7 {
            g[k] -= NEWTON_TO_POWER[m][k] * g[m];
        }
    }
    g
}

/// Kahan compensated summation: adds `value` to `sum` while carrying the
/// rounding error in `compensation`, so long runs don't accumulate it.
fn compensated_add(sum: &mut DVec3, compensation: &mut DVec3, value: DVec3) {
    let y = value - *compensation;
    let t = *sum + y;
    *compensation = (t - *sum) - y;
    *sum = t;
}

/// A safe first step: 1% of the shortest timescale between any two bodies,
/// either the orbital timescale √(r³/μ) or the flyby timescale r/v.
fn initial_step(system: &System) -> f64 {
    let bodies = &system.bodies;
    let mut shortest = f64::INFINITY;
    for (i, bi) in bodies.iter().enumerate() {
        for bj in &bodies[i + 1..] {
            let r = (bj.position - bi.position).length();
            let mu = bi.gm + bj.gm;
            if mu > 0.0 {
                shortest = shortest.min((r.powi(3) / mu).sqrt());
            }
            let v = (bj.velocity - bi.velocity).length();
            if v > 0.0 {
                shortest = shortest.min(r / v);
            }
        }
    }
    if shortest.is_finite() && shortest > 0.0 {
        0.01 * shortest
    } else {
        1.0
    }
}

const fn newton_to_power() -> [[f64; 7]; 7] {
    let mut c = [[0.0; 7]; 7];
    c[0][0] = 1.0;
    let mut m = 1;
    while m < 7 {
        // Multiply the previous product by (s − hₘ).
        let mut k = 0;
        while k <= m {
            let shifted = if k > 0 { c[m - 1][k - 1] } else { 0.0 };
            c[m][k] = shifted - H[m] * c[m - 1][k];
            k += 1;
        }
        m += 1;
    }
    c
}

const fn binomial() -> [[f64; 7]; 7] {
    // Pascal's triangle up to row 7, then pick C(m + 1, k + 1).
    let mut pascal = [[0.0; 8]; 8];
    let mut n = 0;
    while n < 8 {
        pascal[n][0] = 1.0;
        let mut k = 1;
        while k <= n {
            pascal[n][k] = pascal[n - 1][k - 1] + pascal[n - 1][k];
            k += 1;
        }
        n += 1;
    }
    let mut out = [[0.0; 7]; 7];
    let mut m = 0;
    while m < 7 {
        let mut k = 0;
        while k <= m {
            out[m][k] = pascal[m + 1][k + 1];
            k += 1;
        }
        m += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Body;
    use crate::integrator::advance;
    use std::f64::consts::TAU;

    /// Legendre polynomials Pₙ(x) and Pₙ₋₁(x), by the standard recurrence.
    fn legendre(n: usize, x: f64) -> (f64, f64) {
        let (mut previous, mut current) = (1.0, x);
        for k in 1..n {
            let next = ((2 * k + 1) as f64 * x * current - k as f64 * previous) / (k + 1) as f64;
            previous = current;
            current = next;
        }
        (current, previous)
    }

    #[test]
    fn radau_points_are_roots_of_the_defining_polynomial() {
        for &h in &H[1..] {
            let x = 2.0 * h - 1.0;
            let (p8, p7) = legendre(8, x);
            let (_, p6) = legendre(7, x);
            // f = P₇ + P₈ and its derivative, from Pₙ' = n (x Pₙ − Pₙ₋₁) / (x² − 1).
            let f = p7 + p8;
            let df = (7.0 * (x * p7 - p6) + 8.0 * (x * p8 - p7)) / (x * x - 1.0);
            // One Newton iteration would move the point by this much.
            let correction = (f / df).abs();
            assert!(correction < 1e-15, "h = {h}: off by {correction:e}");
        }
    }

    #[test]
    fn newton_and_power_forms_convert_both_ways() {
        let b: [DVec3; 7] = std::array::from_fn(|k| {
            DVec3::new(1.0 + k as f64, -0.5 * k as f64, 3.0 / (k + 1) as f64)
        });
        let g = newton_from_power(&b);
        for k in 0..7 {
            let rebuilt: DVec3 = (k..7).map(|m| NEWTON_TO_POWER[m][k] * g[m]).sum();
            assert!((rebuilt - b[k]).length() < 1e-12, "coefficient {k}");
        }
    }

    #[test]
    fn next_step_prediction_is_exact_for_polynomials() {
        // If the acceleration really is a degree-7 polynomial in time, the
        // prediction for the next step must reproduce it exactly.
        let b: [f64; 7] = [0.3, -1.2, 0.8, 2.0, -0.7, 0.1, 0.05];
        let a0 = 1.5;
        let a = |s: f64| a0 + (0..7).map(|k| b[k] * s.powi(k as i32 + 1)).sum::<f64>();

        let mut ias = Ias15::new();
        ias.prepare(1);
        ias.dt_last_done = 1.0;
        ias.br[0] = b.map(DVec3::splat);
        // Pretend the last prediction was perfect, so no correction is carried.
        ias.er[0] = ias.br[0];
        ias.predict_next_polynomial(0.6);

        let next_a0 = a(1.0);
        for u in [0.0_f64, 0.25, 0.5, 1.0] {
            let predicted = next_a0
                + (0..7)
                    .map(|k| ias.b[0][k].x * u.powi(k as i32 + 1))
                    .sum::<f64>();
            assert!((predicted - a(1.0 + 0.6 * u)).abs() < 1e-13, "u = {u}");
        }
    }

    /// A spring pulling every body toward the origin, with optional
    /// velocity-dependent damping: a = −ω² x − γ v. Has an exact solution.
    struct Spring {
        omega: f64,
        damping: f64,
    }

    impl Gravity for Spring {
        fn name(&self) -> &'static str {
            "test spring"
        }
        fn accelerations(&self, _time: f64, bodies: &[Body], out: &mut [DVec3]) {
            for (b, a) in bodies.iter().zip(out) {
                *a = -self.omega * self.omega * b.position - self.damping * b.velocity;
            }
        }
    }

    fn oscillator() -> System {
        System::new(vec![Body::new("mass", 0.0, 0.0).at(DVec3::X)])
    }

    #[test]
    fn harmonic_oscillator_matches_exact_solution_to_rounding_limit() {
        // With truncation error below machine precision, what remains is
        // rounding: ~10⁻¹⁶ per step, adding up randomly, so errors grow like
        // √N. (For orbits, energy errors also shift the period, making phase
        // errors grow faster, like N^(3/2). A spring's period doesn't depend
        // on its energy, so here both stay at √N.)
        let spring = Spring {
            omega: 1.0,
            damping: 0.0,
        };
        let mut system = oscillator();
        // Its acceleration passes through zero twice per cycle, which the
        // timescale criterion can't handle: use the original one.
        let mut ias = Ias15 {
            criterion: StepCriterion::HighestTerm,
            ..Ias15::new()
        };
        let end = 1000.0 * TAU;
        let mut steps = 0.0_f64;
        while system.time() < end {
            let remaining = end - system.time();
            ias.step(&mut system, &spring, remaining);
            steps += 1.0;
        }
        let t = system.time();
        let body = &system.bodies[0];
        let energy_error =
            (0.5 * (body.position.length_squared() + body.velocity.length_squared()) - 0.5).abs();
        let state_error = (body.position - DVec3::new(t.cos(), 0.0, 0.0)).length()
            + (body.velocity - DVec3::new(-t.sin(), 0.0, 0.0)).length();
        // Rounding over N steps, plus the clock's own resolution: one f64
        // near t can only say what time it is to within about t × ε.
        let bound = 10.0 * f64::EPSILON * steps.sqrt() + t * f64::EPSILON;
        println!(
            "{steps} steps: energy error {energy_error:.1e}, state error {state_error:.1e}, bound {bound:.1e}"
        );
        assert!(energy_error < bound, "energy error {energy_error:e}");
        assert!(state_error < bound, "state error {state_error:e}");
    }

    #[test]
    fn velocity_dependent_force_matches_exact_solution() {
        // Damped oscillator: x(t) = e^(−γt/2) [cos ω_d t + (γ / 2ω_d) sin ω_d t].
        let (omega, damping) = (2.0, 0.3);
        let spring = Spring { omega, damping };
        let mut system = oscillator();
        // An oscillator's acceleration passes through zero: the original
        // step criterion suits it.
        let mut ias = Ias15 {
            criterion: StepCriterion::HighestTerm,
            ..Ias15::new()
        };
        advance(&mut system, &spring, &mut ias, 30.0);
        let t = system.time();
        let wd = (omega * omega - damping * damping / 4.0).sqrt();
        let exact =
            (-damping * t / 2.0).exp() * ((wd * t).cos() + damping / (2.0 * wd) * (wd * t).sin());
        let error = (system.bodies[0].position.x - exact).abs();
        assert!(error < 1e-14, "error {error:e}");
    }
}
