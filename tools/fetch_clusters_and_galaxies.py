"""Fetches star clusters, the Orion Nebula and the Milky Way's satellite galaxies.

Writes clusters-and-galaxies.csv in crates/worldline-data/data: each object's
place, distance, motion across the sky and radial velocity as published, with
its size, its mass where its catalogue gives one, and its galactic longitude
and latitude where its catalogue gives them. See
docs/physics/clusters-and-galaxies.md.

- Globular clusters: Baumgardt et al.'s Galactic Globular Cluster Database,
  version 4 (2023), every cluster with a distance, proper motion and radial
  velocity.
- Open clusters: Hunt & Reffert (2023, A&A 673, A114), from Gaia DR3, through
  VizieR: those with a Messier number, and the Hyades.
- The Orion Nebula: Kuhn et al. (2019, ApJ 870, 32), copied below.
- Satellite galaxies: the Local Volume Database (Pace 2025, OJAp 8, 142), at
  the commit named below, every confirmed satellite of the Milky Way or the
  Large Magellanic Cloud with a distance, proper motion and radial velocity.

Standard library only: python tools/fetch_clusters_and_galaxies.py
"""

import csv
import io
import re
import urllib.parse
import urllib.request
from decimal import Decimal
from html.parser import HTMLParser
from pathlib import Path

OUT = Path(__file__).parent.parent / "crates/worldline-data/data/clusters-and-galaxies.csv"
BAUMGARDT = "https://people.smp.uq.edu.au/HolgerBaumgardt/globular/"
VIZIER = "https://vizier.cfa.harvard.edu/viz-bin/asu-tsv"
# The Local Volume Database at release v1.1.2.
LVDB_COMMIT = "aa4d3d209300060a97611f40240c8458811c708b"
LVDB = f"https://raw.githubusercontent.com/apace7/local_volume_database/{LVDB_COMMIT}/data/dwarf_mw.csv"

COLUMNS = [
    "name", "also", "kind", "ra", "dec", "epoch",
    "distance", "distance_error", "distance_unit",
    "pmra", "pmra_error", "pmdec", "pmdec_error", "rv", "rv_error",
    "radius", "radius_unit", "mass", "mass_error", "l", "b", "source",
]

# Kuhn et al. (2019), ApJ 870, 32. Table 1, the ONC: its center (J2000), its
# median proper motion and its distance from the stars' Gaia DR2 parallaxes.
# Table 5: its radial velocity, the median of 12 stars' Gaia DR2 radial
# velocities, which the paper adopts where at least 5 stars have one.
ORION = {
    "name": "Orion Nebula",
    "also": "M42, NGC 1976; placed as its star cluster, the Orion Nebula Cluster",
    "kind": "nebula",
    "ra": "05 35 15.68",
    "dec": "-05 23 40.1",
    "epoch": "2015.5",
    "distance": "403",
    "distance_error": "-6/+7",
    "distance_unit": "pc",
    "pmra": "1.51",
    "pmra_error": "0.11",
    "pmdec": "0.50",
    "pmdec_error": "0.12",
    "rv": "21.8",
    "rv_error": "6.6",
    "source": "Kuhn et al. 2019, ApJ 870, 32: Table 1 (the Orion Nebula Cluster's center, "
    "proper motion and distance, from Gaia DR2) and Table 5 (its radial velocity, the median "
    "of 12 stars' from Gaia DR2). Its size isn't in their tables",
}

# Names used for clusters better known by them.
COMMON = {"47 Tuc": "47 Tucanae", "ω Cen": "Omega Centauri"}
MESSIER_OPEN = {45: "Pleiades", 44: "Praesepe"}

