"""Write the planets' magnetic dipoles and Earth's radiation belts.

Writes, under crates/worldline-data/data/:

- planetary-dipoles.csv: each magnetized planet's internal field to degree 1
  (the Schmidt-normalized Gauss coefficients g10, g11, h11, in nT, at the
  model's reference radius), from the field model named in each row.
  Earth's comes from IGRF-14, downloaded from NOAA NCEI; the others are
  copied from the papers that published them. Venus and Mars have no global
  field, so they aren't listed.
- radiation-belts.csv: the extent of Earth's two radiation belts in L, the
  equatorial distance of a dipole field line in Earth radii.

Run from anywhere:  python tools/fetch_magnetic_fields.py
Uses only the Python standard library.
"""

from datetime import date

from fetch_moons import DATA, fetch, write

IGRF = "https://www.ngdc.noaa.gov/IAGA/vmod/coeffs/igrf14coeffs.txt"
EPOCH = "2025.0"

# (planet, model, reference radius km, g10, g11, h11 nT, northward offset of
# the dipole along the spin axis in km, source). Earth's coefficients are
# filled in from IGRF.
PUBLISHED = [
    ("Mercury", "offset dipole", "2440", "-195", "0", "0", "484",
     "Anderson et al. 2012, J. Geophys. Res. 117, E00L12 (moment 195 nT R_M^3 pointing south, 484 km north of center)"),
    ("Jupiter", "JRM09", "71492", "410244.7", "-71498.3", "21330.5", "0",
     "Connerney et al. 2018, Geophys. Res. Lett. 45, 2590, doi:10.1002/2018GL077312"),
    ("Saturn", "Cassini Grand Finale (degree 9)", "60268", "21139", "0", "0", "0",
     "Cao et al. 2020, Icarus 344, 113541, Table 4 (axisymmetric to 0.007 degrees)"),
    ("Uranus", "Q3", "25600", "11893", "11579", "-15684", "0",
     "Connerney, Acuna & Ness 1987, J. Geophys. Res. 92, 15329, doi:10.1029/JA092iA13p15329"),
    ("Neptune", "O8", "24765", "9732", "3220", "-9889", "0",
     "Connerney, Acuna & Ness 1991, J. Geophys. Res. 96, 19023, doi:10.1029/91JA01165"),
]

# (planet, belt, L from, L to, source)
BELTS = [
    ("Earth", "inner", "1.1", "2.5",
     "typical extent in the Van Allen Probes era, e.g. Earth Planet. Phys. 2023, doi:10.26464/epp2023009"),
    ("Earth", "outer", "3", "7",
     "typical extent in the Van Allen Probes era, e.g. Earth Planet. Phys. 2023, doi:10.26464/epp2023009"),
]


def igrf_dipole():
    """IGRF-14's degree-1 coefficients at EPOCH, as published."""
    lines = fetch(IGRF).splitlines()
    header = next(l for l in lines if l.startswith("g/h"))
    column = header.split().index(EPOCH)
    coefficients = {}
    for line in lines:
        fields = line.split()
        if len(fields) > column and fields[1] == "1":
            coefficients[(fields[0], fields[2])] = fields[column]
    return coefficients[("g", "0")], coefficients[("g", "1")], coefficients[("h", "1")]


def main():
    g10, g11, h11 = igrf_dipole()
    earth = ("Earth", f"IGRF-14 at {EPOCH}", "6371.2", g10, g11, h11, "0",
             f"IAGA Working Group V-MOD, IGRF-14, from {IGRF}, fetched {date.today()}")
    rows = [PUBLISHED[0], earth] + PUBLISHED[1:]
    write(DATA / "planetary-dipoles.csv", [
        "# Worldline: the planets' internal magnetic dipoles. Regenerate with: python tools/fetch_magnetic_fields.py",
        "# Schmidt-normalized Gauss coefficients of degree 1 at the model's reference radius, in nT;"
        " longitudes in each model's body-fixed frame",
        "planet,model,reference_radius_km,g10_nt,g11_nt,h11_nt,north_offset_km,source",
    ] + [",".join(r[:7]) + ',"' + r[7] + '"' for r in rows])
    write(DATA / "radiation-belts.csv", [
        "# Worldline: where Earth's radiation belts lie, in L (equatorial distance of a dipole field line,"
        " in Earth radii). Regenerate with: python tools/fetch_magnetic_fields.py",
        "planet,belt,l_from,l_to,source",
    ] + [",".join(r[:4]) + ',"' + r[4] + '"' for r in BELTS])
    print(f"IGRF-14 {EPOCH}: g10 {g10}, g11 {g11}, h11 {h11}")


if __name__ == "__main__":
    main()
