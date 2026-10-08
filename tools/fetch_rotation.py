"""Fetch the rotation models and shapes Worldline ships with.

Downloads NASA NAIF's planetary constants kernel pck00011.tpc, which holds
the IAU Working Group on Cartographic Coordinates and Rotational Elements
models (Archinal et al. 2018), and copies out, character for character,
the entries for the bodies Worldline simulates: pole directions, spin and
triaxial radii. Writes them as a valid SPICE text kernel at
crates/worldline-data/data/pck00011-subset.tpc.

Also copies Pluto's and Charon's orientation from the New Horizons team's
kernel nh_pcnh_010.tpc (NASA PDS), which corrects a 1.45 degree error in
pck00011's prime meridians for the two, into nh_pcnh_010-subset.tpc. Like
SPICE, Worldline loads it after pck00011, so its values take precedence.

Run from anywhere:  python tools/fetch_rotation.py
Uses only the Python standard library.
"""

import re
import urllib.request
from datetime import date
from pathlib import Path

KERNEL = "https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/pck00011.tpc"
DATA = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data"
OUT = DATA / "pck00011-subset.tpc"
NH_KERNEL = "https://naif.jpl.nasa.gov/pub/naif/pds/data/nh-j_p_ss-spice-6-v1.0/nhsp_1000/data/pck/nh_pcnh_010.tpc"
NH_OUT = DATA / "nh_pcnh_010-subset.tpc"
NH_WANTED = re.compile(r"^BODY(999|901)_(POLE_RA|POLE_DEC|PM)$")

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

    with urllib.request.urlopen(NH_KERNEL, timeout=60) as response:
        text = response.read().decode()
    kept = [source for name, source in data_statements(text) if NH_WANTED.match(name)]
    assert len(kept) == 6
    header = f"""Pluto's and Charon's orientation from the New Horizons team. Regenerate with: python tools/fetch_rotation.py

Source:  New Horizons SPICE kernel nh_pcnh_010.tpc (NASA Planetary Data
         System, data set NH-J/P/SS-SPICE-6-V1.0), fetched {date.today()}
         {NH_KERNEL}
Why:     the IAU defines Pluto's prime meridian as the mean sub-Charon
         meridian and Charon's as the mean sub-Pluto meridian. pck00011's
         values, unchanged since at least 1994, miss that by 1.45 degrees
         (as of the 2015 flyby). These are fitted to the PLU055 ephemeris
         and keep each body's meridian on the other to under 0.02 degrees
         from 1950 to 2050, according to the kernel's own notes.
Use:     load after pck00011-subset.tpc; these entries replace its.
Content: the entries below are copied character for character from the
         kernel's data blocks. Units as in pck00011-subset.tpc.
"""
    body = "\n\n".join(kept)
    with open(NH_OUT, "w", encoding="utf-8", newline="\n") as file:
        file.write(f"{header}\n\\begindata\n\n{body}\n\n\\begintext\n")
    print(f"Wrote {len(kept)} entries to {NH_OUT}")


if __name__ == "__main__":
    main()