# The open clusters in Messier's catalogue, by their name in Hunt & Reffert
# (2023), with the Hyades.
OPEN_CLUSTERS = {
    "M6": "NGC_6405", "M7": "NGC_6475", "M11": "NGC_6705", "M16": "NGC_6611",
    "M18": "NGC_6613", "M21": "NGC_6531", "M23": "NGC_6494", "M25": "IC_4725",
    "M26": "NGC_6694", "M29": "NGC_6913", "M34": "NGC_1039", "M35": "NGC_2168",
    "M36": "NGC_1960", "M37": "NGC_2099", "M38": "NGC_1912", "M39": "NGC_7092",
    "M41": "NGC_2287", "M44": "NGC_2632", "M45": "Melotte_22", "M46": "NGC_2437",
    "M47": "NGC_2422", "M48": "NGC_2548", "M50": "NGC_2323", "M52": "NGC_7654",
    "M67": "NGC_2682", "M93": "NGC_2447", "M103": "NGC_581", "Hyades": "Melotte_25",
}
SATELLITES = {
    "LMC": ("Large Magellanic Cloud", "LMC"),
    "SMC": ("Small Magellanic Cloud", "SMC"),
    "Sagittarius": ("Sagittarius Dwarf", "Sagittarius dwarf spheroidal galaxy"),
}


def get(url):
    request = urllib.request.Request(url, headers={"User-Agent": "worldline-fetch"})
    with urllib.request.urlopen(request, timeout=180) as reply:
        return reply.read().decode("utf-8")


class Table(HTMLParser):
    """The rows of an HTML table: each cell's text, the sources its tooltips
    name, and whether it's in italics."""

    def __init__(self):
        super().__init__()
        self.rows, self.row, self.cell, self.link = [], None, None, False

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag == "tr":
            self.row = []
        elif tag == "td" and self.row is not None:
            italic = "italic" in (a.get("style") or "")
            self.cell = {"text": "", "link": "", "titles": [], "italic": italic}
        elif self.cell is None:
            return
        elif tag == "span" and "title" in a:
            self.cell["titles"].append(a["title"].strip())
        elif tag == "i":
            self.cell["italic"] = True
        elif tag == "a":
            self.link = True
        elif tag == "sup":
            self.cell["text"] += "^"

    def handle_endtag(self, tag):
        if tag == "td" and self.cell is not None:
            self.row.append(self.cell)
            self.cell = None
        elif tag == "tr" and self.row:
            self.rows.append(self.row)
            self.row = None
        elif tag == "a":
            self.link = False

    def handle_data(self, data):
        if self.cell is not None:
            self.cell["text"] += data
            if self.link:
                self.cell["link"] += data


def clean(text):
    return re.sub(r"\s+", " ", text.replace("\xa0", " ")).strip()


def plus_minus(cell):
    """'4.52 ± 0.03' as ('4.52', '0.03')."""
    value, error = clean(cell["text"]).split(" ± ")
    return value, error


def baumgardt_rows(page):
    table = Table()
    table.feed(get(BAUMGARDT + page))
    return [r for r in table.rows if len(r) > 10 and r[0]["link"]]


def globular_clusters():
    masses = {}
    for r in baumgardt_rows("parameter.html"):
        name = clean(r[0]["link"])
        # "8.53 ± 0.05 · 10^5": the mass and its error, as published; some
        # have no error.
        m = re.fullmatch(r"([\d.]+)(?: ± ([\d.]+))? · 10\^(\d+)", clean(r[7]["text"]))
        if not m:
            raise SystemExit(f"{name}: unexpected mass {r[7]['text']!r}")
        if r[7]["italic"]:
            raise SystemExit(f"{name}: its mass is from its luminosity; label it before using it")
        error = f"{m[2]}e{m[3]}" if m[2] else ""
        masses[name] = (f"{m[1]}e{m[3]}", error, clean(r[11]["text"]))
    out = []
    for r in baumgardt_rows("orbits.html"):
        name = clean(r[0]["link"])
        alias = clean(r[0]["text"])[len(name):].strip()
        distance, distance_error = plus_minus(r[5])
        rv, rv_error = plus_minus(r[7])
        pmra, pmra_error = plus_minus(r[8])
        pmdec, pmdec_error = plus_minus(r[9])
        mass, mass_error, radius = masses.get(name, ("", "", ""))
        messier = re.fullmatch(r"M (\d+)", alias)
        if messier:
            shown, also = f"M{messier[1]}", name
        elif alias in COMMON:
            shown, also = COMMON[alias], name
        else:
            shown, also = name, alias
        center = r[1]["titles"][0] if r[1]["titles"] else "Baumgardt et al."
        distance_by = r[5]["titles"][0] if r[5]["titles"] else "Baumgardt et al."
        motion_by = r[8]["titles"][0] if r[8]["titles"] else "Baumgardt et al."
        out.append({
            "name": shown,
            "also": also,
            "kind": "globular cluster",
            "ra": clean(r[1]["text"]),
            "dec": clean(r[2]["text"]),
            "epoch": "2016.0",
            "distance": distance,
            "distance_error": distance_error,
            "distance_unit": "kpc",
            "pmra": pmra,
            "pmra_error": pmra_error,
            "pmdec": pmdec,
            "pmdec_error": pmdec_error,
            "rv": rv,
            "rv_error": rv_error,
            "radius": radius,
            "radius_unit": "pc" if radius else "",
            "mass": mass,
            "mass_error": mass_error,
            "l": clean(r[3]["text"]),
            "b": clean(r[4]["text"]),
            "source": f"Baumgardt et al., Galactic Globular Cluster Database, version 4 (2023): "
            f"center from {center}; distance from {distance_by}; proper motion from {motion_by} "
            f"(Gaia EDR3); radial velocity from the database's compilation (Baumgardt et al. 2019, "
            f"MNRAS 482, 5138)"
            + ("; mass and projected half-light radius from its N-body fits (Baumgardt & Hilker "
               "2018, MNRAS 478, 1520)" if mass else "; no mass or radius in the database"),
        })
    return out


