//! The simulations behind the collision outcomes of step 6.1: Leinhardt &
//! Stewart's (2012) collisions between 10-km rubble piles. See
//! `tools/fetch_impacts.py`.

/// A number as the paper prints it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Printed {
    /// Its value.
    pub value: f64,
    /// How far the true value can be from it: half the last digit printed.
    pub rounding: f64,
}

impl Printed {
    fn parse(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        let (digits, exponent) = text.split_once('e').unwrap_or((text, "0"));
        let exponent: i32 = exponent.parse().ok()?;
        let places = digits.split_once('.').map_or(0, |(_, d)| d.len()) as i32;
        Some(Self {
            value: text.parse().ok()?,
            rounding: 0.5 * 10f64.powi(exponent - places),
        })
    }
}

/// A simulated collision.
#[derive(Debug, Clone, PartialEq)]
pub struct ImpactRun {
    /// The projectile's mass over the target's.
    pub mass_ratio: f64,
    /// The impact parameter, the sine of the impact angle.
    pub b: f64,
    /// The impact speed, m/s.
    pub speed: f64,
    /// The largest remnant, as a share of the total mass.
    pub largest: f64,
    /// The second-largest.
    pub second: f64,
    /// The specific impact energy Q_R, J/kg.
    pub energy: Printed,
}

/// A simulated collision of the paper's Table 1, with what the paper
/// derives from it.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedRun {
    /// The collision.
    pub run: ImpactRun,
    /// The disruption energy Q′*_RD fitted to its series of three, J/kg.
    pub threshold: Printed,
    /// The interacting mass fraction α of its series.
    pub alpha: Printed,
    /// The paper's predicted largest remnant, unless super-catastrophic.
    pub predicted_largest: Option<Printed>,
    /// And second-largest.
    pub predicted_second: Option<Printed>,
    /// The paper's mark on its prediction: "d" for two equal remnants, "ss"
    /// for an erosive hit-and-run, "supercat", or nothing.
    pub mark: String,
}

/// The paper's own target: its radius, m.
pub const RUBBLE_PILE_RADIUS: f64 = 1e4;

/// And its density, kg/m³.
pub const RUBBLE_PILE_DENSITY: f64 = 1000.0;

/// Every simulation of the paper's Table A.3.
pub fn impact_simulations() -> Vec<ImpactRun> {
    include_str!("../data/impact-simulations-all.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            run(&f, f[5])
        })
        .collect()
}

fn run(f: &[&str], energy: &str) -> ImpactRun {
    let number = |i: usize| f[i].parse::<f64>().expect("bundled impact data is valid");
    ImpactRun {
        mass_ratio: number(0),
        b: number(1),
        speed: number(2),
        largest: number(3),
        second: number(4),
        energy: Printed::parse(energy).expect("every run has an energy"),
    }
}

/// The simulations of the paper's Table 1, in series of three.
pub fn selected_impact_simulations() -> Vec<SelectedRun> {
    let rows: Vec<Vec<&str>> = include_str!("../data/impact-simulations-selected.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').collect())
        .collect();
    rows.chunks(3)
        .flat_map(|series| {
            // The table prints each series' fit on one of its rows.
            let printed = |column: usize| {
                series
                    .iter()
                    .find_map(|f| Printed::parse(f[column]))
                    .expect("each series is fitted")
            };
            let (threshold, alpha) = (printed(7), printed(8));
            series.iter().map(move |f| SelectedRun {
                run: run(f, f[6]),
                threshold,
                alpha,
                predicted_largest: Printed::parse(f[9]),
                predicted_second: Printed::parse(f[10]),
                mark: f[12].to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_simulations_load() {
        assert_eq!(impact_simulations().len(), 189);
        let selected = selected_impact_simulations();
        assert_eq!(selected.len(), 48);
        // Equal masses, b = 0.35, 17 m/s: the series' Q′*_RD and α are
        // printed on the next row.
        let row = &selected[3];
        assert_eq!(
            (row.run.mass_ratio, row.run.b, row.run.speed),
            (1.0, 0.35, 17.0)
        );
        assert_eq!(row.threshold.value, 54.0);
        assert_eq!(row.threshold.rounding, 0.5);
        assert_eq!(row.alpha.value, 0.72);
        assert_eq!(row.mark, "d");
        let cat = selected.iter().filter(|s| s.mark == "supercat").count();
        assert_eq!(cat, 1);
        assert_eq!(selected.iter().filter(|s| s.mark == "d").count(), 8);
    }

    #[test]
    fn printed_numbers_know_their_rounding() {
        let p = Printed::parse("7.2e1").unwrap();
        assert_eq!(p.value, 72.0);
        assert!((p.rounding - 0.5).abs() < 1e-12);
        assert_eq!(Printed::parse("0.003").unwrap().rounding, 0.5e-3);
        assert_eq!(Printed::parse(""), None);
    }
}
