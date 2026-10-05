"""Fetch the planet textures Worldline ships with.

Downloads 2K (2048 x 1024) equirectangular surface maps from Solar System
Scope (https://www.solarsystemscope.com/textures/), distributed under the
Creative Commons Attribution 4.0 International license (CC BY 4.0). The
maps are based on NASA imagery and elevation data. Credit is given in
CREDITS.md. Maps the site labels "fictional" are not used.

Run from anywhere:  python tools/fetch_textures.py
Uses only the Python standard library.
"""

import urllib.request
from pathlib import Path

BASE = "https://www.solarsystemscope.com/textures/download/"
OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-app" / "assets" / "textures"

FILES = [
    "2k_sun.jpg",
    "2k_mercury.jpg",
    "2k_venus_atmosphere.jpg",
    "2k_earth_daymap.jpg",
    "2k_earth_nightmap.jpg",
    "2k_moon.jpg",
    "2k_mars.jpg",
    "2k_jupiter.jpg",
    "2k_saturn.jpg",
    "2k_uranus.jpg",
    "2k_neptune.jpg",
]


def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name in FILES:
        request = urllib.request.Request(BASE + name, headers={"User-Agent": "Worldline texture fetch"})
        with urllib.request.urlopen(request, timeout=120) as response:
            data = response.read()
        (OUT_DIR / name).write_bytes(data)
        print(f"{name}: {len(data) / 1e6:.2f} MB")


if __name__ == "__main__":
    main()
