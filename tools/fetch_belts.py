"""Fetch the asteroid belt, Jupiter's Trojans and the Kuiper belt.

Writes, under crates/worldline-data/data/, the osculating orbital elements
JPL's Small-Body Database (SBDB) publishes for:

- asteroid-belt.csv: every main-belt asteroid (SBDB classes IMB, MBA and
  OMB: inner, middle and outer belt, including the Hildas and Cybeles)
  brighter than absolute magnitude H = 14, roughly larger than 5 to 10 km
  across. Surveys have found essentially all asteroids this bright, so the
  sample isn't biased by what is easy to see.
- jupiter-trojans.csv: every Jupiter Trojan (class TJN) brighter than
  H = 14.
- kuiper-belt.csv: every trans-Neptunian object (class TNO) with a
  well-determined orbit (JPL orbit condition code 5 or better).

The 62 bodies Worldline simulates individually (small-bodies-2025-01-01.csv)
are left out, so nothing is drawn twice.

Elements are heliocentric, in the ecliptic and mean equinox of J2000, at
each orbit's own epoch, in SBDB's full precision.

Run from anywhere:  python tools/fetch_belts.py
Uses only the Python standard library.
"""

import json
import urllib.parse
from datetime import date

from fetch_moons import DATA, fetch, write

SBDB_QUERY = "https://ssd-api.jpl.nasa.gov/sbdb_query.api"
FIELDS = ["pdes", "a", "e", "i", "om", "w", "ma", "epoch"]

# (file, description, query parameters)
BELTS = [
    ("asteroid-belt.csv",
     "main-belt asteroids (SBDB classes IMB, MBA, OMB) with absolute magnitude H < 14",
     {"sb-class": "IMB,MBA,OMB", "sb-cdata": json.dumps({"AND": ["H|LT|14"]}, separators=(",", ":"))}),
    ("jupiter-trojans.csv",
     "Jupiter Trojans (SBDB class TJN) with absolute magnitude H < 14",
     {"sb-class": "TJN", "sb-cdata": json.dumps({"AND": ["H|LT|14"]}, separators=(",", ":"))}),
    ("kuiper-belt.csv",
     "trans-Neptunian objects (SBDB class TNO) with orbit condition code 5 or better",
     {"sb-class": "TNO", "sb-cdata": json.dumps({"AND": ["condition_code|LE|5"]}, separators=(",", ":"))}),
]


def simulated_numbers():
    """Asteroid numbers of the bodies Worldline simulates individually
    (NAIF id 2,000,000 + number)."""
    numbers = set()
    with open(DATA / "small-bodies-2025-01-01.csv", encoding="utf-8") as file:
        rows = [line for line in file if not line.startswith("#")][1:]
    for row in rows:
        naif_id = int(row.split(",")[2])
        if 2_000_000 < naif_id < 3_000_000:
            numbers.add(str(naif_id - 2_000_000))
    return numbers


def main():
    skip = simulated_numbers()
    for name, description, query in BELTS:
        params = {"fields": ",".join(FIELDS), "full-prec": "true", **query}
        url = SBDB_QUERY + "?" + urllib.parse.urlencode(params)
        reply = json.loads(fetch(url))
        assert reply["fields"] == FIELDS, reply["fields"]
        rows = [r for r in reply["data"] if r[0] not in skip]
        if any(None in r or "" in r for r in rows):
            raise SystemExit(f"{name}: SBDB returned a row with a missing element")
        lines = [
            f"# Worldline {description}. Regenerate with: python tools/fetch_belts.py",
            f"# source: NASA JPL Small-Body Database Query API ({SBDB_QUERY}), fetched {date.today()}",
            f"# query: {urllib.parse.unquote(urllib.parse.urlencode(params))}",
            f"# count: {len(rows)} (SBDB matched {reply['count']}; the bodies simulated individually,"
            " listed in small-bodies-2025-01-01.csv, are left out)",
            "# elements: osculating, heliocentric, ecliptic and mean equinox of J2000, at each orbit's epoch",
            "# units: a in au; angles in degrees; epoch as a Julian Date (TDB)",
            "pdes,a_au,e,i_deg,node_deg,periapsis_deg,mean_anomaly_deg,epoch_jd_tdb",
        ]
        lines += [",".join(r) for r in rows]
        write(DATA / name, lines)
        print(f"  {len(rows)} orbits")


if __name__ == "__main__":
    main()
