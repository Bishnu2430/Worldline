"""Fetch the Saturn ring data Worldline ships with.

1. A measured radial profile of the rings' optical depth: Cassini Radio
   Science radio occultation Rev 7 egress, X band (2005-05-03), as archived
   by NASA's Planetary Data System Ring-Moon Systems Node (data set
   CO-SR-RSS-4/5-OCC-V2.0, product RSS_2005_123_X43_E_TAU_10KM). Its radial
   resolution is 10 km; this script averages it into 10 km bins.
2. The table of named rings, gaps and ringlets with their boundaries, from
   the same node's "Vital Statistics for Saturn's Rings".

Writes CSV files under crates/worldline-data/data/.

Run from anywhere:  python tools/fetch_rings.py
Uses only the Python standard library.
"""

import math
import re
import urllib.request
from datetime import date
from html.parser import HTMLParser
from pathlib import Path

OCCULTATION = (
    "https://pds-rings.seti.org/holdings/volumes/CORSS_8xxx/CORSS_8001/data/Rev007/Rev007E/"
    "Rev007E_RSS_2005_123_X43_E/RSS_2005_123_X43_E_TAU_10KM.TAB"
)
TABLE = "https://pds-rings.seti.org/saturn/saturn_tables.html"
OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data"
BIN_KM = 10.0


def fetch(url):
    # The PDS server turns away Python's default user agent.
    request = urllib.request.Request(url, headers={"User-Agent": "Worldline data fetch (github.com/Bishnu2430/Worldline)"})
    with urllib.request.urlopen(request, timeout=180) as response:
        return response.read().decode("utf-8", errors="replace")


def write(path, lines):
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines) + "\n")
    print(f"Wrote {path}")


def profile():
    bins = {}
    for line in fetch(OCCULTATION).splitlines():
        fields = [f.strip() for f in line.split(",")]
        if len(fields) < 9:
            continue
        # Columns (from the PDS label): 1 ring radius, 2 and 3 radius
        # corrections to add, 7 normal optical depth, 9 the threshold above
        # which the signal is lost in noise.
        radius = float(fields[0]) + float(fields[1]) + float(fields[2])
        tau, threshold = float(fields[6]), float(fields[8])
        index = math.floor(radius / BIN_KM)
        bins.setdefault(index, []).append((tau, threshold))

    lines = [
        "# Saturn's rings: measured normal optical depth versus radius.",
        "# Regenerate with: python tools/fetch_rings.py",
        "# source: NASA PDS Ring-Moon Systems Node, Cassini Radio Science (RSS) occultation,",
        "#         data set CO-SR-RSS-4/5-OCC-V2.0, product RSS_2005_123_X43_E_TAU_10KM",
        "#         (Rev 7 egress, X band, 2005-05-03; reconstruction per Marouf et al. 1986, Icarus 68, 120)",
        f"# fetched: {date.today()}",
        "# processing: ring radius corrected by the label's two radius-correction columns;",
        f"#             samples averaged into {BIN_KM:.0f} km bins (bin center listed).",
        "# tau_measured: mean normal optical depth (can be slightly negative from noise).",
        "# tau_threshold: optical depth at which the signal sinks into the noise; where tau",
        "#                reaches it (core of the B ring), the true value is at least this.",
        "radius_km,tau_measured,tau_threshold",
    ]
    for index in sorted(bins):
        samples = bins[index]
        tau = sum(s[0] for s in samples) / len(samples)
        threshold = sum(s[1] for s in samples) / len(samples)
        center = (index + 0.5) * BIN_KM
        lines.append(f"{center:.1f},{tau:.6f},{threshold:.6f}")
    write(OUT_DIR / "saturn-rings-profile.csv", lines)


class TableParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.tables, self.row, self.cell = [], None, None

    def handle_starttag(self, tag, attrs):
        if tag == "table":
            self.tables.append([])
        elif tag == "tr":
            self.row = []
        elif tag in ("td", "th"):
            self.cell = ""

    def handle_endtag(self, tag):
        if tag in ("td", "th") and self.cell is not None:
            self.row.append(" ".join(self.cell.split()))
            self.cell = None
        elif tag == "tr" and self.row is not None:
            self.tables[-1].append(self.row)
            self.row = None

    def handle_data(self, data):
        if self.cell is not None:
            self.cell += data


def features():
    parser = TableParser()
    parser.feed(fetch(TABLE))
    rings = parser.tables[0]
    lines = [
        "# Saturn's named rings, regions, gaps and ringlets.",
        "# Regenerate with: python tools/fetch_rings.py",
        f"# source: NASA PDS Ring-Moon Systems Node, 'Vital Statistics for Saturn's Rings', {TABLE}",
        f"# fetched: {date.today()}",
        "# Boundaries copied as published, in km. Features with only one published radius",
        "# have inner = outer. optical_depth is the published text.",
        "name,inner_km,outer_km,optical_depth,type",
    ]
    for row in rings[1:]:
        name, inner, outer, tau, kind = row[0], row[1], row[2], row[3], row[4]
        inner = re.sub(r"[~,]", "", inner)
        outer = re.sub(r"[~,]", "", outer) or inner
        lines.append(",".join([name, inner, outer, f'"{tau}"', f'"{kind}"']))
    write(OUT_DIR / "saturn-ring-features.csv", lines)


if __name__ == "__main__":
    profile()
    features()
