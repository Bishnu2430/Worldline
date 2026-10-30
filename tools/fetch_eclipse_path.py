"""Fetch NASA's path of the total solar eclipse of 2026 August 12: the
reference for the Moon's shadow on Earth (step 5.3).

From Fred Espenak's path table on NASA's eclipse web site: every 2 minutes
(UT), the geographic coordinates (WGS 84) of the umbra's northern and
southern limits and of the central line, as published (degrees and
minutes). The table states the Delta T it assumed, which turns its UT back
into the dynamical time the eclipse was computed in. Also, from the
eclipse's Besselian elements page, the elements at their reference time
t0: x, y, l1, l2 (the polynomials' constant terms), tan f1 and tan f2.

Writes eclipse-path-2026-08-12.csv under crates/worldline-data/data/.

Run from anywhere:  python tools/fetch_eclipse_path.py
Uses only the Python standard library.
"""

import re
import urllib.request
from datetime import date
from pathlib import Path

URL = "https://eclipse.gsfc.nasa.gov/SEpath/SEpath2001/SE2026Aug12Tpath.html"
ELEMENTS = "https://eclipse.gsfc.nasa.gov/SEbeselm/SEbeselm2001/SE2026Aug12Tbeselm.html"
OUT = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data" / "eclipse-path-2026-08-12.csv"

# A coordinate as published: degrees, minutes and a hemisphere ("65 10.3N",
# "025 12.3W"), or "-" where the limit isn't on Earth.
COORD = r"(\d+ \d+\.\dN|\d+ \d+\.\dS|\d+ \d+\.\dE|\d+ \d+\.\dW|-)"
ROW = re.compile(r"^\s*(\d\d:\d\d)\s+" + r"\s+".join([COORD + r"\s+" + COORD] * 3) + r"\s")


def page(url):
    with urllib.request.urlopen(url, timeout=120) as response:
        return re.sub(r"<[^>]+>", "", response.read().decode(errors="replace"))


def elements():
    """The Besselian elements at t0, as the exact strings published."""
    text = page(ELEMENTS)
    t0 = re.search(r"([\d:.]+) TDT\s+\(=t0\)", text).group(1)
    row = re.search(r"^\s+0\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)", text, re.M).groups()
    tan_f1 = re.search(r"Tan &#402;1 = (\S+)", text).group(1)
    tan_f2 = re.search(r"Tan &#402;2 = (\S+)", text).group(1)
    x, y, _d, l1, l2, _mu = row
    return f"# besselian: t0 {t0} TDT; x {x}; y {y}; l1 {l1}; l2 {l2}; tan_f1 {tan_f1}; tan_f2 {tan_f2} ({ELEMENTS})"


def main():
    text = page(URL)
    delta_t = re.search(r"&Delta;T =\s*([\d.]+)\s*seconds", text).group(1)
    ephemerides = re.search(r"generated using the\s*(?:\S+\s*-->\s*)?(\S+)\s*solar and lunar", text).group(1)
    rows = []
    for line in text.splitlines():
        m = ROW.match(line)
        if m:
            fields = ["" if f == "-" else f for f in m.groups()[1:]]
            rows.append(",".join([m.group(1)] + fields))
    lines = [
        "# NASA's path of the total solar eclipse of 2026 August 12. Regenerate with: python tools/fetch_eclipse_path.py",
        f"# source: F. Espenak, NASA GSFC, path table {URL}; fetched {date.today()}",
        "# \"Eclipse Predictions by Fred Espenak, NASA's GSFC\"",
        f"# delta_t_s: {delta_t} (the table's UT is dynamical time less this); ephemerides: {ephemerides}",
        elements(),
        "# coordinates: WGS 84, as published: degrees, minutes and hemisphere; empty where the limit isn't on Earth",
        "ut,north_lat,north_lon,south_lat,south_lon,central_lat,central_lon",
    ]
    with open(OUT, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines + rows) + "\n")
    print(f"Wrote {OUT}: {len(rows)} rows, delta T {delta_t} s")


if __name__ == "__main__":
    main()
