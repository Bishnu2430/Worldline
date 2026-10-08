"""Write the catalog of notable objects: black holes, neutron stars, white
dwarfs and nearby stars, with their published masses and sizes.

Writes, under crates/worldline-data/data/:

- notable-objects.csv: one row per object. Each value is copied by hand,
  digit for digit, from the paper named in its row; nothing is fetched,
  because these numbers live in papers, not in a queryable database. Where
  a size isn't measured, the row says how it was chosen:
    measured  - measured for this object;
    estimated - from an empirical relation, not measured directly;
    assumed   - not known for this object; another neutron star's
                measured radius stands in for it;
    horizon   - a black hole: its size is its event horizon, computed from
                its mass and spin (a hole whose spin isn't measured is
                treated as not spinning, which gives the largest horizon).
- binary-orbits.csv: the measured orbits of binaries in the catalog, and
  how fast their periods shrink (s/s): as measured, corrected for the
  galaxy's pull, and as general relativity predicts from the masses.

Uncertainties are as published: 1 sigma unless the source says otherwise
(the gravitational-wave catalog gives 90% credible intervals).

Run from anywhere:  python tools/write_notable_objects.py
Uses only the Python standard library.
"""

import csv
import io

from fetch_moons import DATA, write

GWTC1 = ("LIGO/Virgo GWTC-1: Abbott et al. 2019, Phys. Rev. X 9, 031040, Table III "
         "(medians and 90% credible intervals")
GW170817_RADII = "Abbott et al. 2018, Phys. Rev. Lett. 121, 161101 (11.9 +1.4/-1.4 km, 90% credible)"
ALPHA_CEN = ("Kervella, Thevenin & Lovis 2017, A&A 598, L7, Table 1: mass from Kervella et al. 2016 "
             "(A&A 594, A107), radius from interferometry by Kervella et al. 2017 (A&A 597, A137); "
             "distance 1.3384 +/- 0.0011 pc")
SIRIUS = ("parallax 378.9 +/- 1.4 mas (Bond et al. 2017, ApJ 840, 70, as used by Joyce et al. 2018, "
          "MNRAS 481, 2361)")

COLUMNS = [
    "name", "kind", "system",
    "mass_msun", "mass_plus", "mass_minus",
    "radius", "radius_unit", "radius_plus", "radius_minus", "radius_basis",
    "spin", "spin_basis",
    "distance", "distance_unit",
    "where", "source",
]

