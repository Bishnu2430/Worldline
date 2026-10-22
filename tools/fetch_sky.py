"""Fetch the sky behind everything: the bright stars and the Milky Way's glow.

Writes:

- crates/worldline-data/data/bright-stars.csv: every star of the Yale
  Bright Star Catalogue, 5th revised edition (Hoffleit & Warren 1991),
  from CDS catalog V/50: its J2000 position, galactic coordinates, V
  magnitude and B-V color, digits as published. Entries without a position (removed from the
  catalogue) are left out.
- crates/worldline-app/assets/textures/gaia_edr3_sky_2k.png: "The colour of
  the sky from Gaia's Early Data Release 3", equirectangular, in galactic
  coordinates, 2000 x 1000. ESA/Gaia/DPAC, CC BY-SA 3.0 IGO;
  acknowledgement: A. Moitinho. Stored unchanged.

Credit is given in CREDITS.md.

Run from anywhere:  python tools/fetch_sky.py
Uses only the Python standard library.
"""

import csv
import gzip
import io
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "crates" / "worldline-data" / "data"
TEXTURES = ROOT / "crates" / "worldline-app" / "assets" / "textures"

CATALOG = "https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz"
GAIA = (
    "https://sci.esa.int/documents/33580/35361/Gaia_EDR3_flux_cartesian_2k.png/"
    "09ea4802-8b67-3086-6c86-b64d5ca7911b?version=1.0&t=1606925449184&download=true"
)


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "Worldline data fetch (github.com/Bishnu2430/Worldline)"})
    with urllib.request.urlopen(request, timeout=300) as response:
        return response.read()


def field(line, first, last):
    """Bytes first to last (1-based, inclusive) of a fixed-width line."""
    return line[first - 1:last].strip()


def bright_stars():
    text = gzip.decompress(fetch(CATALOG)).decode("ascii")
    rows = []
    for line in text.splitlines():
        line = line.ljust(197)
        # Bytes as in the catalogue's ReadMe: the J2000 position (76-90),
        # galactic longitude and latitude (91-102), Vmag (103-107), B-V
        # (110-114) and the proper motion (149-160).
        if not field(line, 76, 77):
            continue
        rows.append([
            field(line, 1, 4),
            field(line, 5, 14),
            field(line, 76, 77), field(line, 78, 79), field(line, 80, 83),
            field(line, 84, 84) or "+", field(line, 85, 86), field(line, 87, 88), field(line, 89, 90),
            field(line, 91, 96), field(line, 97, 102),
            field(line, 103, 107),
            field(line, 110, 114),
            field(line, 149, 154), field(line, 155, 160),
        ])
    out = io.StringIO()
    out.write("# Worldline: the Yale Bright Star Catalogue, 5th revised edition (Hoffleit & Warren 1991), "
              "CDS catalog V/50. Regenerate with: python tools/fetch_sky.py\n")
    out.write("# Position: equinox and epoch J2000 (FK5). glon, glat: galactic longitude and latitude (deg). "
              "vmag: visual magnitude; b_v: B-V color, empty if not measured. Stars removed from the "
              "catalogue (no position) are left out. pm_ra (times cos dec), pm_dec: proper motion, arcsec/yr. "
              "The galactic coordinates are for the positions of epoch 1950.\n")
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(["hr", "name", "ra_h", "ra_m", "ra_s", "dec_sign", "dec_deg", "dec_arcmin", "dec_arcsec",
                     "glon", "glat", "vmag", "b_v", "pm_ra", "pm_dec"])
    writer.writerows(rows)
    path = DATA / "bright-stars.csv"
    path.write_bytes(out.getvalue().encode("utf-8"))
    print(f"Wrote {path} ({len(rows)} stars)")


def gaia_map():
    data = fetch(GAIA)
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    path = TEXTURES / "gaia_edr3_sky_2k.png"
    path.write_bytes(data)
    print(f"Wrote {path} ({len(data) / 1e6:.1f} MB)")


def main():
    bright_stars()
    gaia_map()


if __name__ == "__main__":
    main()
