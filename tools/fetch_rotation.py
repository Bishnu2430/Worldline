"""Fetch the rotation models and shapes Worldline ships with.

Downloads NASA NAIF's planetary constants kernel pck00011.tpc, which holds
the IAU Working Group on Cartographic Coordinates and Rotational Elements
models (Archinal et al. 2018), and copies out, character for character,
the entries for the bodies Worldline simulates: pole directions, spin and
triaxial radii. Writes them as a valid SPICE text kernel at
crates/worldline-data/data/pck00011-subset.tpc.

Run from anywhere:  python tools/fetch_rotation.py
Uses only the Python standard library.
"""

import re
import urllib.request
from datetime import date
from pathlib import Path

KERNEL = "https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/pck00011.tpc"
OUT = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data" / "pck00011-subset.tpc"

# The Sun, Mercury, Venus, Earth and the Moon, every planet, dwarf planet
# and moon of the Mars-to-Pluto systems that the kernel describes (NAIF ids
# 401-999), and the asteroids and comets it describes (seven-digit ids).
BODIES = "10|199|299|399|301|[4-9][0-9][0-9]|[129][0-9]{6}"
WANTED = re.compile(
    rf"^BODY({BODIES})_(POLE_RA|POLE_DEC|PM|NUT_PREC_RA|NUT_PREC_DEC|NUT_PREC_PM|RADII)$"
    r"|^BODY[1-9]_(NUT_PREC_ANGLES|MAX_PHASE_DEGREE)$"
)


def data_statements(text):
    """Yield (name, exact source text) for every assignment in \\begindata blocks."""
    in_data = False
    statement = []
    depth = 0
    for line in text.splitlines():
        stripped = line.strip()
        if stripped == "\\begindata":
            in_data = True
            continue
        if stripped == "\\begintext":
            in_data = False
            continue
        if not in_data:
            continue
        if not statement and "=" not in line:
            continue
        statement.append(line.rstrip())
        depth += line.count("(") - line.count(")")
        if depth == 0:
            joined = "\n".join(statement)
            yield joined.split("=")[0].strip(), joined
            statement = []


def main():
    with urllib.request.urlopen(KERNEL, timeout=60) as response:
        text = response.read().decode()
    kept = [source for name, source in data_statements(text) if WANTED.match(name)]
    header = f"""Worldline rotation models and shapes. Regenerate with: python tools/fetch_rotation.py

Source:  NASA NAIF generic kernel pck00011.tpc, fetched {date.today()}
         {KERNEL}
Models:  IAU WGCCRE (Archinal et al. 2018, Celest. Mech. Dyn. Astr. 130, 22)
Content: the entries below are copied character for character from the
         kernel's data blocks, for the Sun, the planets, the Moon, Pluto,
         the moons of Mars through Pluto, and asteroids and comets.
Units:   pole RA and Dec in degrees with time in Julian centuries (TDB)
         since J2000; prime meridian in degrees with time in days (TDB)
         since J2000; reference frame J2000 (equatorial); RADII in km,
         as (equatorial a, equatorial b, polar c).
"""
    body = "\n\n".join(kept)
    with open(OUT, "w", encoding="utf-8", newline="\n") as file:
        file.write(f"{header}\n\\begindata\n\n{body}\n\n\\begintext\n")
    print(f"Wrote {len(kept)} entries to {OUT}")


if __name__ == "__main__":
    main()