# Distances: "pc", "kpc" or "Mpc"; "mas" for a parallax; "z" for a redshift
# (TON 618, whose distance depends on the cosmological model).
OBJECTS = [
    # Black holes.
    ("Sagittarius A*", "black hole", "",
     "4.297e6", "0.012e6", "0.012e6",
     "", "", "", "", "horizon", "", "",
     "8277", "pc", "The center of the Milky Way, in Sagittarius",
     "GRAVITY Collaboration 2022, A&A 657, L12: M = (4.297 +/- 0.012) x 10^6 M_sun and "
     "R0 = 8277 +/- 9 pc (statistical; systematic about 0.04 x 10^6 M_sun and 30 pc), from the "
     "orbits of the stars S2, S29, S38 and S55"),
    ("M87*", "black hole", "",
     "6.5e9", "0.7e9", "0.7e9",
     "", "", "", "", "horizon", "", "",
     "16.8", "Mpc", "The center of the galaxy M87, in Virgo",
     "Event Horizon Telescope Collaboration 2019, ApJL 875, L1 (M = (6.5 +/- 0.7) x 10^9 M_sun) "
     "and L6 (GM/Dc^2 = 3.8 +/- 0.4 microarcseconds; D = 16.8 +0.8/-0.7 Mpc)"),
    ("TON 618", "black hole", "",
     "10^10.82", "", "",
     "", "", "", "", "horizon", "", "",
     "2.219", "z", "A quasar in Canes Venatici",
     "Shemmer et al. 2004, ApJ 614, 547: log M_BH = 10.82 from the width of its H-beta line "
     "(Table 2; typical uncertainty a factor of two); redshift 2.219 from NED (Table 1)"),
    ("Cygnus X-1", "black hole", "",
     "21.2", "2.2", "2.2",
     "", "", "", "", "horizon", "0.9985", "lower limit",
     "2.22", "kpc", "In the Milky Way, in Cygnus",
     "Miller-Jones et al. 2021, Science 371, 1046 (mass; distance 2.22 +0.18/-0.17 kpc); "
     "spin above 0.9985 (3 sigma): Zhao et al. 2021, ApJ 908, 117"),
    ("Gaia BH1", "black hole", "",
     "9.62", "0.18", "0.18",
     "", "", "", "", "horizon", "", "",
     "480", "pc", "In the Milky Way, in Ophiuchus: the nearest known black hole",
     "El-Badry et al. 2023, MNRAS 518, 1057"),
    ("Gaia BH3", "black hole", "",
     "32.70", "0.82", "0.82",
     "", "", "", "", "horizon", "", "",
     "590", "pc", "In the Milky Way, in Aquila: the heaviest stellar black hole known in it",
     "Gaia Collaboration (Panuzzo et al.) 2024, A&A 686, L2"),
    ("GW150914 heavier hole", "black hole", "GW150914",
     "35.6", "4.7", "3.1",
     "", "", "", "", "horizon", "", "",
     "440", "Mpc", "Merged in a distant galaxy; its gravitational waves reached Earth on 14 September 2015",
     GWTC1 + "; luminosity distance 440 +150/-170 Mpc)"),
    ("GW150914 lighter hole", "black hole", "GW150914",
     "30.6", "3.0", "4.4",
     "", "", "", "", "horizon", "", "",
     "440", "Mpc", "Merged in a distant galaxy; its gravitational waves reached Earth on 14 September 2015",
     GWTC1 + "; luminosity distance 440 +150/-170 Mpc)"),
    ("GW150914 final hole", "black hole", "GW150914",
     "63.1", "3.4", "3.0",
     "", "", "", "", "horizon", "0.69", "measured",
     "440", "Mpc", "Formed when the two holes of GW150914 merged",
     GWTC1 + "; final spin 0.69 +0.05/-0.04; luminosity distance 440 +150/-170 Mpc)"),
    # Neutron stars.
    ("PSR B1913+16", "neutron star", "PSR B1913+16",
     "1.438", "0.001", "0.001",
     "11.9", "km", "1.4", "1.4", "assumed", "", "",
     "", "", "In the Milky Way, in Aquila: the Hulse-Taylor binary pulsar",
     "Weisberg & Huang 2016, ApJ 829, 55 (masses from the periastron advance and the Einstein "
     "delay); radius not measured: GW170817's, " + GW170817_RADII),
    ("PSR B1913+16 companion", "neutron star", "PSR B1913+16",
     "1.390", "0.001", "0.001",
     "11.9", "km", "1.4", "1.4", "assumed", "", "",
     "", "", "In the Milky Way, in Aquila: the Hulse-Taylor binary pulsar",
     "Weisberg & Huang 2016, ApJ 829, 55 (masses from the periastron advance and the Einstein "
     "delay); radius not measured: GW170817's, " + GW170817_RADII),
    ("PSR J0740+6620", "neutron star", "",
     "2.08", "0.07", "0.07",
     "12.39", "km", "1.30", "0.98", "measured", "", "",
     "1.14", "kpc", "In the Milky Way: one of the heaviest neutron stars known",
     "Fonseca et al. 2021, ApJL 915, L12 (mass from the Shapiro delay; distance 1.14 +0.17/-0.15 "
     "kpc); radius: Riley et al. 2021, ApJL 918, L27 (NICER)"),
    ("PSR J0030+0451", "neutron star", "",
     "1.34", "0.15", "0.16",
     "12.71", "km", "1.14", "1.19", "measured", "", "",
     "", "", "In the Milky Way, in Pisces",
     "Riley et al. 2019, ApJL 887, L21 (NICER)"),
    ("GW170817 heavier star", "neutron star", "GW170817",
     "1.46", "0.12", "0.10",
     "11.9", "km", "1.4", "1.4", "measured", "", "",
     "40", "Mpc", "Merged in the galaxy NGC 4993; seen on 17 August 2017 in gravitational waves and light",
     GWTC1 + ", low-spin prior; luminosity distance 40 +7/-15 Mpc); radius: " + GW170817_RADII),
    ("GW170817 lighter star", "neutron star", "GW170817",
     "1.27", "0.09", "0.09",
     "11.9", "km", "1.4", "1.4", "measured", "", "",
     "40", "Mpc", "Merged in the galaxy NGC 4993; seen on 17 August 2017 in gravitational waves and light",
     GWTC1 + ", low-spin prior; luminosity distance 40 +7/-15 Mpc); radius: " + GW170817_RADII),
    # White dwarfs.
    ("Sirius B", "white dwarf", "Sirius",
     "1.018", "0.011", "0.011",
     "0.803e-2", "R_sun", "0.011e-2", "0.011e-2", "measured", "", "",
     "378.9", "mas", "Orbiting Sirius A, in Canis Major",
     "Mass from the visual-binary orbit: Bond et al. 2017, ApJ 840, 70. Radius from its flux and "
     "parallax: Joyce et al. 2018, MNRAS 481, 2361 (0.803 +/- 0.011 R_sun/100), which also measured "
     "its gravitational redshift, 80.65 +/- 0.77 km/s; " + SIRIUS),
    # Stars.
    ("Sirius A", "star", "Sirius",
     "2.063", "0.023", "0.023",
     "1.711", "R_sun", "0.013", "0.013", "measured", "", "",
     "378.9", "mas", "In Canis Major: the brightest star in the night sky",
     "Mass: Bond et al. 2017, ApJ 840, 70. Radius from its angular diameter, 5.936 +/- 0.016 mas: "
     "Kervella et al. 2003, A&A 408, 681; " + SIRIUS),
    ("Alpha Centauri A", "star", "Alpha Centauri",
     "1.1055", "0.0039", "0.0039",
     "1.2234", "R_sun", "0.0053", "0.0053", "measured", "", "",
     "1.3384", "pc", "In Centaurus: the nearest star like the Sun",
     ALPHA_CEN),
    ("Alpha Centauri B", "star", "Alpha Centauri",
     "0.9373", "0.0033", "0.0033",
     "0.8632", "R_sun", "0.0037", "0.0037", "measured", "", "",
     "1.3384", "pc", "In Centaurus, orbiting Alpha Centauri A",
     ALPHA_CEN),
    ("Proxima Centauri", "star", "Alpha Centauri",
     "0.1221", "0.0022", "0.0022",
     "0.1542", "R_sun", "0.0045", "0.0045", "estimated", "", "",
     "1.3008", "pc", "In Centaurus: the nearest star to the Sun, a red dwarf",
     "Kervella, Thevenin & Lovis 2017, A&A 598, L7, Table 1: mass and radius from its infrared "
     "brightness through the relations of Mann et al. 2015 (not measured directly); distance "
     "1.3008 +/- 0.0006 pc (Benedict et al. 1999)"),
]

