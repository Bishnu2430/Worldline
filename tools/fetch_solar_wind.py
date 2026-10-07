"""Fetch a year of hourly solar wind measurements at Earth.

Writes crates/worldline-data/data/solar-wind-2025.csv: NASA's OMNI hourly
solar wind data (King & Papitashvili 2005, J. Geophys. Res. 110, A02104),
for calendar year 2025, the year the simulation starts. OMNI merges
measurements from spacecraft upstream of Earth (ACE, Wind, DSCOVR),
shifted to Earth's bow shock. For each hour:

- the interplanetary magnetic field (nT) in GSE coordinates: x toward the
  Sun, y in the ecliptic opposite to Earth's motion, z to ecliptic north;
- the bulk speed of the solar wind (km/s);
- the proton density (per cm^3);
- the flow (dynamic) pressure (nPa), as OMNI computes it from the density
  and speed: (1.67e-6) Np Vp^2 (1 + 4 Na/Np) where the helium ratio was
  measured, and 2e-6 Np Vp^2 (4% helium) where it wasn't.

NASA's own OMNI servers (spdf.gsfc.nasa.gov) were unreachable when this was
written, so the data come from the same OMNI dataset as served by the French
CDPP's AMDA through the standard HAPI protocol. Its catalog record (SPASE,
spase://CNES/NumericalData/CDPP-AMDA/OMNI/omni-hour-all) names the field
vector "b_gse". Missing values are left empty.

Run from anywhere:  python tools/fetch_solar_wind.py
Uses only the Python standard library.
"""

import urllib.parse
from datetime import date

from fetch_moons import DATA, fetch, write

HAPI = "https://amda.irap.omp.eu/service/hapi/data"
DATASET = "omni-hour-all"
PARAMETERS = "omni_imf,omni_sw_v,omni_sw_n,omni_sw_ram"
START, STOP = "2025-01-01T00:00:00Z", "2026-01-01T00:00:00Z"
FILL = -1e31


def main():
    query = {"id": DATASET, "time.min": START, "time.max": STOP, "parameters": PARAMETERS}
    url = HAPI + "?" + urllib.parse.urlencode(query)
    rows = []
    for line in fetch(url).splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split(",")
        if len(fields) != 7:
            raise SystemExit(f"unexpected line: {line}")
        time, values = fields[0], fields[1:]
        if not time.startswith("2025"):
            continue  # HAPI may include the end of the range
        # Keep the published digits; mark fill values as missing.
        values = ["" if float(v) <= FILL / 10 else v for v in values]
        rows.append(",".join([time[:13]] + values))
    expected_hours = 365 * 24
    if len(rows) != expected_hours:
        raise SystemExit(f"expected {expected_hours} hours, got {len(rows)}")
    lines = [
        "# Worldline: a year of hourly solar wind at Earth. Regenerate with: python tools/fetch_solar_wind.py",
        "# source: NASA OMNI hourly data (King & Papitashvili 2005, J. Geophys. Res. 110, A02104),"
        " served by CDPP/AMDA (dataset omni-hour-all, provider CDAWeb) through HAPI",
        f"# query: {HAPI}?{urllib.parse.unquote(urllib.parse.urlencode(query))}, fetched {date.today()}",
        "# frame: magnetic field in GSE (x toward the Sun, y in the ecliptic opposite Earth's motion,"
        " z to ecliptic north), at Earth's bow shock",
        "# units: hour (UTC, start of the averaging hour); nT; km/s; protons per cm^3; nPa. Empty = missing",
        "hour_utc,bx_gse_nt,by_gse_nt,bz_gse_nt,speed_km_s,density_cm3,flow_pressure_npa",
    ]
    write(DATA / "solar-wind-2025.csv", lines + rows)
    missing = sum(1 for r in rows if ",," in r or r.endswith(","))
    print(f"  {len(rows)} hours, {missing} with a missing value")


if __name__ == "__main__":
    main()
