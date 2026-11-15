"""Fetches the nearest and brightest stars' measured places and motions.

Writes neighbors.csv and neighbor-binaries.csv in crates/worldline-data/data.
Positions, parallaxes and proper motions come from Gaia DR3 where it measures a star well, otherwise from
the new reduction of Hipparcos (van Leeuwen 2007), both from ESA's Gaia
archive, each at its own reference epoch. Radial velocities come from Gaia DR3
or, for bright stars, from SIMBAD, with the paper it cites. Masses and radii
are copied below from the papers named. See docs/physics/neighbors.md.

Standard library only: python tools/fetch_neighbors.py
"""

import csv
import io
import time
import urllib.parse
import urllib.request
from pathlib import Path

DATA = Path(__file__).parent.parent / "crates/worldline-data/data"
OUT = DATA / "neighbors.csv"
BINARIES = DATA / "neighbor-binaries.csv"
GAIA = "https://gea.esac.esa.int/tap-server/tap/sync"
SIMBAD = "https://simbad.harvard.edu/simbad/sim-tap/sync"

# Gaia's astrometry is trusted under this RUWE (Lindegren et al. 2021).
RUWE = 1.4

# name, Hipparcos number, Gaia DR3 source if it has no Hipparcos number,
# SIMBAD identifier, mass (Suns), radius (Suns), where they come from.
STARS = [
    ("Proxima Centauri", 70890, None, "GJ 551", 0.1221, 0.1542,
     "Kervella, Thevenin & Lovis 2017, A&A 598, L7"),
    ("Barnard's Star", 87937, None, "GJ 699", 0.162, 0.185,
     "Gonzalez Hernandez et al. 2024, A&A 690, A79, after Schweitzer et al. 2019"),
    ("Wolf 359", None, 3864972938605115520, "GJ 406", 0.110, 0.144,
     "Pineda et al. 2021, ApJ, doi:10.3847/1538-4357/ac0aea (as GJ 406)"),
    ("Ross 128", 57548, None, "GJ 447", 0.168, 0.209,
     "Souto et al. 2018, ApJL, doi:10.3847/2041-8213/aac896, after Mann et al. 2015 "
     "and Stassun et al. 2017"),
    ("61 Cygni A", 104214, None, "GJ 820 A", 0.690, 0.665,
     "Kervella et al. 2008, A&A, doi:10.1051/0004-6361:200810080; the mass is their model's"),
    ("61 Cygni B", 104217, None, "GJ 820 B", 0.605, 0.595,
     "Kervella et al. 2008, A&A, doi:10.1051/0004-6361:200810080; the mass is their model's"),
    ("Tau Ceti", 8102, None, "* tau Cet", 0.783, 0.793,
     "Teixeira et al. 2009, A&A, doi:10.1051/0004-6361:200810746"),
    ("Procyon", 37279, None, "* alf CMi", 1.478, 2.048,
     "Mass: Bond et al. 2015, ApJ 813, 106. Radius: Kervella et al. 2004, A&A 413, 251. "
     "Procyon A alone; its white dwarf isn't a separate body"),
    ("Canopus", 30438, None, "* alf Car", 9.81, 73.2,
     "Domiciano de Souza et al. 2021, A&A 654, A19; the mass from its surface gravity and radius"),
    ("Arcturus", 69673, None, "* alf Boo", 1.08, 25.4,
     "Ramirez & Allende Prieto 2011, ApJ 743, 135"),
    ("Vega", 91262, None, "* alf Lyr", 2.15, 2.726,
     "Monnier et al. 2012, ApJ 761, L3; the radius is the equatorial one"),
    ("Capella", 24608, None, "* alf Aur", 5.061, 11.87,
     "Weber & Strassmeier 2011, A&A 531, A89: 2.573 and 2.488 Suns, one body here, "
     "with the larger star's radius"),
    ("Rigel", 24436, None, "* bet Ori", 23.0, 78.9,
     "Mass: Przybilla et al. 2010, as Moravveji et al. 2012, ApJ 747, 108 quote it. "
     "Radius: Moravveji et al. 2012"),
    ("Achernar", 7588, None, "* alf Eri", 6.1, 9.16,
     "Domiciano de Souza et al. 2014: the equatorial radius as Kervella et al. 2022, "
     "A&A 667, A111 give it, the mass as Montesinos 2024 (arXiv:2406.18392) does"),
    ("Betelgeuse", 27989, None, "* alf Ori", 17.75, 764.0,
     "Joyce et al. 2020, ApJ 902, doi:10.3847/1538-4357/abb8db; the radius is their "
     "model's prediction, the mass the middle of their 16.5 to 19 Suns"),
]