# (system, orbital period in days, eccentricity, periastron advance in
# degrees per year, source).
ORBITS = [
    ("PSR B1913+16", "0.322997448918", "0.6171340", "4.226585",
     "-2.398e-12", "0.004e-12", "-2.40263e-12", "0.00005e-12",
     "Weisberg & Huang 2016, ApJ 829, 55, Table 2: Pb = 0.322997448918(3) d, e = 0.6171340(4), "
     "mean periastron advance 4.226585(4) deg/yr; intrinsic Pb-dot -(2.398 +- 0.004)e-12 "
     "(observed -2.423(1)e-12 less the galactic -(0.025 +- 0.004)e-12), and general relativity's "
     "prediction from the measured masses (Peters & Mathews 1963) -(2.40263 +- 0.00005)e-12"),
]


def rows(header, records):
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(header)
    writer.writerows(records)
    return out.getvalue().rstrip("\n").split("\n")


def main():
    assert all(len(row) == len(COLUMNS) for row in OBJECTS)
    write(DATA / "notable-objects.csv", [
        "# Worldline: notable black holes, neutron stars, white dwarfs and nearby stars. "
        "Regenerate with: python tools/write_notable_objects.py",
        "# Values copied by hand from the paper in each row's source. Masses in solar masses; "
        "radius_basis says whether a radius is measured, estimated, assumed or an event horizon "
        "(computed from the mass and spin; non-spinning if the spin isn't measured)",
    ] + rows(COLUMNS, OBJECTS))
    write(DATA / "binary-orbits.csv", [
        "# Worldline: measured orbits of binaries in notable-objects.csv. "
        "Regenerate with: python tools/write_notable_objects.py",
    ] + rows(["system", "period_days", "eccentricity", "periastron_advance_deg_per_yr",
              "period_derivative", "period_derivative_sigma", "period_derivative_gr",
              "period_derivative_gr_sigma", "source"],
             ORBITS))


if __name__ == "__main__":
    main()
