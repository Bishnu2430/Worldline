"""Fetch the dwarf planets, major asteroids and comets Worldline ships with.

Writes, under crates/worldline-data/data/:

- small-bodies-2025-01-01.csv: each body's position and velocity (NASA JPL
  Horizons, relative to the solar system barycenter, like the planets'
  snapshot), its GM where JPL's planetary ephemeris DE440 uses one (the
  bodies heavy enough to pull on the planets), its mean radius where the IAU
  (pck00011) or JPL gives one, and the non-gravitational force model JPL
  fitted to its orbit, if any: comets' outgassing, and the Yarkovsky drift
  of some asteroids.
- small-bodies-2026-01-01T06.csv: the same, exactly one Julian year later,
  the epoch of the planets' validation reference.
- The JPL-predicted 2061 perihelion of Halley's Comet, in the first file's
  header.

The model parameters come from the same Horizons record as the states, so
the two are consistent. Run tools/fetch_rotation.py first (for radii).

Run from anywhere:  python tools/fetch_small_bodies.py
Uses only the Python standard library.
"""

import json
import re
import urllib.parse
from datetime import date

from fetch_moons import DATA, HORIZONS, fetch, mean_radii, write

GM_FILE = "https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/gm_de440.tpc"
START, ONE_YEAR_ON = 2460676.5, 2461041.75
FILES = {START: ("small-bodies-2025-01-01.csv", "2025-01-01 00:00:00 TDB"),
         ONE_YEAR_ON: ("small-bodies-2026-01-01T06.csv", "2026-01-01 06:00:00 TDB")}

