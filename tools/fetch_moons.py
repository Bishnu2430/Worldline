"""Fetch the major moons Worldline ships with.

For Mars, Jupiter, Saturn, Uranus, Neptune and Pluto this writes, under
crates/worldline-data/data/:

- moons-<date>.csv: the planet's center and its major moons, with their
  positions and velocities (NASA JPL Horizons) at the snapshot epoch and
  30 days later (the reference for validation), their GM, and their mean
  radius.
- zonal-harmonics.csv: each planet's oblateness coefficients J2..J6 and
  their reference radius.
- tesseral-harmonics.csv: the longitude-dependent coefficients C_nm, S_nm
  where JPL's integration uses them (Mars, to degree 6).
- moon-figures.csv: the J2 and C22 of tidally locked moons, where JPL's
  integration uses them with a reference radius.
- ring-masses.csv: ring masses JPL's integration includes (Saturn's).

GMs and zonal harmonics are the constants JPL used to integrate the same
satellite ephemerides Horizons serves the states from, copied from the
comment files NASA NAIF publishes alongside them. A comment file can hold
several integrations; the one used is the one that covers the epochs and
integrated all the requested moons. Mixing solutions matters: a planet GM
off by 1 part in 10,000 shifts a moon by 1 part in 5,000 of its path.
Radii are the IAU mean radii from
crates/worldline-data/data/pck00011-subset.tpc, so run
tools/fetch_rotation.py first.

Run from anywhere:  python tools/fetch_moons.py
Uses only the Python standard library.
"""

import json
import re
import urllib.parse
import urllib.request
from datetime import date
from pathlib import Path

HORIZONS = "https://ssd.jpl.nasa.gov/api/horizons.api"
NAIF = "https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/satellites/"
DATA = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data"

# Julian Date (TDB) -> (file name, human-readable epoch)
EPOCHS = {
    2460676.5: ("moons-2025-01-01.csv", "2025-01-01 00:00:00 TDB"),
    2460706.5: ("moons-2025-01-31.csv", "2025-01-31 00:00:00 TDB"),
}

# (host as named in the solar-system snapshot, NAIF comment file, planet id,
# moons as (name, id)). The comment files match the solutions Horizons
# reports as each moon's source: mar099, jup365_merged, sat441l,
# ura184_merged (whose major moons come from the URA182 integration, described
# in ura184_part-1.cmt), nep098_merged and plu060.
SYSTEMS = [
    ("Mars", "mar099", 499, [("Phobos", 401), ("Deimos", 402)]),
    ("Jupiter", "jup365", 599, [("Io", 501), ("Europa", 502), ("Ganymede", 503), ("Callisto", 504)]),
    ("Saturn", "sat441", 699, [
        ("Mimas", 601), ("Enceladus", 602), ("Tethys", 603), ("Dione", 604),
        ("Rhea", 605), ("Titan", 606), ("Hyperion", 607), ("Iapetus", 608),
    ]),
    ("Uranus", "ura184_part-1", 799, [
        ("Miranda", 705), ("Ariel", 701), ("Umbriel", 702), ("Titania", 703), ("Oberon", 704),
    ]),
    ("Neptune", "nep098_part-1", 899, [("Triton", 801)]),
    ("Pluto", "plu060", 999, [("Charon", 901)]),
]

# A row of a comment file's body table:  Io   501   5.959915466180539E+03   16   15   SATORBINT
TABLE_ROW = re.compile(r"^\s*[A-Za-z_0-9]+\s+(\d{3})\s+([-+0-9.E]+)\s+\d+\s+\d+\s+SATORBINT", re.M)


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "Worldline data fetch (github.com/Bishnu2430/Worldline)"})
    with urllib.request.urlopen(request, timeout=180) as response:
        return response.read().decode("utf-8", errors="replace")


