"""Write the galactic center: where Sagittarius A* is on the sky, and the
orbits of the stars around it (the S-stars).

Writes, under crates/worldline-data/data/:

- sgr-a-star-position.csv: Sagittarius A*'s position (ICRF3) from VLBI
  radio astrometry.
- s-stars.csv: the orbital elements of 39 stars around Sagittarius A*.
  S2, S29, S38 and S55 come from the GRAVITY Collaboration's 2022
  four-star fit, whose elements are osculating at each star's apocenter
  epoch (in its Table B.1 note); the other 35 from Gillessen et al.
  2017, Table 3, a Keplerian fit. S111 (hyperbolic) is left out. Each row
  keeps the distance its paper converts angles with.

Values are copied by hand, digit for digit, from the papers named in each
row: they are published in tables, not in a queryable database.

Run from anywhere:  python tools/write_galactic_center.py
Uses only the Python standard library.
"""

import csv
import io

from fetch_moons import DATA, write

GRAVITY = ("GRAVITY Collaboration 2022, A&A 657, L12, Table B.1 (four-star fit; elements "
           "osculating at the apocenter epoch); spectral type and K magnitude: Gillessen et al. 2017")
GILLESSEN = "Gillessen et al. 2017, ApJ 837, 30, Table 3"

COLUMNS = ["star", "a_arcsec", "e", "i_deg", "node_deg", "periapsis_deg", "t_peri_yr",
           "period_yr", "osculating_yr", "r0_pc", "spectral_type", "k_mag", "source"]

# (star, a ["], e, i [°], Ω [°], ω [°], t_peri [yr], period [yr] (empty: from
# Kepler's law), osculating epoch [yr] (empty: Keplerian fit), R0 [pc],
# spectral type ('e' early, 'l' late, '' unknown), K magnitude, source)
STARS = [
    ("S2", "0.12495", "0.88441", "134.70", "228.19", "66.25", "2018.3789", "", "2010.35",
     "8277", "e", "13.95", GRAVITY),
    ("S29", "0.3975", "0.9693", "144.37", "7.00", "205.79", "2021.4104", "", "1977",
     "8277", "e", "16.7", GRAVITY),
    ("S38", "0.14254", "0.8145", "166.65", "109.45", "27.17", "2022.7044", "", "2000",
     "8277", "l", "17.", GRAVITY),
    ("S55", "0.10440", "0.7267", "158.52", "314.94", "322.78", "2021.6940", "", "2012",
     "8277", "", "17.5", GRAVITY),
    ("S1", "0.595", "0.556", "119.14", "342.04", "122.3", "2001.80", "166.0", "", "8320", "e", "14.7", GILLESSEN),
    ("S4", "0.3570", "0.3905", "80.33", "258.84", "290.8", "1957.4", "77.0", "", "8320", "e", "14.4", GILLESSEN),
    ("S6", "0.6574", "0.8400", "87.24", "85.07", "116.23", "2108.61", "192.0", "", "8320", "e", "15.4", GILLESSEN),
    ("S8", "0.4047", "0.8031", "74.37", "315.43", "346.70", "1983.64", "92.9", "", "8320", "e", "14.5", GILLESSEN),
    ("S9", "0.2724", "0.644", "82.41", "156.60", "150.6", "1976.71", "51.3", "", "8320", "e", "15.1", GILLESSEN),
    ("S12", "0.2987", "0.8883", "33.56", "230.1", "317.9", "1995.59", "58.9", "", "8320", "e", "15.5", GILLESSEN),
    ("S13", "0.2641", "0.4250", "24.70", "74.5", "245.2", "2004.86", "49.00", "", "8320", "e", "15.8", GILLESSEN),
    ("S14", "0.2863", "0.9761", "100.59", "226.38", "334.59", "2000.12", "55.3", "", "8320", "e", "15.7", GILLESSEN),
    ("S17", "0.3559", "0.397", "96.83", "191.62", "326.0", "1991.19", "76.6", "", "8320", "l", "15.3", GILLESSEN),
    ("S18", "0.2379", "0.471", "110.67", "49.11", "349.46", "1993.86", "41.9", "", "8320", "e", "16.7", GILLESSEN),
    ("S19", "0.520", "0.750", "71.96", "344.60", "155.2", "2005.39", "135", "", "8320", "e", "16.", GILLESSEN),
    ("S21", "0.2190", "0.764", "58.8", "259.64", "166.4", "2027.40", "37.00", "", "8320", "l", "16.9", GILLESSEN),
    ("S22", "1.31", "0.449", "105.76", "291.7", "95", "1996.9", "540", "", "8320", "e", "16.6", GILLESSEN),
    ("S23", "0.253", "0.56", "48.0", "249", "39.0", "2024.7", "45.8", "", "8320", "e", "17.8", GILLESSEN),
    ("S24", "0.944", "0.8970", "103.67", "7.93", "290", "2024.50", "331", "", "8320", "l", "15.6", GILLESSEN),
    ("S31", "0.449", "0.5497", "109.03", "137.16", "308.0", "2018.07", "108.", "", "8320", "e", "15.7", GILLESSEN),
    ("S33", "0.657", "0.608", "60.5", "100.1", "303.7", "1928", "192.0", "", "8320", "e", "16.", GILLESSEN),
    ("S39", "0.370", "0.9236", "89.36", "159.03", "23.3", "2000.06", "81.1", "", "8320", "", "16.8", GILLESSEN),
    ("S42", "0.95", "0.567", "67.16", "196.14", "35.8", "2008.24", "335", "", "8320", "e", "17.5", GILLESSEN),
    ("S54", "1.20", "0.893", "62.2", "288.35", "140.8", "2004.46", "477", "", "8320", "e", "17.5", GILLESSEN),
    ("S60", "0.3877", "0.7179", "126.87", "170.54", "29.37", "2023.89", "87.1", "", "8320", "e", "16.3", GILLESSEN),
    ("S66", "1.502", "0.128", "128.5", "92.3", "134", "1771", "664", "", "8320", "e", "14.8", GILLESSEN),
    ("S67", "1.126", "0.293", "136.0", "96.5", "213.5", "1705", "431", "", "8320", "e", "12.1", GILLESSEN),
    ("S71", "0.973", "0.899", "74.0", "35.16", "337.8", "1695", "346", "", "8320", "e", "16.1", GILLESSEN),
    ("S83", "1.49", "0.365", "127.2", "87.7", "203.6", "2046.8", "656", "", "8320", "e", "13.6", GILLESSEN),
    ("S85", "4.6", "0.78", "84.78", "107.36", "156.3", "1930.2", "3580", "", "8320", "l", "15.6", GILLESSEN),
    ("S87", "2.74", "0.224", "119.54", "106.32", "336.1", "611", "1640", "", "8320", "e", "13.6", GILLESSEN),
    ("S89", "1.081", "0.639", "87.61", "238.99", "126.4", "1783", "406", "", "8320", "l", "15.3", GILLESSEN),
    ("S91", "1.917", "0.303", "114.49", "105.35", "356.4", "1108", "958", "", "8320", "e", "12.2", GILLESSEN),
    ("S96", "1.499", "0.174", "126.36", "115.66", "233.6", "1646", "662", "", "8320", "e", "10.", GILLESSEN),
    ("S97", "2.32", "0.35", "113.0", "113.2", "28", "2132", "1270", "", "8320", "e", "10.3", GILLESSEN),
    ("S145", "1.12", "0.50", "83.7", "263.92", "185", "1808", "426", "", "8320", "l", "17.5", GILLESSEN),
    ("S175", "0.414", "0.9867", "88.53", "326.83", "68.52", "2009.51", "96.2", "", "8320", "e", "17.5", GILLESSEN),
    ("R34", "1.81", "0.641", "136.0", "330", "57.0", "1522", "877", "", "8320", "e", "14.", GILLESSEN),
    ("R44", "3.9", "0.27", "131.0", "80.5", "217", "1963", "2730", "", "8320", "e", "14.", GILLESSEN),
]