# (name, kind, Horizons command, asteroid number, or minus the NAIF id of a
# comet or interstellar object as JPL's small-body database gives it)
BODIES = [
    # Dwarf planets, and the largest trans-Neptunian objects.
    ("Ceres", "dwarf planet", "1;", 1),
    ("Eris", "dwarf planet", "136199;", 136199),
    ("Haumea", "dwarf planet", "136108;", 136108),
    ("Makemake", "dwarf planet", "136472;", 136472),
    ("Gonggong", "trans-Neptunian object", "225088;", 225088),
    ("Quaoar", "trans-Neptunian object", "50000;", 50000),
    ("Orcus", "trans-Neptunian object", "90482;", 90482),
    ("Sedna", "trans-Neptunian object", "90377;", 90377),
    ("Arrokoth", "trans-Neptunian object", "486958;", 486958),
    # The asteroids massive enough that JPL's planetary ephemeris (DE440)
    # includes their pull, besides Ceres.
    ("Pallas", "asteroid", "2;", 2),
    ("Juno", "asteroid", "3;", 3),
    ("Vesta", "asteroid", "4;", 4),
    ("Iris", "asteroid", "7;", 7),
    ("Hygiea", "asteroid", "10;", 10),
    ("Eunomia", "asteroid", "15;", 15),
    ("Psyche", "asteroid", "16;", 16),
    ("Euphrosyne", "asteroid", "31;", 31),
    ("52 Europa", "asteroid", "52;", 52),
    ("Cybele", "asteroid", "65;", 65),
    ("Sylvia", "asteroid", "87;", 87),
    ("Thisbe", "asteroid", "88;", 88),
    ("Camilla", "asteroid", "107;", 107),
    ("Davida", "asteroid", "511;", 511),
    ("Interamnia", "asteroid", "704;", 704),
    # Asteroids visited by spacecraft, or famous for their orbits.
    ("Lutetia", "asteroid", "21;", 21),
    ("Ida", "asteroid", "243;", 243),
    ("Mathilde", "asteroid", "253;", 253),
    ("Eros", "asteroid", "433;", 433),
    ("Patroclus", "asteroid", "617;", 617),
    ("Gaspra", "asteroid", "951;", 951),
    ("Icarus", "asteroid", "1566;", 1566),
    ("Steins", "asteroid", "2867;", 2867),
    ("Phaethon", "asteroid", "3200;", 3200),
    ("Eurybates", "asteroid", "3548;", 3548),
    ("Toutatis", "asteroid", "4179;", 4179),
    ("Annefrank", "asteroid", "5535;", 5535),
    ("Braille", "asteroid", "9969;", 9969),
    ("Itokawa", "asteroid", "25143;", 25143),
    ("Donaldjohanson", "asteroid", "52246;", 52246),
    ("Didymos", "asteroid", "65803;", 65803),
    ("Apophis", "asteroid", "99942;", 99942),
    ("Bennu", "asteroid", "101955;", 101955),
    ("Dinkinesh", "asteroid", "152830;", 152830),
    ("Ryugu", "asteroid", "162173;", 162173),
    # Comets: famous periodic ones, recent great comets, and the three
    # known interstellar visitors.
    ("Halley", "comet", "DES=1P;CAP;NOFRAG;", -1000036),
    ("Encke", "comet", "DES=2P;CAP;NOFRAG;", -1000025),
    ("Tempel 1", "comet", "DES=9P;CAP;NOFRAG;", -1000093),
    ("Pons-Brooks", "comet", "DES=12P;CAP;NOFRAG;", -1000068),
    ("Borrelly", "comet", "DES=19P;CAP;NOFRAG;", -1000005),
    ("Giacobini-Zinner", "comet", "DES=21P;CAP;NOFRAG;", -1000032),
    ("Wirtanen", "comet", "DES=46P;CAP;NOFRAG;", -1000109),
    ("Tempel-Tuttle", "comet", "DES=55P;CAP;NOFRAG;", -1000095),
    ("Churyumov-Gerasimenko", "comet", "DES=67P;CAP;NOFRAG;", -1000012),
    ("Wild 2", "comet", "DES=81P;CAP;NOFRAG;", -1000107),
    ("Hartley 2", "comet", "DES=103P;CAP;NOFRAG;", -1000041),
    ("Swift-Tuttle", "comet", "DES=109P;CAP;NOFRAG;", -1000140),
    ("Hale-Bopp", "comet", "DES=C/1995 O1;CAP;", -1000132),
    ("NEOWISE", "comet", "DES=C/2020 F3;CAP;", -1003667),
    ("Tsuchinshan-ATLAS", "comet", "DES=C/2023 A3;CAP;", -1003913),
    # 'Oumuamua is catalogued as an asteroid, A/2017 U1.
    ("'Oumuamua", "interstellar object", "DES=A/2017 U1;", -50788063),
    ("Borisov", "interstellar object", "DES=2I;CAP;", -1003639),
    ("3I/ATLAS", "interstellar object", "DES=3I;CAP;", -1004083),
]

# Non-gravitational model parameters Horizons lists, with their defaults
# (the Marsden, Sekanina & Yeomans 1973 water-ice g(r)).
MODEL = ["A1", "A2", "A3", "DT", "ALN", "NM", "NN", "NK", "R0"]


def horizons(command, **extra):
    params = {
        "format": "json",
        "COMMAND": f"'{command}'",
        "OBJ_DATA": "'YES'",
        "MAKE_EPHEM": "'YES'",
        "CENTER": "'500@0'",
        "REF_PLANE": "'ECLIPTIC'",
        "REF_SYSTEM": "'ICRF'",
        "OUT_UNITS": "'KM-S'",
        "CSV_FORMAT": "'YES'",
        "TIME_TYPE": "'TDB'",
    }
    params.update({k: f"'{v}'" for k, v in extra.items()})
    return json.loads(fetch(HORIZONS + "?" + urllib.parse.urlencode(params)))["result"]


def gms():
    text = fetch(GM_FILE)
    return {int(m.group(1)): m.group(2) for m in re.finditer(r"BODY(\d+)_GM\s*=\s*\(\s*([-+0-9.EeDd]+)\s*\)", text)}


