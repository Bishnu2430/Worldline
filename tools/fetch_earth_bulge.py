"""Fetch Earth's zonal gravity harmonics as JPL's planetary ephemeris DE440
integrates them: the bulge that pulls on the Moon, which Worldline
simulates beside Earth at the top level.

From the DE440 header (header.440, constants groups 1040 and 1041): RE,
Earth's reference radius in km, and J2E to J5E, unnormalized.

Writes earth-zonal-harmonics.csv under crates/worldline-data/data/.

Run from anywhere:  python tools/fetch_earth_bulge.py
Uses only the Python standard library.
"""

import urllib.request
from datetime import date
from pathlib import Path

HEADER = "https://ssd.jpl.nasa.gov/ftp/eph/planets/ascii/de440/header.440"
OUT = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data" / "earth-zonal-harmonics.csv"


def constants():
    """DE440's named constants, as the exact strings the header gives (D exponents as E)."""
    with urllib.request.urlopen(HEADER, timeout=120) as response:
        text = response.read().decode()
    names = text.split("GROUP   1040")[1].split("GROUP   1041")[0].split()
    values = text.split("GROUP   1041")[1].split("GROUP   1050")[0].split()
    count = int(names[0])
    assert int(values[0]) == count
    return dict(zip(names[1:1 + count], (v.replace("D", "E") for v in values[1:1 + count])))


def main():
    found = constants()
    lines = [
        "# Earth's zonal gravity harmonics (unnormalized J_n) and their reference radius, as JPL's DE440 integrates them.",
        "# Regenerate with: python tools/fetch_earth_bulge.py",
        f"# source: the DE440 header, constants RE and J2E to J5E ({HEADER}); fetched {date.today()}",
        "reference_radius_km,degree,j",
    ]
    for degree in range(2, 6):
        lines.append(f"{found['RE']},{degree},{found[f'J{degree}E']}")
    with open(OUT, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines) + "\n")
    print(f"Wrote {OUT}")


if __name__ == "__main__":
    main()