def rows(header, records):
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(header)
    writer.writerows(records)
    return out.getvalue().rstrip("\n").split("\n")


def main():
    assert all(len(row) == len(COLUMNS) for row in STARS)
    write(DATA / "sgr-a-star-position.csv", [
        "# Worldline: the position of Sagittarius A* on the sky. "
        "Regenerate with: python tools/write_galactic_center.py",
        "# ICRF3, J2000, at the 2015.0 proper-motion epoch, from VLBI radio astrometry",
    ] + rows(["ra_h", "ra_m", "ra_s", "dec_deg", "dec_arcmin", "dec_arcsec", "source"], [
        ("17", "45", "40.034047", "-29", "00", "28.21601",
         "Gordon, de Witt & Jacobs 2023, AJ, doi:10.3847/1538-3881/aca65b (RA uncertainty "
         "0.000018 s, Dec 0.00044 arcsec)"),
    ]))
    write(DATA / "s-stars.csv", [
        "# Worldline: stars orbiting Sagittarius A*. Regenerate with: python tools/write_galactic_center.py",
        "# Values copied by hand from the paper in each row's source. Angles in degrees: i the "
        "inclination, node the position angle of the ascending node (east of north), periapsis "
        "the argument of pericenter; the line of sight points away from the observer (positive "
        "radial velocity receding). a in arcseconds, converted with the row's r0_pc.",
        "# period_yr empty: from Kepler's law and the GRAVITY mass; osculating_yr: the epoch at "
        "which the elements are osculating (GRAVITY), empty for Keplerian fits",
    ] + rows(COLUMNS, STARS))


if __name__ == "__main__":
    main()
