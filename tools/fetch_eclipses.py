"""Fetch NASA's eclipses of 2026-2030, the reference for Worldline's
eclipse finder.

From the catalogs of the Five Millennium Canons of Solar and Lunar Eclipses
(Espenak & Meeus 2006, 2009), on NASA's eclipse web site: each eclipse's
date, time of greatest eclipse (TD), Delta T, Saros series, type, gamma
and magnitudes.

Writes solar-eclipses-2026-2030.csv and lunar-eclipses-2026-2030.csv under
crates/worldline-data/data/.

Run from anywhere:  python tools/fetch_eclipses.py
Uses only the Python standard library.
"""

import re
import urllib.request
from datetime import date
from pathlib import Path

SOLAR = "https://eclipse.gsfc.nasa.gov/SEcat5/SE2001-2100.html"
LUNAR = "https://eclipse.gsfc.nasa.gov/LEcat5/LE2001-2100.html"
OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data"
YEARS = range(2026, 2031)
MONTHS = {m: i + 1 for i, m in enumerate(
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"])}


def rows(url):
    """Each catalog row of the years wanted, as its whitespace-separated fields."""
    with urllib.request.urlopen(url, timeout=120) as response:
        text = response.read().decode(errors="replace")
    for line in text.splitlines():
        fields = re.sub(r"<[^>]+>", " ", line).split()
        if len(fields) > 8 and fields[0].isdigit() and fields[1].isdigit() and int(fields[1]) in YEARS:
            yield fields


def iso(fields):
    return f"{fields[1]}-{MONTHS[fields[2]]:02d}-{int(fields[3]):02d}"


def write(name, header, lines):
    path = OUT_DIR / name
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(header + lines) + "\n")
    print(f"Wrote {path}: {len(lines)} eclipses")


def main():
    fetched = f"fetched {date.today()}"
    # Solar: number, date (3), TD, Delta T, lunation, Saros, type, QLE,
    # gamma, magnitude, ...
    solar = [
        ",".join([iso(f), f[4], f[5], f[7], f[8], f[10], f[11]])
        for f in rows(SOLAR)
    ]
    write(
        "solar-eclipses-2026-2030.csv",
        [
            "# NASA's solar eclipses of 2026-2030. Regenerate with: python tools/fetch_eclipses.py",
            f"# source: Five Millennium Canon of Solar Eclipses (Espenak & Meeus 2006), catalog {SOLAR}; {fetched}",
            "# td: time of greatest eclipse, Terrestrial Dynamical Time; type: T total, A annular, H hybrid, P partial, then qualifiers",
            "# gamma: the shadow axis's least distance from Earth's center, Earth equatorial radii, + north; magnitude: of greatest eclipse",
            "date,td,delta_t_s,saros,type,gamma,magnitude",
        ],
        solar,
    )
    # Lunar: number, date (3), TD, Delta T, lunation, Saros, type, QSE,
    # gamma, penumbral magnitude, umbral magnitude, ...
    lunar = [
        ",".join([iso(f), f[4], f[5], f[7], f[8], f[10], f[11], f[12]])
        for f in rows(LUNAR)
    ]
    write(
        "lunar-eclipses-2026-2030.csv",
        [
            "# NASA's lunar eclipses of 2026-2030. Regenerate with: python tools/fetch_eclipses.py",
            f"# source: Five Millennium Canon of Lunar Eclipses (Espenak & Meeus 2009), catalog {LUNAR}; {fetched}",
            "# td: time of greatest eclipse, Terrestrial Dynamical Time; type: T total, P partial, N penumbral, then qualifiers",
            "# gamma: the Moon's least distance from the umbra's axis, Earth equatorial radii, + north; magnitudes: of greatest eclipse",
            "date,td,delta_t_s,saros,type,gamma,penumbral_magnitude,umbral_magnitude",
        ],
        lunar,
    )


if __name__ == "__main__":
    main()