def ephemeris_constants(name, planet_id, moon_ids):
    """The integration's name, GMs (km^3/s^2) by NAIF id, zonal coefficients
    J_n by degree, tesseral coefficients {"C": .., "S": ..} by (degree,
    order), and the reference radius (km), from the integration in a NAIF
    satellite ephemeris comment file that covers the epochs and integrated
    all of `moon_ids`."""
    text = fetch(NAIF + name + ".cmt")
    for block in re.split(r"^\s*Satellite Ephemeris:", text, flags=re.M)[1:]:
        span = re.search(r"Timespan from JED\s+([\d.]+)\S*\s+to JED\s+([\d.]+)", block)
        if not span or not float(span.group(1)) <= min(EPOCHS) < max(EPOCHS) <= float(span.group(2)):
            continue
        gms = {int(m.group(1)): m.group(2) for m in TABLE_ROW.finditer(block)}
        if not all(i in gms for i in moon_ids):
            continue
        # Constants such as 499GM, or 700GM for the planet in some files.
        for m in re.finditer(r"\b(\d{3})GM\s+([-+0-9.E]+)", block):
            gms.setdefault(int(m.group(1)), m.group(2))
        if planet_id not in gms:
            gms[planet_id] = gms[planet_id - 99]
        zonal = {}
        for m in re.finditer(r"\bJ[4-9](\d{2})\s+([-+0-9.E]+)", block):
            degree = int(m.group(1))
            if 2 <= degree <= 6:
                zonal.setdefault(degree, m.group(2))
        # Constants such as C40202 and S40202: planet 4, degree 2, order 2.
        tesseral = {}
        for m in re.finditer(r"\b([CS])\d(\d{2})(\d{2})\s+([-+0-9.E]+)", block):
            tesseral.setdefault((int(m.group(2)), int(m.group(3))), {})[m.group(1)] = m.group(4)
        # Moon shapes, written 401RAD / 401J02 / 401C02_02 in some files and
        # S1RADEQ / S1J2 / S1C22 (moon 1 of the planet) in others.
        figures = {}
        for pattern, key in [
            (r"\b(\d{3})RAD\s+([-+0-9.E]+)", "radius"),
            (r"\b(\d{3})J02\s+([-+0-9.E]+)", "j2"),
            (r"\b(\d{3})C02_02\s+([-+0-9.E]+)", "c22"),
            (r"\bS(\d)RADEQ\s+([-+0-9.E]+)", "radius"),
            (r"\bS(\d)J2\s+([-+0-9.E]+)", "j2"),
            (r"\bS(\d)C22\s+([-+0-9.E]+)", "c22"),
        ]:
            for m in re.finditer(pattern, block):
                number = int(m.group(1))
                naif_id = number if number > 99 else planet_id - 99 + number
                figures.setdefault(naif_id, {})[key] = m.group(2)
        # Ring masses, such as Saturn's RINGM1, RINGM2 and RINGM4, in km^3/s^2.
        rings = {m.group(1): m.group(2) for m in re.finditer(r"\b(RINGM\d)\s+([-+0-9.E]+)", block)}
        radius = re.search(r"\bRADIUS\s+([-+0-9.E]+)", block).group(1)
        return block.split()[0], gms, zonal, tesseral, figures, rings, radius
    raise SystemExit(f"{name}.cmt has no integration of {moon_ids} covering the epochs")


def mean_radii():
    """IAU mean radius (km) by NAIF id, from the bundled kernel subset."""
    text = (DATA / "pck00011-subset.tpc").read_text(encoding="utf-8")
    radii = {}
    for m in re.finditer(r"BODY(\d+)_RADII\s*=\s*\(([^)]*)\)", text):
        values = [float(v) for v in m.group(2).split()]
        radii[int(m.group(1))] = sum(values) / len(values)
    return radii


class NoEphemeris(Exception):
    """Horizons has no ephemeris for the body over the epochs."""


def states(naif_id, center="500@0"):
    """Horizons states at both epochs, relative to `center` (the solar
    system barycenter by default), and the ephemeris they come from."""
    params = {
        "format": "json",
        "COMMAND": f"'{naif_id}'",
        "OBJ_DATA": "'NO'",
        "MAKE_EPHEM": "'YES'",
        "EPHEM_TYPE": "'VECTORS'",
        "CENTER": f"'{center}'",
        "START_TIME": f"'JD{min(EPOCHS)}'",
        "STOP_TIME": f"'JD{max(EPOCHS)}'",
        "STEP_SIZE": "'1'",
        "TIME_TYPE": "'TDB'",
        "REF_PLANE": "'ECLIPTIC'",
        "REF_SYSTEM": "'ICRF'",
        "OUT_UNITS": "'KM-S'",
        "VEC_TABLE": "'2'",
        "CSV_FORMAT": "'YES'",
    }
    result = json.loads(fetch(HORIZONS + "?" + urllib.parse.urlencode(params)))["result"]
    if "No ephemeris for target" in result:
        raise NoEphemeris(result.strip().splitlines()[-1])
    source = re.search(r"Target body name: .*\{source: ([^}]+)\}", result).group(1).strip()
    block = result.split("$$SOE")[1].split("$$EOE")[0]
    rows = {}
    for line in block.strip().splitlines():
        fields = [f.strip() for f in line.split(",")]
        rows[float(fields[0])] = fields[2:8]
    return source, rows


