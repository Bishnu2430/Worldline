"""Fetch where Voyager 1 and 2 crossed the heliosphere's boundaries.

Writes crates/worldline-data/data/voyager-crossings.csv: for each of the
four crossings, the date and distance the Voyager teams published, and the
spacecraft's position relative to the Sun at noon (TDB) that day from NASA
JPL Horizons, in the ecliptic and equinox of J2000. The header also records
the direction the interstellar wind comes from (the heliosphere's nose),
measured by IBEX.

Run from anywhere:  python tools/fetch_heliosphere.py
Uses only the Python standard library.
"""

import json
import urllib.parse
from datetime import date

from fetch_moons import DATA, HORIZONS, fetch, write

# (spacecraft, Horizons id, boundary, date, published distance in AU, source)
CROSSINGS = [
    ("Voyager 1", -31, "termination shock", "2004-12-16", "94.01",
     "Stone et al. 2005, Science 309, 2017"),
    ("Voyager 2", -32, "termination shock", "2007-08-31", "83.7",
     "Stone et al. 2008, Nature 454, 71"),
    ("Voyager 1", -31, "heliopause", "2012-08-25", "121.6",
     "Stone et al. 2013, Science 341, 150"),
    ("Voyager 2", -32, "heliopause", "2018-11-05", "119.0",
     "Stone et al. 2019, Nature Astronomy 3, 1013"),
]

# Where the interstellar wind comes from: ecliptic longitude and latitude
# (J2000), from the flow of interstellar helium through the solar system.
NOSE = ("255.8", "5.16", "Bzowski et al. 2015, ApJS 220, 28 (IBEX)")


def position(spacecraft_id, day):
    """Heliocentric position (AU) at noon TDB on `day`, and its source."""
    params = {
        "format": "json",
        "COMMAND": f"'{spacecraft_id}'",
        "OBJ_DATA": "'NO'",
        "MAKE_EPHEM": "'YES'",
        "EPHEM_TYPE": "'VECTORS'",
        "CENTER": "'500@10'",
        "TLIST": f"'{day} 12:00'",
        "TIME_TYPE": "'TDB'",
        "REF_PLANE": "'ECLIPTIC'",
        "REF_SYSTEM": "'ICRF'",
        "OUT_UNITS": "'AU-D'",
        "VEC_TABLE": "'1'",
        "CSV_FORMAT": "'YES'",
    }
    result = json.loads(fetch(HORIZONS + "?" + urllib.parse.urlencode(params)))["result"]
    block = result.split("$$SOE")[1].split("$$EOE")[0]
    fields = [f.strip() for f in block.strip().split(",")]
    return fields[2:5]


def main():
    lines = [
        "# Worldline: where Voyager 1 and 2 crossed the heliosphere's boundaries."
        " Regenerate with: python tools/fetch_heliosphere.py",
        "# dates and distances: as published by the Voyager teams (source column)",
        f"# positions: NASA JPL Horizons, relative to the Sun's center at 12:00 TDB that day,"
        f" ecliptic and equinox of J2000, in AU; fetched {date.today()}",
        f"# nose_ecliptic_lon_lat_deg: {NOSE[0]}, {NOSE[1]} (where the interstellar wind comes from; {NOSE[2]})",
        "spacecraft,boundary,date,published_distance_au,x_au,y_au,z_au,source",
    ]
    for name, naif, boundary, day, distance, source in CROSSINGS:
        x, y, z = position(naif, day)
        lines.append(",".join([name, boundary, day, distance, x, y, z, source]))
        r = sum(float(v) ** 2 for v in (x, y, z)) ** 0.5
        print(f"{name} {boundary} {day}: Horizons {r:.3f} AU, published {distance} AU")
    write(DATA / "voyager-crossings.csv", lines)


if __name__ == "__main__":
    main()
