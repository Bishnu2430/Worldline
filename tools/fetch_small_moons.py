"""Fetch every known moon of Mars through Pluto beyond the major ones.

JPL's Solar System Dynamics group lists the mean orbital elements of every
planetary satellite with a computed orbit (https://ssd.jpl.nasa.gov/sats/elem/).
This script takes that list as "every known moon" and writes, under
crates/worldline-data/data/:

- moon-elements.csv: JPL's mean elements for every satellite in the list,
  copied as published (the epoch is converted from a calendar date to a
  Julian Date). Where the list has two entries for one moon, the one from
  the ephemeris Horizons serves is kept.
- small-moons-<date>.csv: each moon that isn't one of the 21 major moons,
  with its position and velocity relative to its planet's center (NASA JPL
  Horizons) at the snapshot epoch and 30 days later, its GM where JPL has
  measured one, and its mean radius where the IAU (pck00011) or JPL gives
  one. Asking Horizons for planet-relative states directly matters: a
  small moon's ephemeris can sit on an older planetary ephemeris than its
  planet's (Saturn's ring moons on DE437), and subtracting positions from
  the two misplaces the moon by over 100 km.
- small-moon-zonal.csv: each planet's oblateness (J2..J6) as the JPL
  solution of its innermost regular small moon has it. That solution can
  differ from the major moons' (Uranus's inner moons come from URA184).
- inner-moon-masses.csv: small moons that JPL's integration of the major
  moons includes with a mass, and that orbit inside the innermost major
  moon. To the major moons they pull like extra mass at the planet's center.

Run tools/fetch_rotation.py and tools/fetch_moons.py first. Takes about
15 minutes (one Horizons request per moon).

Run from anywhere:  python tools/fetch_small_moons.py
Uses only the Python standard library.
"""

import html
import re
import time
from datetime import date

from fetch_moons import DATA, EPOCHS, SYSTEMS, NoEphemeris, ephemeris_constants, fetch, mean_radii, states, write

ELEMENTS = "https://ssd.jpl.nasa.gov/sats/elem/"
PHYSICAL = "https://ssd.jpl.nasa.gov/sats/phys_par/"
HOSTS = [host for host, _, _, _ in SYSTEMS]
PLANET_IDS = {host: planet_id for host, _, planet_id, _ in SYSTEMS}
# Comment files for the Horizons ephemerides small moons come from, where
# the name differs.
COMMENT_FILES = {"ura184": "ura184_part-1", "nep098": "nep098_part-1", "sat441l": "sat441"}
MAJOR = {naif_id for _, _, _, moons in SYSTEMS for _, naif_id in moons} | {301}


def table(page, table_id):
    """Rows of an HTML table as lists of cell texts."""
    start = page.index(f'id="{table_id}"')
    body = page[start:page.index("</table>", start)]
    rows = []
    for row in re.findall(r"<tr[^>]*>(.*?)</tr>", body, re.S):
        cells = re.findall(r"<t[hd][^>]*>(.*?)</t[hd]>", row, re.S)
        rows.append([" ".join(html.unescape(re.sub(r"<[^>]+>", " ", c)).split()) for c in cells])
    return rows


def julian_date(epoch):
    """'2000-01-01.5' (TDB, fractional day) to a Julian Date."""
    y, m, d = epoch.split("-")
    year, month, day = int(y), int(m), float(d)
    if month <= 2:
        year, month = year - 1, month + 12
    a = year // 100
    b = 2 - a + a // 4
    return int(365.25 * (year + 4716)) + int(30.6001 * (month + 1)) + day + b - 1524.5


def published(value):
    """A table value as published, or empty if the table has none."""
    return "" if value in ("", "-", "n/a") else value