def open_clusters():
    columns = ("Name,AllNames,Type,N,RA_ICRS,DE_ICRS,GLON,GLAT,r50pc,pmRA,e_pmRA,pmDE,e_pmDE,"
               "dist16,dist50,dist84,RV,e_RV,n_RV")
    query = urllib.parse.urlencode({
        "-source": "J/A+A/673/A114/clusters",
        "-out": columns,
        "-out.max": "unlimited",
        "Type": "o",
    })
    lines = [l for l in get(f"{VIZIER}?{query}").splitlines() if l and not l.startswith("#")]
    header = lines[0].split("\t")
    rows = [dict(zip(header, (f.strip() for f in l.split("\t")))) for l in lines[3:]]
    out = []
    for key, catalog_name in OPEN_CLUSTERS.items():
        # By its name, or among the names it's cross-matched to (M39 is
        # Theia 517 there); of several, the one with the most members.
        matches = [
            r for r in rows if r["Name"] == catalog_name or catalog_name in r["AllNames"].split(",")
        ]
        if not matches:
            raise SystemExit(f"{key}: {catalog_name} isn't in Hunt & Reffert's catalogue")
        row = max(matches, key=lambda r: int(r["N"]))
        if not row["RV"]:
            print(f"skipped {key} ({catalog_name}): no radial velocity")
            continue
        designation = catalog_name.replace("_", " ")
        number = int(key[1:]) if key.startswith("M") else None
        if key == "Hyades":
            shown, also = "Hyades", designation
        elif number in MESSIER_OPEN:
            shown, also = MESSIER_OPEN[number], f"{key}, {designation}"
        else:
            shown, also = key, designation
        low, mid, high = (Decimal(row[k]) for k in ("dist16", "dist50", "dist84"))
        out.append({
            "name": shown,
            "also": also,
            "kind": "open cluster",
            "ra": row["RA_ICRS"],
            "dec": row["DE_ICRS"],
            "epoch": "2016.0",
            "distance": row["dist50"],
            "distance_error": f"-{mid - low}/+{high - mid}",
            "distance_unit": "pc",
            "pmra": row["pmRA"],
            "pmra_error": row["e_pmRA"],
            "pmdec": row["pmDE"],
            "pmdec_error": row["e_pmDE"],
            "rv": row["RV"],
            "rv_error": row["e_RV"],
            "radius": row["r50pc"],
            "radius_unit": "pc",
            "mass": "",
            "mass_error": "",
            "l": row["GLON"],
            "b": row["GLAT"],
            "source": f"Hunt & Reffert 2023, A&A 673, A114 (Gaia DR3), as {row['Name']}: its densest "
            f"point, median distance, mean proper motion, mean Gaia DR3 radial velocity of "
            f"{row['n_RV']} stars, and the radius holding half its members",
        })
    return out