# The two pairs close enough for their orbit to matter: a star's own motion in
# the catalogues includes it, so these go by their published orbits. B's orbit
# around A, on the sky: period (yr), periastron (Julian year), semi-major axis
# ("), eccentricity, inclination, node and periastron (deg).
ALPHA_CENTAURI = {
    "primary": "Alpha Centauri A",
    "secondary": "Alpha Centauri B",
    "period": 79.929,
    "periastron": 1955.604,
    "a": 17.592,
    "e": 0.5208,
    "i": 79.320,
    "node": 205.064,
    "omega": 232.006,
    "parallax": 747.17,
    "pmra": -3619.9,
    "pmdec": 693.8,
    "rv": -22.3930,
    "refers_to": "barycenter",
    "source": "Kervella et al. 2016, A&A 594, A107, Table 1; the barycenter's place from their "
    "SOFI image of 2009.1517 and its mass ratio 0.45884",
}
SIRIUS = {
    "primary": "Sirius A",
    "secondary": "Sirius B",
    "period": 50.1284,
    "periastron": 1994.5715,
    "a": 7.4957,
    "e": 0.59142,
    "i": 136.336,
    "node": 45.400,
    "omega": 149.161,
    "parallax": 378.9,
    "rv": -8.47,
    "refers_to": "primary",
    "source": "Bond et al. 2017, ApJ 840, 70: orbit, parallax, and the velocity corrected for "
    "Sirius A's gravitational redshift; place and proper motion: Hipparcos, new reduction, of "
    "Sirius A at 1991.25",
}


def degrees(hours, minutes, seconds):
    return (hours + minutes / 60 + seconds / 3600) * 15


def get(url, form):
    """The table a TAP service returns for a query."""
    query = urllib.parse.urlencode(form)
    for attempt in range(4):
        try:
            with urllib.request.urlopen(f"{url}?{query}", timeout=120) as reply:
                text = reply.read().decode("utf-8")
            return list(csv.DictReader(io.StringIO(text)))
        except OSError:
            if attempt == 3:
                raise
            time.sleep(45)
    return []


def gaia(adql):
    return get(GAIA, {"REQUEST": "doQuery", "LANG": "ADQL", "FORMAT": "csv", "QUERY": adql})


def simbad(adql):
    return get(SIMBAD, {"request": "doQuery", "lang": "adql", "format": "csv", "query": adql})


