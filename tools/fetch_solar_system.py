"""Fetch the solar-system snapshots that Worldline ships with.

Downloads positions and velocities from NASA JPL Horizons (ephemeris DE441)
and gravitational parameters (GM) from JPL's DE440 constants file
(NAIF gm_de440.tpc). Writes them as plain CSV files under
crates/worldline-data/data/.

Two epochs are written: the starting snapshot, and one Julian year later,
which the validation test uses as the "right answer".

Run from anywhere:  python tools/fetch_solar_system.py
Uses only the Python standard library.
"""

import json
import re
import urllib.parse
import urllib.request
from datetime import date
from pathlib import Path

HORIZONS = "https://ssd.jpl.nasa.gov/api/horizons.api"
GM_FILE = "https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/gm_de440.tpc"
OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data"

# Julian Date (TDB) -> (file name, human-readable epoch)
EPOCHS = {
    2460676.5: ("solar-system-2025-01-01.csv", "2025-01-01 00:00:00 TDB"),
    2461041.75: ("solar-system-2026-01-01T06.csv", "2026-01-01 06:00:00 TDB"),
}

# (name, NAIF/Horizons id, mean radius in km)
# Mars, Jupiter, Saturn, Uranus, Neptune and Pluto are their system
# barycenters (planet plus moons), as in JPL's own ephemerides.
BODIES = [
    ("Sun", 10, 695700.0),
    ("Mercury", 199, 2439.4),
    ("Venus", 299, 6051.8),
    ("Earth", 399, 6371.0084),
    ("Moon", 301, 1737.4),
    ("Mars", 4, 3389.5),
    ("Jupiter", 5, 69911.0),
    ("Saturn", 6, 58232.0),
    ("Uranus", 7, 25362.0),
    ("Neptune", 8, 24622.0),
    ("Pluto", 9, 1188.3),
]


def fetch(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read().decode()


def gm_values():
    """GM for each NAIF id, in km^3/s^2, as the exact strings JPL publishes."""
    text = fetch(GM_FILE)
    pattern = r"BODY(\d+)_GM\s*=\s*\(\s*([-+0-9.EeDd]+)\s*\)"
    return {int(m.group(1)): m.group(2).replace("D", "E") for m in re.finditer(pattern, text)}


def states(naif_id):
    """Barycentric state vectors at each epoch, as the exact strings Horizons returns."""
    params = {
        "format": "json",
        "COMMAND": f"'{naif_id}'",
        "OBJ_DATA": "'NO'",
        "MAKE_EPHEM": "'YES'",
        "EPHEM_TYPE": "'VECTORS'",
        "CENTER": "'500@0'",
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
    source = re.search(r"Target body name: .*\{source: (\w+)\}", result).group(1)
    block = result.split("$$SOE")[1].split("$$EOE")[0]
    rows = {}
    for line in block.strip().splitlines():
        fields = [f.strip() for f in line.split(",")]
        rows[float(fields[0])] = fields[2:8]
    return source, rows


def main():
    gms = gm_values()
    fetched = {}
    sources = set()
    for name, naif_id, _ in BODIES:
        print(f"Fetching {name}...")
        source, rows = states(naif_id)
        sources.add(source)
        fetched[naif_id] = rows

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for jd, (file_name, label) in EPOCHS.items():
        lines = [
            "# Worldline solar-system snapshot. Regenerate with: python tools/fetch_solar_system.py",
            f"# epoch_jd_tdb: {jd}",
            f"# epoch: {label}",
            f"# states: NASA JPL Horizons, ephemeris {', '.join(sorted(sources))}, fetched {date.today()}",
            "# frame: ICRF, ecliptic and mean equinox of J2000, origin at the solar system barycenter",
            "# units: km, km/s, km^3/s^2",
            "# gm: JPL DE440 (NAIF gm_de440.tpc)",
            "# radii: mean radii, IAU WGCCRE (Archinal et al. 2018); Sun: IAU 2015 nominal",
            "# note: Mars, Jupiter, Saturn, Uranus, Neptune and Pluto are system barycenters (planet plus moons)",
            "name,naif_id,gm_km3_s2,radius_km,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s",
        ]
        for name, naif_id, radius in BODIES:
            state = fetched[naif_id][jd]
            lines.append(",".join([name, str(naif_id), gms[naif_id], str(radius), *state]))
        path = OUT_DIR / file_name
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"Wrote {path}")


if __name__ == "__main__":
    main()
