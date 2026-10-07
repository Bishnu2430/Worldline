"""Fetch NASA JPL Horizons states for a sample of the belts' bodies, to check
the swarm (the belts as live particles) against JPL.

Writes, under crates/worldline-data/data/:

- belt-samples.csv: for every 1,664th main-belt asteroid, every 752nd
  Jupiter Trojan and every 642nd trans-Neptunian object in the belt files
  (12, 6 and 6 bodies, spread evenly through each list), its position and
  velocity at 2025-01-01 00:00 TDB, the planets' snapshot, and exactly one
  Julian year later, 2026-01-01 06:00 TDB. Relative to the solar system
  barycenter, like the snapshot.

Run tools/fetch_belts.py first. Run from anywhere:
python tools/fetch_belt_samples.py
Uses only the Python standard library.
"""

import re
from datetime import date

from fetch_moons import DATA, write
from fetch_small_bodies import horizons

START, ONE_YEAR_ON = 2460676.5, 2461041.75
BELTS = [("asteroid-belt.csv", "asteroid belt", 12),
         ("jupiter-trojans.csv", "jupiter trojans", 6),
         ("kuiper-belt.csv", "kuiper belt", 6)]


def designations(file, count):
    rows = [line.split(",")[0] for line in (DATA / file).read_text(encoding="utf-8").splitlines()
            if line and not line.startswith("#")][1:]
    stride = len(rows) // count
    return [rows[k * stride] for k in range(count)]


def main():
    lines = []
    sources = []
    for file, belt, count in BELTS:
        for des in designations(file, count):
            # Numbered bodies by number; the rest by designation.
            command = f"{des};" if des.isdigit() else f"DES={des};"
            result = horizons(command, EPHEM_TYPE="VECTORS", VEC_TABLE="2",
                              START_TIME=f"JD{START}", STOP_TIME=f"JD{ONE_YEAR_ON}", STEP_SIZE="1")
            source = re.search(r"Target body name: (.*?)\s*\{source: ([^}]+)\}", result)
            sources.append(f"{source.group(1).strip()} {source.group(2).strip()}")
            block = result.split("$$SOE")[1].split("$$EOE")[0]
            states = {}
            for line in block.strip().splitlines():
                fields = [f.strip() for f in line.split(",")]
                states[float(fields[0])] = fields[2:8]
            lines.append(",".join([des, belt, *states[START], *states[ONE_YEAR_ON]]))
            print(f"{des} ({belt}): {sources[-1]}")
    write(DATA / "belt-samples.csv", [
        "# Worldline: a sample of the belts' bodies from NASA JPL Horizons, to check the swarm. "
        "Regenerate with: python tools/fetch_belt_samples.py",
        f"# states: NASA JPL Horizons small-body solutions ({'; '.join(sources)}), fetched {date.today()}",
        "# frame: ICRF, ecliptic and mean equinox of J2000, origin at the solar system barycenter; km, km/s",
        f"# epochs: 2025-01-01 00:00:00 TDB (JD {START}) and 2026-01-01 06:00:00 TDB (JD {ONE_YEAR_ON})",
        "designation,belt,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s,"
        "x1_km,y1_km,z1_km,vx1_km_s,vy1_km_s,vz1_km_s",
    ] + lines)


if __name__ == "__main__":
    main()