def write(path, lines):
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines) + "\n")
    print(f"Wrote {path}")


def main():
    radii = mean_radii()
    rows = {jd: [] for jd in EPOCHS}
    sources = set()
    zonal_rows = []
    tesseral_rows = []
    figure_rows = []
    ring_rows = []
    integrations = []
    for host, ephemeris, planet_id, moons in SYSTEMS:
        print(f"{host}: reading {ephemeris}.cmt")
        integration, gms, zonal, tesseral, figures, rings, radius = ephemeris_constants(
            ephemeris, planet_id, [i for _, i in moons]
        )
        for constant in sorted(rings):
            ring_rows.append(f"{host},{constant},{rings[constant]}")
        for name, naif_id in moons:
            # Only shapes given with their reference radius; Saturn's file
            # lists J2 and C22 for some moons without one.
            figure = figures.get(naif_id, {})
            if {"radius", "j2", "c22"} <= figure.keys():
                figure_rows.append(f"{name},{naif_id},{figure['radius']},{figure['j2']},{figure['c22']}")
        integrations.append(f"{integration} in {ephemeris}.cmt")
        print(f"  using {integration}")
        for degree in sorted(zonal):
            zonal_rows.append(f"{host},{radius},{degree},{zonal[degree]}")
        for degree, order in sorted(tesseral):
            terms = tesseral[(degree, order)]
            tesseral_rows.append(f"{host},{radius},{degree},{order},{terms['C']},{terms['S']}")
        for name, naif_id in [(host, planet_id)] + moons:
            print(f"  fetching {name}")
            source, fetched = states(naif_id)
            sources.add(source)
            gm = gms[naif_id]
            for jd in EPOCHS:
                fields = [name, str(naif_id), host, gm, f"{radii[naif_id]:.4f}", *fetched[jd]]
                rows[jd].append(",".join(fields))

    provenance = [
        "# source: constants of the JPL satellite ephemeris integrations, from the NASA NAIF comment",
        f"#         files: {', '.join(integrations)}; fetched {date.today()}",
    ]
    zonal_header = [
        "# Planets' zonal gravity harmonics (unnormalized J_n) and their reference radius.",
        "# Regenerate with: python tools/fetch_moons.py",
        *provenance,
        "planet,reference_radius_km,degree,j",
    ]
    for jd, (file_name, label) in EPOCHS.items():
        header = [
            "# Worldline major moons, with their planets' centers. Regenerate with: python tools/fetch_moons.py",
            f"# epoch_jd_tdb: {jd}",
            f"# epoch: {label}",
            f"# states: NASA JPL Horizons ({', '.join(sorted(sources))}), fetched {date.today()}",
            "# frame: ICRF, ecliptic and mean equinox of J2000, origin at the solar system barycenter",
            "# units: km, km/s, km^3/s^2",
            f"# gm: JPL satellite ephemeris constants ({', '.join(integrations)}); radius: IAU mean radius (pck00011)",
            "# parent: the planetary system's name in the solar-system snapshot",
            "name,naif_id,parent,gm_km3_s2,radius_km,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s",
        ]
        write(DATA / file_name, header + rows[jd])
    write(DATA / "zonal-harmonics.csv", zonal_header + zonal_rows)
    tesseral_header = [
        "# Planets' tesseral gravity harmonics (unnormalized C_nm, S_nm) and their reference radius,",
        "# in the planet's body-fixed frame. Regenerate with: python tools/fetch_moons.py",
        *provenance,
        "planet,reference_radius_km,degree,order,c,s",
    ]
    write(DATA / "tesseral-harmonics.csv", tesseral_header + tesseral_rows)
    figure_header = [
        "# Shapes of tidally locked moons (unnormalized J2 and C22) and their reference radius.",
        "# Regenerate with: python tools/fetch_moons.py",
        *provenance,
        "name,naif_id,reference_radius_km,j2,c22",
    ]
    write(DATA / "moon-figures.csv", figure_header + figure_rows)
    ring_header = [
        "# Masses of planetary rings that JPL's integrations include, as GM, under the constant's name.",
        "# Regenerate with: python tools/fetch_moons.py",
        *provenance,
        "planet,constant,gm_km3_s2",
    ]
    write(DATA / "ring-masses.csv", ring_header + ring_rows)


if __name__ == "__main__":
    main()
