//! Saturn's rings: a measured optical-depth profile and the named features.
//!
//! The profile is a Cassini radio occultation, the table is the PDS Ring-Moon
//! Systems Node's list of rings, gaps and ringlets. `tools/fetch_rings.py`
//! regenerates both. See `docs/physics/saturn-rings.md`.

use std::sync::OnceLock;

/// Normal optical depth of the rings versus distance from Saturn's center.
///
/// Optical depth τ measures how much light the ring stops: light passing
/// straight through keeps a fraction e^(−τ). Seen at an angle, the path is
/// longer and e^(−τ/μ) gets through, where μ is the cosine of the angle from
/// the ring's normal.
#[derive(Debug, Clone, PartialEq)]
pub struct RingProfile {
    /// Inner edge of the first bin, in m.
    pub inner: f64,
    /// Width of each bin, in m.
    pub bin: f64,
    /// Optical depth in each bin. Noise below zero is set to zero; where the
    /// signal was lost (the B ring's core) the value is the detection
    /// threshold, a lower limit on the true value.
    pub tau: Vec<f64>,
}

impl RingProfile {
    /// Outer edge of the last bin, in m.
    pub fn outer(&self) -> f64 {
        self.inner + self.bin * self.tau.len() as f64
    }

    /// Optical depth at `radius` (m), or 0 outside the profile.
    pub fn tau_at(&self, radius: f64) -> f64 {
        let index = ((radius - self.inner) / self.bin).floor();
        if index < 0.0 {
            return 0.0;
        }
        self.tau.get(index as usize).copied().unwrap_or(0.0)
    }

    /// Fraction of light passing straight through each bin, e^(−τ).
    pub fn transmission(&self) -> Vec<f64> {
        self.tau.iter().map(|t| (-t).exp()).collect()
    }
}

/// A named ring, region, gap or ringlet.
#[derive(Debug, Clone, PartialEq)]
pub struct RingFeature {
    /// Name as published, e.g. "Cassini Division".
    pub name: String,
    /// Inner boundary, in m.
    pub inner: f64,
    /// Outer boundary, in m (equal to `inner` for features with one radius).
    pub outer: f64,
}

/// The measured profile of Saturn's main rings, 72,770–145,000 km: Cassini
/// Radio Science occultation Rev 7, X band, in 10 km bins.
pub fn saturn_rings() -> &'static RingProfile {
    static PROFILE: OnceLock<RingProfile> = OnceLock::new();
    PROFILE.get_or_init(|| parse_profile(include_str!("../data/saturn-rings-profile.csv")))
}

/// Saturn's named rings, regions, gaps and ringlets, from the PDS
/// Ring-Moon Systems Node.
pub fn saturn_ring_features() -> Vec<RingFeature> {
    data_lines(include_str!("../data/saturn-ring-features.csv"))
        .map(|line| {
            let fields: Vec<&str> = line.splitn(4, ',').collect();
            let km = |s: &str| s.parse::<f64>().expect("ring boundary is a number") * 1e3;
            RingFeature {
                name: fields[0].to_string(),
                inner: km(fields[1]),
                outer: km(fields[2]),
            }
        })
        .collect()
}

fn parse_profile(text: &str) -> RingProfile {
    let rows: Vec<(f64, f64, f64)> = data_lines(text)
        .map(|line| {
            let f: Vec<f64> = line
                .split(',')
                .map(|x| x.parse().expect("profile value is a number"))
                .collect();
            (f[0] * 1e3, f[1], f[2])
        })
        .collect();
    let bin = rows[1].0 - rows[0].0;
    RingProfile {
        inner: rows[0].0 - bin / 2.0,
        bin,
        tau: rows
            .iter()
            .map(|&(_, tau, threshold)| tau.clamp(0.0, threshold))
            .collect(),
    }
}

/// Lines of a CSV file after its comments and header.
fn data_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .skip(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_covers_the_main_rings_in_10_km_bins() {
        let profile = saturn_rings();
        assert_eq!(profile.bin, 10_000.0);
        assert!(profile.inner < 74_491e3, "starts inside the C ring");
        assert!(profile.outer() > 140_612e3, "reaches past the F ring");
        assert!(profile.tau.iter().all(|&t| (0.0..=5.1).contains(&t)));
    }

    #[test]
    fn features_load() {
        let features = saturn_ring_features();
        let cassini = features
            .iter()
            .find(|f| f.name == "Cassini Division")
            .unwrap();
        assert_eq!((cassini.inner, cassini.outer), (117_500e3, 122_050e3));
        assert_eq!(features.len(), 41);
    }
}