def main():
    elements = table(fetch(ELEMENTS), "sat_elem")
    header, rows = elements[0], elements[1:]
    columns = [h.replace(" ", "") for h in header]
    assert columns[:8] == ["ID", "Planet", "Satellite", "Code", "Ephemeris", "Frame", "Epoch(TDB)", "a(km)"], header
    assert columns[13:18] == ["P(days)", "Papsis(yr)", "Pnode(yr)", "R.A.(deg)", "Dec.(deg)"], header
    listed = len(rows)
    by_code = {}
    for row in rows:
        by_code.setdefault(int(row[3]), []).append(row)

    # JPL's measured GMs and radii: the first number in each cell is the value.
    physical = {}
    for row in table(fetch(PHYSICAL), "sat_phys_par")[2:]:
        first = lambda cell: published(cell.split()[0]) if cell else ""
        physical[int(row[2])] = (first(row[3]), first(row[4]))
    radii = mean_radii()

    small = [code for code, entries in by_code.items() if code not in MAJOR and entries[0][1] in HOSTS]
    print(f"{listed} rows in JPL's list, {len(by_code)} moons, {len(small)} beyond the major ones")

    chosen = {}
    states_by_code = {}
    sources_by_code = {}
    sources = set()
    missing = []
    for k, code in enumerate(sorted(small)):
        entries = by_code[code]
        source = ""
        for attempt in range(5):
            try:
                source, fetched = states(code, f"500@{PLANET_IDS[entries[0][1]]}")
                # A number Horizons doesn't know as a moon, it reads as an
                # asteroid's: accept only answers from this planet's moons.
                prefix = entries[0][1][:3].lower()
                if not source.lower().startswith(prefix):
                    raise NoEphemeris(f"Horizons answered with {source}, not a {entries[0][1]} satellite ephemeris")
                sources.add(source)
                sources_by_code[code] = source
                states_by_code[code] = fetched
                break
            except NoEphemeris as gap:
                print(f"  no 2025 state for {entries[0][2]} ({code}): {gap}")
                missing.append(entries[0][2])
                break
            except Exception as error:  # Horizons occasionally drops a request
                print(f"  retrying {code}: {error}")
                time.sleep(5 * (attempt + 1))
        else:
            raise SystemExit(f"Horizons kept failing for {code}")
        matching = [e for e in entries if source and e[4].lower() in source.lower()]
        chosen[code] = (matching or sorted(entries, key=lambda e: e[6]))[-1]
        if k % 25 == 0:
            print(f"  {k + 1}/{len(small)}: {chosen[code][2]} ({source})")
    for code in MAJOR:
        if code in by_code:
            chosen[code] = by_code[code][-1]

    # Elements of every moon in the list, inner moons first within each planet.
    order = sorted(chosen, key=lambda c: (["Earth"] + HOSTS).index(chosen[c][1]) * 1e12 + float(chosen[c][7]))
    element_rows = []
    for code in order:
        r = chosen[code]
        epoch = julian_date(r[6])
        element_rows.append(",".join([
            r[2], str(code), r[1], r[4], r[5], f"{epoch}", *[published(v) for v in r[7:16]],
            published(r[16]), published(r[17]),
        ]))
    write(DATA / "moon-elements.csv", [
        "# Mean orbital elements of every planetary satellite in JPL's list. Regenerate with: python tools/fetch_small_moons.py",
        f"# source: JPL Solar System Dynamics, Planetary Satellite Mean Elements, {ELEMENTS}, fetched {date.today()}",
        f"# satellites: {listed} rows in JPL's list, {len(by_code)} after merging duplicate entries for one moon",
        "#   (the entry from the ephemeris Horizons serves is kept)",
        "# frame: reference plane of the elements: ecliptic (J2000 ecliptic), Laplace (the local Laplace plane,",
        "#   pole at laplace_ra_deg / laplace_dec_deg in ICRF) or equatorial (the planet's equator). The node is",
        "#   measured from the reference plane's ascending node on the ICRF equator.",
        "# epoch_jd_tdb: the element epoch, converted from JPL's calendar date (TDB)",
        "# period_days: sidereal period; apsis/node_period_yr: precession periods (empty where JPL gives none)",
        "name,naif_id,parent,ephemeris,frame,epoch_jd_tdb,a_km,e,w_deg,m_deg,i_deg,node_deg,period_days,"
        "apsis_period_yr,node_period_yr,laplace_ra_deg,laplace_dec_deg",
        *element_rows,
    ])

    # States of the small moons, in the same order.
    for jd, (file_name, label) in EPOCHS.items():
        lines = [
            "# Worldline small moons: every known moon of Mars through Pluto beyond the major ones."
            " Regenerate with: python tools/fetch_small_moons.py",
            f"# epoch_jd_tdb: {jd}",
            f"# epoch: {label}",
            f"# states: NASA JPL Horizons ({', '.join(sorted(sources))}), fetched {date.today()}",
            "# frame: ICRF, ecliptic and mean equinox of J2000, origin at the planet's center",
            "# units: km, km/s, km^3/s^2",
            f"# gm: JPL's measured values ({PHYSICAL}), empty where none is published",
            "# radius: IAU mean radius (pck00011), else JPL's mean radius, empty where none is published",
            f"# without a state (Horizons has no ephemeris covering 2025): {', '.join(missing) or 'none'}",
            "# parent: the planetary system's name in the solar-system snapshot",
            "name,naif_id,parent,gm_km3_s2,radius_km,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s",
        ]
        for code in order:
            if code not in states_by_code:
                continue
            r = chosen[code]
            gm, jpl_radius = physical.get(code, ("", ""))
            radius = f"{radii[code]:.4f}" if code in radii else jpl_radius
            lines.append(",".join([r[2], str(code), r[1], gm, radius, *states_by_code[code][jd]]))
        write(DATA / file_name.replace("moons-", "small-moons-"), lines)

    # Small moons inside the innermost major moon that the major moons'
    # own JPL integration gives a mass.
    mass_rows = []
    integrations = []
    for host, ephemeris, planet_id, moons in SYSTEMS:
        integration, gms, *_ = ephemeris_constants(ephemeris, planet_id, [i for _, i in moons])
        integrations.append(f"{integration} in {ephemeris}.cmt")
        innermost = min(float(chosen[i][7]) for _, i in moons)
        for code, gm in sorted(gms.items()):
            if code in MAJOR or code == planet_id or code not in chosen or float(gm) <= 0:
                continue
            if float(chosen[code][7]) < innermost:
                mass_rows.append(f"{host},{chosen[code][2]},{code},{gm}")
    # Each planet's oblateness as its innermost regular small moon's JPL
    # solution has it.
    zonal_rows = []
    for host, _, planet_id, _ in SYSTEMS:
        regular = [c for c in states_by_code if chosen[c][1] == host and chosen[c][5] != "ecliptic"]
        if not regular:
            continue
        innermost = min(regular, key=lambda c: float(chosen[c][7]))
        family = sources_by_code[innermost].split("_merged")[0].split("_")[0].lower()
        comment_file = COMMENT_FILES.get(family, family)
        same = [c for c in regular if sources_by_code[c].lower().startswith(family)]
        integration, _, zonal, *_rest, radius = ephemeris_constants(comment_file, planet_id, same)
        for degree in sorted(zonal):
            zonal_rows.append(f"{host},{integration},{radius},{degree},{zonal[degree]}")
    write(DATA / "small-moon-zonal.csv", [
        "# Planets' oblateness (unnormalized J_n) as the JPL solution of each planet's innermost regular small",
        "# moon has it. Regenerate with: python tools/fetch_small_moons.py",
        f"# source: the NASA NAIF comment files of those solutions; fetched {date.today()}",
        "planet,solution,reference_radius_km,degree,j",
        *zonal_rows,
    ])

    write(DATA / "inner-moon-masses.csv", [
        "# Small moons that orbit inside a planet's innermost major moon and that JPL's integration of the",
        "# major moons includes with a mass. Regenerate with: python tools/fetch_small_moons.py",
        f"# source: the NASA NAIF comment files: {', '.join(integrations)}; fetched {date.today()}",
        "planet,name,naif_id,gm_km3_s2",
        *mass_rows,
    ])


if __name__ == "__main__":
    main()
