//! The CIE 1931 colour-matching functions, 2° observer (CIE 018:2019,
//! Table 6; ISO/CIE 11664-1:2019): how strongly light of each wavelength
//! stirs the three responses x̄, ȳ and z̄ the CIE's colors are measured in,
//! ȳ being the eye's sensitivity to brightness. From the CIE's own dataset,
//! DOI 10.25039/CIE.DS.xvudnb9b, licensed CC BY-SA 4.0, fetched by
//! `tools/fetch_cie.py`. Used to draw a star the color of a blackbody at
//! its temperature (step 8.5b).

/// One wavelength's row: λ (nm), x̄, ȳ, z̄.
pub type CieRow = [f64; 4];

/// The table, 360 nm to 830 nm in steps of 1 nm, as the CIE publishes it.
pub fn cie_1931() -> Vec<CieRow> {
    include_str!("../data/cie-1931-2deg.csv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .skip(1)
        .map(|line| {
            let mut row = [0.0; 4];
            for (value, field) in row.iter_mut().zip(line.split(',')) {
                *value = field.parse().expect("a published number");
            }
            row
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_one_the_cie_published() {
        // Its columns' sums, as the dataset's metadata gives them
        // (CIE_xyz_1931_2deg.csv_metadata.json, "sumOfColumns"), and its
        // sample row 120, to the last digit printed.
        let table = cie_1931();
        assert_eq!(table.len(), 471);
        let sum = |k: usize| table.iter().map(|r| r[k]).sum::<f64>();
        assert_eq!(sum(0), 280245.0);
        for (k, published) in [
            (1, 106.865469489595),
            (2, 106.8569171011719),
            (3, 106.892251278636),
        ] {
            assert!((sum(k) - published).abs() < 1e-11, "column {k}: {}", sum(k));
        }
        assert_eq!(table[119], [479.0, 0.1042979, 0.1334528, 0.8566193]);
    }
}
