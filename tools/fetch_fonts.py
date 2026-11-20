"""Fetch the interface font Worldline ships with: Inter.

Downloads Inter's official release (Inter 4.1, by Rasmus Andersson,
https://github.com/rsms/inter), distributed under the SIL Open Font License
1.1, and keeps two of its static fonts, Regular and SemiBold, with the
license beside them. Credit is given in CREDITS.md.

Run from anywhere:  python tools/fetch_fonts.py
Uses only the Python standard library. The release (about 34 MB) is read
into memory and never written to disk; only the three files kept are.
"""

import io
import urllib.request
import zipfile
from pathlib import Path

RELEASE = "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip"
OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-app" / "assets" / "fonts"

# The files kept, as the release names them (by the end of their path), and
# what they're saved as.
KEEP = {
    "extras/ttf/Inter-Regular.ttf": "Inter-Regular.ttf",
    "extras/ttf/Inter-SemiBold.ttf": "Inter-SemiBold.ttf",
    "LICENSE.txt": "Inter-LICENSE.txt",
}


def main():
    request = urllib.request.Request(RELEASE, headers={"User-Agent": "Worldline font fetch"})
    with urllib.request.urlopen(request, timeout=300) as response:
        data = response.read()
    print(f"downloaded {len(data) / 1e6:.1f} MB")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as release:
        names = release.namelist()
        for wanted, saved in KEEP.items():
            matches = [n for n in names if n == wanted or n.endswith("/" + wanted)]
            if len(matches) != 1:
                raise SystemExit(f"{wanted}: found {matches} in the release")
            content = release.read(matches[0])
            (OUT_DIR / saved).write_bytes(content)
            print(f"{saved}: {len(content) / 1e3:.0f} KB")


if __name__ == "__main__":
    main()