def main():
    published_gm = gms()
    radii = mean_radii()
    rows = {jd: [] for jd in FILES}
    sources = []
    for name, kind, command, number in BODIES:
        result = horizons(command, EPHEM_TYPE="VECTORS", VEC_TABLE="2", START_TIME=f"JD{START}",
                          STOP_TIME=f"JD{ONE_YEAR_ON}", STEP_SIZE="1")
        source = re.search(r"Target body name: (.*?)\s*\{source: ([^}]+)\}", result)
        sources.append(f"{source.group(1).strip()} {source.group(2).strip()}")
        block = result.split("$$SOE")[1].split("$$EOE")[0]
        states = {}
        for line in block.strip().splitlines():
            fields = [f.strip() for f in line.split(",")]
            states[float(fields[0])] = fields[2:8]
        model = {m.group(1): m.group(2) for m in re.finditer(r"\b(A1|A2|A3|DT|ALN|NM|NN|NK|R0)=\s*([-+0-9.E]+)", result)}
        has_model = any(float(model.get(k, "0")) != 0.0 for k in ("A1", "A2", "A3"))
        # NAIF ids: 2000000 + number for asteroids in the rotation kernel;
        # DE440's GM file writes the trans-Neptunian systems as 20000000 + n.
        if number > 0:
            naif_id = 2000000 + number
            gm = published_gm.get(naif_id) or published_gm.get(20000000 + number) or ""
        else:
            naif_id, gm = -number, ""
        rad = re.search(r"\bRAD=\s*([0-9.]+)", result)
        radius = f"{radii[naif_id]:.4f}" if naif_id in radii else (rad.group(1) if rad else "")
        values = [model.get(k, "") if has_model else "" for k in MODEL]
        for jd in FILES:
            rows[jd].append(",".join([name, kind, str(naif_id), gm, radius, *states[jd], *values]))
        print(f"{name}: {sources[-1]}; GM {gm or '-'}; radius {radius or '-'}; model {'yes' if has_model else 'no'}")

    # JPL's own prediction of Halley's next perihelion: the perihelion time
    # of its osculating orbit in mid-2061.
    elements = horizons("DES=1P;CAP;NOFRAG;", EPHEM_TYPE="ELEMENTS", CENTER="500@10",
                        START_TIME="2061-07-01", STOP_TIME="2061-07-02", STEP_SIZE="1")
    line = elements.split("$$SOE")[1].split("$$EOE")[0].strip().splitlines()[0]
    header = [h.strip() for h in elements.split("$$SOE")[0].strip().splitlines()[-2].split(",")]
    perihelion = line.split(",")[header.index("Tp")].strip()
    print(f"Halley's perihelion predicted by JPL: JD {perihelion}")

    for jd, (file_name, label) in FILES.items():
        lines = [
            "# Worldline dwarf planets, major asteroids and comets. Regenerate with: python tools/fetch_small_bodies.py",
            f"# epoch_jd_tdb: {jd}",
            f"# epoch: {label}",
            f"# states: NASA JPL Horizons small-body solutions ({'; '.join(sources)}), fetched {date.today()}",
            "# frame: ICRF, ecliptic and mean equinox of J2000, origin at the solar system barycenter",
            "# units: km, km/s, km^3/s^2; a1-a3 in au/d^2, dt in days, r0 in au",
            f"# gm: JPL DE440 ({GM_FILE}), only for the bodies heavy enough that DE440 includes them",
            "# radius: IAU mean radius (pck00011), else the radius Horizons lists; empty where none",
            "# a1..r0: the non-gravitational force model of the same Horizons record (Marsden, Sekanina & Yeomans",
            "#   1973 form g(r) = aln (r/r0)^-nm (1 + (r/r0)^nn)^-nk); empty where the orbit has none",
            f"# jpl_halley_perihelion_jd_tdb: {perihelion}",
            "name,kind,naif_id,gm_km3_s2,radius_km,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s,"
            "a1,a2,a3,dt,aln,nm,nn,nk,r0",
            *rows[jd],
        ]
        write(DATA / file_name, lines)


if __name__ == "__main__":
    main()
