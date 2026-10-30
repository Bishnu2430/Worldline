"""Fetch the references for the sky from a place on Earth (step 5.4).

- Rise and set of the Sun and Moon in 2026, in Universal Time, for two
  cities in opposite hemispheres: the U.S. Naval Observatory's one-year
  tables, as published (to the minute). The tables are for places given to
  the minute of arc, so these cities are asked for exactly that.
- Where the Sun, Moon and planets stand from the same cities at a few
  moments of 2026: NASA JPL Horizons' apparent azimuth and elevation, without
  refraction ("airless"), from its observer tables.

Writes rise-set-2026.csv and sky-from-earth-2026.csv under
crates/worldline-data/data/.

Run from anywhere:  python tools/fetch_sky.py
Uses only the Python standard library.
"""

import json
import re
import urllib.parse
import urllib.request
from datetime import date
from pathlib import Path

USNO = "https://aa.usno.navy.mil/calculated/rstt/year"
DEFINITIONS = "https://aa.usno.navy.mil/faq/RST_defs"
HORIZONS = "https://ssd.jpl.nasa.gov/api/horizons.api"
OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data"
YEAR = 2026

# (key, name, latitude and longitude as degrees and minutes, signed).
SITES = [
    ("washington", "Washington, DC", (38, 53), (-77, 2)),
    ("sydney", "Sydney", (-33, 52), (151, 12)),
]
TASKS = [("Sun", 0), ("Moon", 1)]
MONTH_DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]

# Moments of 2026 for the azimuths and elevations: noon UT on the first of
# every third month, Julian Dates (UT).
MOMENTS = [2461042.0, 2461132.0, 2461223.0, 2461315.0]
BODIES = [
    ("Sun", 10),
    ("Moon", 301),
    ("Mercury", 199),
    ("Venus", 299),
    ("Mars", 499),
    ("Jupiter", 599),
    ("Saturn", 699),
    ("Uranus", 799),
    ("Neptune", 899),
]


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "Worldline tools/fetch_sky.py"})
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read().decode(errors="replace")


def degrees(pair):
    d, m = pair
    return (abs(d) + m / 60.0) * (1 if d >= 0 else -1)


def rows_of(key, name, lat, lon, body, task):
    """The year table's events: columns are fixed, 11 wide, one pair a month."""
    params = {
        "ID": "AA",
        "year": YEAR,
        "task": task,
        "lat": f"{degrees(lat):.6f}",
        "lon": f"{degrees(lon):.6f}",
        "label": name,
        "tz": 0,
        "tz_sign": -1,
        "submit": "Get Data",
    }
    text = fetch(USNO + "?" + urllib.parse.urlencode(params))
    table = text.split("<pre")[1].split("</pre>")[0]
    expected = f"{'N' if lat[0] >= 0 else 'S'}{abs(lat[0]):02d} {lat[1]:02d}"
    assert expected in table, (expected, table[:300])
    assert "Universal Time" in table
    rows = []
    for line in table.splitlines():
        if not re.match(r"\d\d  ", line):
            continue
        day = int(line[:2])
        line = line.ljust(4 + 11 * 12)
        for month in range(12):
            if day > MONTH_DAYS[month]:
                continue
            start = 4 + 11 * month
            for event, field in (("rise", line[start:start + 4]), ("set", line[start + 5:start + 9])):
                if field.strip():
                    assert re.fullmatch(r"\d{4}", field), (line, field)
                    rows.append(f"{key},{body},{event},{YEAR}-{month + 1:02d}-{day:02d},{field}")
    return rows


def rise_set():
    rows = []
    for key, name, lat, lon in SITES:
        for body, task in TASKS:
            found = rows_of(key, name, lat, lon, body, task)
            print(f"{name}, {body}: {len(found)} events")
            rows += found
    header = [
        "# The U.S. Naval Observatory's rise and set of the Sun and Moon in 2026, in Universal Time, to the minute. Regenerate with: python tools/fetch_sky.py",
        f"# source: USNO Astronomical Applications Department, 'Sun or Moon Rise/Set Table for One Year' ({USNO}); fetched {date.today()}",
        f"# definitions: {DEFINITIONS}: the Sun's center 90.8333 deg from the zenith; the Moon's 90.5666 deg + its apparent radius - its horizontal parallax",
        "# time: hhmm, Universal Time, on the UT date given; a day without an event has no row",
    ]
    for key, name, lat, lon in SITES:
        header.append(f"# site: {key}; {name}; latitude {lat[0]} {lat[1]}; longitude {lon[0]} {lon[1]}")
    write("rise-set-2026.csv", header + ["site,body,event,date,time_ut"] + rows)


def horizons(key, lat, lon, body, naif_id):
    params = {
        "format": "json",
        "COMMAND": f"'{naif_id}'",
        "OBJ_DATA": "'NO'",
        "MAKE_EPHEM": "'YES'",
        "EPHEM_TYPE": "'OBSERVER'",
        "CENTER": "'coord@399'",
        "COORD_TYPE": "'GEODETIC'",
        "SITE_COORD": f"'{degrees(lon):.8f},{degrees(lat):.8f},0'",
        "TLIST": " ".join(f"'{jd}'" for jd in MOMENTS),
        "TIME_TYPE": "'UT'",
        "QUANTITIES": "'4'",
        "ANG_FORMAT": "'DEG'",
        "APPARENT": "'AIRLESS'",
        "EXTRA_PREC": "'YES'",
        "CSV_FORMAT": "'YES'",
    }
    result = json.loads(fetch(HORIZONS + "?" + urllib.parse.urlencode(params)))["result"]
    block = result.split("$$SOE")[1].split("$$EOE")[0].strip().splitlines()
    assert len(block) == len(MOMENTS), result[:500]
    source = re.search(r"Target body name: .*\{source: (\w+)\}", result).group(1)
    rows = []
    for jd, line in zip(MOMENTS, block):
        f = [x.strip() for x in line.split(",")]
        rows.append(f"{key},{jd},{body},{f[3]},{f[4]}")
    return source, rows


def sky():
    rows, sources = [], set()
    for key, name, lat, lon in SITES:
        for body, naif_id in BODIES:
            print(f"{name}, {body}...")
            source, found = horizons(key, lat, lon, body, naif_id)
            sources.add(source)
            rows += found
    write(
        "sky-from-earth-2026.csv",
        [
            "# Where the Sun, Moon and planets stand from two cities, by NASA JPL Horizons. Regenerate with: python tools/fetch_sky.py",
            f"# source: Horizons observer tables, quantity 4, ephemerides {', '.join(sorted(sources))}; fetched {date.today()}",
            "# azimuth_deg, elevation_deg: apparent, topocentric, airless (no refraction); azimuth from north through east",
            "# jd_ut: Julian Date, UT; the sites as in rise-set-2026.csv, at the ellipsoid's surface",
            "site,jd_ut,body,azimuth_deg,elevation_deg",
        ]
        + rows,
    )


def write(name, lines):
    path = OUT_DIR / name
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines) + "\n")
    print(f"Wrote {path}: {len(lines)} lines")


def main():
    rise_set()
    sky()


if __name__ == "__main__":
    main()
