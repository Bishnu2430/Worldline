"""Fetch the CIE 1931 colour-matching functions (2 degree observer).

How the eye's three kinds of cone respond to light of each wavelength, as
the CIE standardised it: x_bar, y_bar and z_bar from 360 nm to 830 nm in
steps of 1 nm (CIE 018:2019, Table 6; ISO/CIE 11664-1:2019). Worldline uses
them to find the color of a blackbody at a given temperature, the color it
draws a star (step 8.5b).

The file is the CIE's own open-access dataset, DOI 10.25039/CIE.DS.xvudnb9b,
under the Creative Commons Attribution-ShareAlike 4.0 licence. It is checked
against the SHA-256 the CIE publishes with it, and its rows are copied
exactly, under a header saying where they come from. Credit is given in
CREDITS.md.

Run from anywhere:  python tools/fetch_cie.py
Uses only the Python standard library.
"""

import hashlib
import urllib.request
from pathlib import Path

URL = "https://files.cie.co.at/Publications-datasets/CIE_xyz_1931_2deg.csv"
# As published in the dataset's metadata,
# CIE_xyz_1931_2deg.csv_metadata.json.
SHA256 = "fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1"
OUT = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data" / "cie-1931-2deg.csv"

HEADER = [
    "# Worldline: the CIE 1931 colour-matching functions, 2 degree observer, written by tools/fetch_cie.py.",
    "# CIE 2019, Colour-matching functions of CIE 1931 standard colorimetric observer, International "
    "Commission on Illumination (CIE), Vienna, AT, DOI: 10.25039/CIE.DS.xvudnb9b (original source: "
    "CIE 018:2019, Table 6). Licensed under CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/); "
    "this file is that dataset with this header added. Rows copied exactly.",
    "wavelength_nm,x_bar,y_bar,z_bar",
]


def main():
    request = urllib.request.Request(URL, headers={"User-Agent": "Worldline data fetch"})
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != SHA256:
        raise SystemExit(f"SHA-256 {digest} is not the one the CIE publishes, {SHA256}")
    rows = [line.strip() for line in data.decode("ascii").splitlines() if line.strip()]
    first, last = rows[0].split(",")[0], rows[-1].split(",")[0]
    if (first, last, len(rows)) != ("360", "830", 471):
        raise SystemExit(f"expected 360 to 830 nm in 471 rows, found {first} to {last} in {len(rows)}")
    OUT.write_text("\n".join(HEADER + rows) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {OUT} ({len(rows)} rows, {len(data) / 1e3:.0f} KB checked)")


if __name__ == "__main__":
    main()
