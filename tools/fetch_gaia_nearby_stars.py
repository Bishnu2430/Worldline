"""Fetches Gaia DR3's stars within 50 parsecs with a full measured motion.

Writes gaia-nearby-stars.csv in crates/worldline-data/data: every Gaia DR3
source with a parallax over 20 mas measured to better than a tenth, good
astrometry (a RUWE under 1.4, as Lindegren et al. 2021 advise) and a radial
velocity, from ESA's Gaia archive. Each row's place, parallax, proper motion
and radial velocity are Gaia's, at its epoch, 2016.0. See
docs/physics/galaxy-stars.md.

Standard library only: python tools/fetch_gaia_nearby_stars.py
"""

import urllib.parse
import urllib.request
from pathlib import Path

OUT = Path(__file__).parent.parent / "crates/worldline-data/data/gaia-nearby-stars.csv"
GAIA = "https://gea.esac.esa.int/tap-server/tap/sync"
QUERY = (
    "SELECT source_id, ra, dec, parallax, pmra, pmdec, radial_velocity "
    "FROM gaiadr3.gaia_source "
    "WHERE parallax > 20 AND parallax_over_error > 10 AND ruwe < 1.4 "
    "AND radial_velocity IS NOT NULL "
    "ORDER BY source_id"
)


def main():
    data = urllib.parse.urlencode(
        {"REQUEST": "doQuery", "LANG": "ADQL", "FORMAT": "csv", "QUERY": QUERY}
    ).encode()
    request = urllib.request.Request(GAIA, data=data, headers={"User-Agent": "worldline-fetch"})
    with urllib.request.urlopen(request, timeout=600) as reply:
        rows = reply.read().decode("utf-8").splitlines()
    if rows[0] != "source_id,ra,dec,parallax,pmra,pmdec,radial_velocity":
        raise SystemExit(f"unexpected header {rows[0]!r}")
    with OUT.open("w", newline="\n", encoding="utf-8") as out:
        out.write(
            "# Worldline: Gaia DR3's stars within 50 pc with a full measured motion, written by "
            "tools/fetch_gaia_nearby_stars.py from ESA's Gaia archive (Gaia Collaboration, "
            "Vallenari et al. 2023, A&A 674, A1).\n"
            f"# Query: {QUERY}\n"
            "# ra, dec: ICRS, deg, at 2016.0. parallax: mas. pmra (times cos dec), pmdec: mas/yr. "
            "radial_velocity: km/s. As the archive gives them.\n"
        )
        out.write("\n".join(rows) + "\n")
    print(f"wrote {len(rows) - 1} stars to {OUT}")


if __name__ == "__main__":
    main()
