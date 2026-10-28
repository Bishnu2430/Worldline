"""Fetch where NASA JPL Horizons puts the Sun, planets, Moon and Pluto on
the day step 5.1 was built, to check the simulation run from its 2025
snapshot up to that day.

Positions and velocities from Horizons (ephemeris DE441), of the planets'
own centers, not their systems' barycenters: the app simulates their moons
and shows the planets themselves. GM from JPL's DE440 constants file.

Also where the Sun is overhead on Earth on 2025-01-01 and on that day, by
JPL's own Earth orientation (IERS): the Sun's geocentric apparent right
ascension and declination, and Greenwich apparent sidereal time.

Writes planets-2026-10-28.csv and sun-overhead.csv under
crates/worldline-data/data/.

Run from anywhere:  python tools/fetch_today.py
Uses only the Python standard library.
"""

import json
import re
import urllib.parse
from datetime import date

from fetch_solar_system import HORIZONS, OUT_DIR, fetch, gm_values

EPOCH_JD = 2461341.5
EPOCH = "2026-10-28 00:00:00 TDB"
FILE = "planets-2026-10-28.csv"

# (name, NAIF/Horizons id of the body's own center, mean radius in km)
BODIES = [
    ("Sun", 10, 695700.0),
    ("Mercury", 199, 2439.4),
    ("Venus", 299, 6051.8),
    ("Earth", 399, 6371.0084),
    ("Moon", 301, 1737.4),
    ("Mars", 499, 3389.5),
    ("Jupiter", 599, 69911.0),
    ("Saturn", 699, 58232.0),
    ("Uranus", 799, 25362.0),
    ("Neptune", 899, 24622.0),
    ("Pluto", 999, 1188.3),
]


def state(naif_id):
    """The barycentric state vector at the epoch, as the exact strings Horizons returns."""
    params = {
        "format": "json",
        "COMMAND": f"'{naif_id}'",
        "OBJ_DATA": "'NO'",
        "MAKE_EPHEM": "'YES'",
        "EPHEM_TYPE": "'VECTORS'",
        "CENTER": "'500@0'",
        "TLIST": f"'{EPOCH_JD}'",
        "TIME_TYPE": "'TDB'",
        "REF_PLANE": "'ECLIPTIC'",
        "REF_SYSTEM": "'ICRF'",
        "OUT_UNITS": "'KM-S'",
        "VEC_TABLE": "'2'",
        "CSV_FORMAT": "'YES'",
    }
    result = json.loads(fetch(HORIZONS + "?" + urllib.parse.urlencode(params)))["result"]
    source = re.search(r"Target body name: .*\{source: (\w+)\}", result).group(1)
    line = result.split("$$SOE")[1].split("$$EOE")[0].strip().splitlines()[0]
    fields = [f.strip() for f in line.split(",")]
    assert float(fields[0]) == EPOCH_JD, fields[0]
    return source, fields[2:8]


# The epochs (JD, TT) of the overhead Sun: the 2025 snapshot, and that day.
OVERHEAD = [2460676.5, 2461341.5]


def observer(center, quantities):
    """Horizons observer table rows for the Sun at the OVERHEAD epochs."""
    params = {
        "format": "json",
        "COMMAND": "'10'",
        "OBJ_DATA": "'NO'",
        "MAKE_EPHEM": "'YES'",
        "EPHEM_TYPE": "'OBSERVER'",
        "CENTER": f"'{center}'",
        "TLIST": " ".join(f"'{jd}'" for jd in OVERHEAD),
        "TIME_TYPE": "'TT'",
        "QUANTITIES": f"'{quantities}'",
        "ANG_FORMAT": "'DEG'",
        "EXTRA_PREC": "'YES'",
        "CSV_FORMAT": "'YES'",
    }
    result = json.loads(fetch(HORIZONS + "?" + urllib.parse.urlencode(params)))["result"]
    block = result.split("$$SOE")[1].split("$$EOE")[0]
    return [[f.strip() for f in line.split(",")] for line in block.strip().splitlines()]


def overhead():
    # Geocentric apparent RA and Dec; Greenwich (site 000, longitude 0)
    # local apparent sidereal time, which is Greenwich's.
    geocentric = observer("500@399", "2")
    greenwich = observer("000@399", "7")
    lines = [
        "# Where the Sun is overhead on Earth, by NASA JPL Horizons (Earth orientation from the IERS). Regenerate with: python tools/fetch_today.py",
        f"# fetched {date.today()}",
        "# sun_ra_deg, sun_dec_deg: the Sun's geocentric apparent right ascension and declination, of date",
        "# gast_hours: Greenwich apparent sidereal time (local apparent sidereal time at site 000, Greenwich)",
        "epoch_jd_tt,sun_ra_deg,sun_dec_deg,gast_hours",
    ]
    for jd, sun, site in zip(OVERHEAD, geocentric, greenwich):
        lines.append(",".join([str(jd), sun[3], sun[4], site[3]]))
    path = OUT_DIR / "sun-overhead.csv"
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines) + "\n")
    print(f"Wrote {path}")


def main():
    overhead()
    gms = gm_values()
    lines, sources = [], set()
    for name, naif_id, radius in BODIES:
        print(f"Fetching {name}...")
        source, vector = state(naif_id)
        sources.add(source)
        lines.append(",".join([name, str(naif_id), gms[naif_id], str(radius), *vector]))
    header = [
        "# Where JPL puts the Sun, planets, Moon and Pluto on the day step 5.1 was built. Regenerate with: python tools/fetch_today.py",
        f"# epoch_jd_tdb: {EPOCH_JD}",
        f"# epoch: {EPOCH}",
        f"# states: NASA JPL Horizons, ephemerides {', '.join(sorted(sources))}, fetched {date.today()}",
        "# frame: ICRF, ecliptic and mean equinox of J2000, origin at the solar system barycenter",
        "# units: km, km/s, km^3/s^2",
        "# gm: JPL DE440 (NAIF gm_de440.tpc)",
        "# radii: mean radii, IAU WGCCRE (Archinal et al. 2018); Sun: IAU 2015 nominal",
        "# note: every planet is its own center, not its system's barycenter",
        "name,naif_id,gm_km3_s2,radius_km,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s",
    ]
    path = OUT_DIR / FILE
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(header + lines) + "\n")
    print(f"Wrote {path}")


if __name__ == "__main__":
    main()