def main():
    numbers = ",".join(str(s[1]) for s in STARS if s[1])
    hipparcos = {
        int(r["hip"]): r
        for r in gaia(
            "SELECT hip, ra, dec, plx, e_plx, pm_ra, pm_de, e_pm_ra, e_pm_de "
            f"FROM public.hipparcos_newreduction WHERE hip IN ({numbers})"
        )
    }
    columns = (
        "s.source_id, s.ref_epoch, s.ra, s.dec, s.parallax, s.parallax_error, s.pmra, s.pmdec, "
        "s.pmra_error, s.pmdec_error, "
        "s.radial_velocity, s.radial_velocity_error, s.ruwe"
    )
    matched = {
        int(r["hip"]): r
        for r in gaia(
            f"SELECT n.original_ext_source_id AS hip, {columns} "
            "FROM gaiadr3.hipparcos2_best_neighbour AS n "
            "JOIN gaiadr3.gaia_source AS s ON n.source_id = s.source_id "
            f"WHERE n.original_ext_source_id IN ({numbers})"
        )
    }
    sources = ",".join(str(s[2]) for s in STARS if s[2])
    direct = {
        int(r["source_id"]): r
        for r in gaia(f"SELECT {columns} FROM gaiadr3.gaia_source AS s WHERE s.source_id IN ({sources})")
    }
    names = ",".join("'" + s[3].replace("'", "''") + "'" for s in STARS)
    velocities = {
        r["id"]: r
        for r in simbad(
            "SELECT i.id, b.rvz_radvel, b.rvz_err, b.rvz_bibcode FROM basic AS b "
            f"JOIN ident AS i ON b.oid = i.oidref WHERE i.id IN ({names})"
        )
    }
    rows = []
    for name, hip, source, ident, mass, radius, reference in STARS:
        g = matched.get(hip) if hip else direct.get(source)
        if g and g["parallax"] and float(g["ruwe"] or 99) < RUWE:
            row = {
                "catalog": f"Gaia DR3 {g['source_id']}",
                "epoch": g["ref_epoch"],
                "ra": g["ra"],
                "dec": g["dec"],
                "parallax": g["parallax"],
                "parallax_error": g["parallax_error"],
                "pmra": g["pmra"],
                "pmdec": g["pmdec"],
                "pmra_error": g["pmra_error"],
                "pmdec_error": g["pmdec_error"],
            }
        else:
            h = hipparcos[hip]
            row = {
                "catalog": f"Hipparcos {hip}",
                "epoch": "1991.25",
                "ra": h["ra"],
                "dec": h["dec"],
                "parallax": h["plx"],
                "parallax_error": h["e_plx"],
                "pmra": h["pm_ra"],
                "pmdec": h["pm_de"],
                "pmra_error": h["e_pm_ra"],
                "pmdec_error": h["e_pm_de"],
            }
        if g and g["radial_velocity"] and row["catalog"].startswith("Gaia"):
            speed, error, cited = g["radial_velocity"], g["radial_velocity_error"], "Gaia DR3"
        else:
            v = velocities[ident]
            speed, error, cited = v["rvz_radvel"], v["rvz_err"], v["rvz_bibcode"]
        rows.append(
            {
                "name": name,
                **row,
                "rv": speed,
                "rv_error": error,
                "rv_source": cited,
                "mass": mass,
                "radius": radius,
                "source": reference,
            }
        )
    # Between the two stars, at B's share of the mass, in their SOFI image.
    share = 0.45884
    a = degrees(14, 39, 31.813), -(60 + 50 / 60 + 0.01 / 3600)
    b = degrees(14, 39, 30.919), -(60 + 50 / 60 + 3.48 / 3600)
    centauri = {
        **ALPHA_CENTAURI,
        "epoch": 2009.1517,
        "ra": a[0] + share * (b[0] - a[0]),
        "dec": a[1] + share * (b[1] - a[1]),
    }
    sirius = gaia(
        "SELECT ra, dec, pm_ra, pm_de FROM public.hipparcos_newreduction WHERE hip = 32349"
    )[0]
    pair = {
        **SIRIUS,
        "epoch": 1991.25,
        "ra": sirius["ra"],
        "dec": sirius["dec"],
        "pmra": sirius["pm_ra"],
        "pmdec": sirius["pm_de"],
    }
    fields = [
        "primary", "secondary", "epoch", "ra", "dec", "parallax", "pmra", "pmdec", "rv",
        "refers_to", "period", "periastron", "a", "e", "i", "node", "omega", "source",
    ]
    with BINARIES.open("w", newline="\n", encoding="utf-8") as out:
        out.write(
            "# Worldline: nearby binaries by their published orbits, written by "
            "tools/fetch_neighbors.py.\n"
            "# The place (deg, ICRS, at `epoch`), parallax (mas), proper motion (mas/yr, pmra with "
            "cos dec) and velocity (km/s) are of the barycenter or of the primary, as `refers_to` "
            "says. Then B's orbit around A: period (yr), periastron (Julian year), semi-major axis "
            "(arcsec), eccentricity, inclination, node and periastron (deg). Masses and radii are "
            "in notable-objects.csv.\n"
        )
        writer = csv.DictWriter(out, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows([centauri, pair])
    with OUT.open("w", newline="\n", encoding="utf-8") as out:
        out.write(
            "# Worldline: the nearest and brightest stars, written by tools/fetch_neighbors.py.\n"
            "# Places and motions: Gaia DR3 (Gaia Collaboration 2023) where its RUWE is under 1.4, "
            "otherwise Hipparcos, new reduction (van Leeuwen 2007), both ICRS, at `epoch`, from "
            "ESA's Gaia archive. ra, dec: deg. parallax, its error: mas. pmra (with cos dec), "
            "pmdec, their errors: mas/yr. rv, its error: km/s, from Gaia DR3 or from SIMBAD with the bibcode "
            "it cites. mass, radius: Suns, from `source`.\n"
        )
        writer = csv.DictWriter(out, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    print(f"wrote {len(rows)} stars to {OUT}")


if __name__ == "__main__":
    main()