def reference(key):
    """'Pietrzynski2019Natur.567..200P' as 'Pietrzynski 2019 (2019Natur.567..200P)'."""
    m = re.fullmatch(r"(\D+?)(\d{4})(.+)", key)
    return f"{m[1]} {m[2]} ({m[2]}{m[3]})" if m else key


def error(minus, plus):
    return minus if minus == plus else f"-{minus}/+{plus}"


def satellites():
    out = []
    for row in csv.DictReader(io.StringIO(get(LVDB))):
        needed = ("ra", "dec", "distance_modulus", "pmra", "pmdec", "vlos_systemic", "rhalf")
        if row["confirmed_galaxy"] != "1" or row["host"] not in ("mw", "lmc"):
            continue
        if any(row[k] in ("", "nan") for k in needed):
            continue
        shown, also = SATELLITES.get(row["name"], (row["name"], ""))
        out.append({
            "name": shown,
            "also": also,
            "kind": "satellite galaxy",
            "ra": row["ra"],
            "dec": row["dec"],
            "epoch": "2016.0",
            "distance": row["distance_modulus"],
            "distance_error": error(row["distance_modulus_em"], row["distance_modulus_ep"]),
            "distance_unit": "mag",
            "pmra": row["pmra"],
            "pmra_error": error(row["pmra_em"], row["pmra_ep"]),
            "pmdec": row["pmdec"],
            "pmdec_error": error(row["pmdec_em"], row["pmdec_ep"]),
            "rv": row["vlos_systemic"],
            "rv_error": error(row["vlos_systemic_em"], row["vlos_systemic_ep"]),
            "radius": row["rhalf"],
            "radius_unit": "arcmin",
            "mass": "",
            "mass_error": "",
            "l": row["ll"],
            "b": row["bb"],
            "source": f"Local Volume Database (Pace 2025, OJAp 8, 142), release v1.1.2: center and "
            f"half-light radius from {reference(row['ref_structure'])}; distance from "
            f"{reference(row['ref_distance'])}; proper motion from "
            f"{reference(row['ref_proper_motion'])}; radial velocity from {reference(row['ref_vlos'])}",
        })
    return out


def main():
    objects = globular_clusters() + open_clusters() + [ORION] + satellites()
    names = [o["name"] for o in objects]
    duplicates = {n for n in names if names.count(n) > 1}
    if duplicates:
        raise SystemExit(f"names used twice: {duplicates}")
    with OUT.open("w", newline="\n", encoding="utf-8") as out:
        out.write(
            "# Worldline: star clusters, the Orion Nebula and the Milky Way's satellite galaxies, "
            "written by tools/fetch_clusters_and_galaxies.py. Each as its catalogue publishes it.\n"
            "# Globular clusters: Baumgardt et al., Galactic Globular Cluster Database, version 4 "
            "(2023), https://people.smp.uq.edu.au/HolgerBaumgardt/globular/ (orbits and parameter "
            "tables). Open clusters: Hunt & Reffert 2023, A&A 673, A114, from VizieR. The Orion "
            "Nebula: Kuhn et al. 2019, ApJ 870, 32. Satellite galaxies: the Local Volume Database, "
            f"release v1.1.2 (commit {LVDB_COMMIT}).\n"
            "# ra, dec: ICRS, degrees, or h m s and d m s; epoch: the Julian year the motion is "
            "measured for. distance in distance_unit (a distance modulus in mag). pmra (times "
            "cos dec), pmdec: mas/yr. rv: km/s, barycentric. radius in radius_unit: a globular "
            "cluster's projected half-light radius, an open cluster's radius holding half its "
            "members, a galaxy's half-light radius along its major axis. mass: Suns. l, b: "
            "galactic longitude and latitude, degrees, as the catalogue gives them. Errors are as "
            "published, one standard deviation, or lower/upper.\n"
        )
        writer = csv.DictWriter(out, fieldnames=COLUMNS, lineterminator="\n")
        writer.writeheader()
        writer.writerows(objects)
    kinds = {}
    for o in objects:
        kinds[o["kind"]] = kinds.get(o["kind"], 0) + 1
    print(f"wrote {len(objects)} objects to {OUT}: {kinds}")


if __name__ == "__main__":
    main()
