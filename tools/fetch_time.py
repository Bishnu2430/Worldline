"""Fetch the tables that turn the computer's clock (UTC) into Worldline's
time (TDB) and Earth's rotation (UT1).

- Leap seconds (TAI - UTC from each date on): IERS Bulletin C, as IANA
  republishes it in leap-seconds.list, with the date it is valid until.
- UT1 - UTC, daily: the IERS Rapid Service/Prediction Center's
  finals2000A.data (IERS Bulletin A), measured and then predicted about a
  year ahead. Only the days from 2024-12-01 on are kept: Worldline starts
  on 2025-01-01.

Writes leap-seconds.csv and ut1-utc.csv under crates/worldline-data/data/.

Run from anywhere:  python tools/fetch_time.py
Uses only the Python standard library.
"""

import urllib.request
from datetime import date, timedelta
from pathlib import Path

LEAP_SECONDS = "https://data.iana.org/time-zones/data/leap-seconds.list"
FINALS = "https://datacenter.iers.org/products/eop/rapid/standard/finals2000A.data"
OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data"

# NTP time counts seconds from 1900-01-01, which is MJD 15020.
NTP_EPOCH_MJD = 15020
# The first day kept of UT1 - UTC: 2024-12-01.
FIRST_MJD = 60645


def fetch(url):
    with urllib.request.urlopen(url, timeout=120) as response:
        return response.read().decode()


def mjd_date(mjd):
    return date(1858, 11, 17) + timedelta(days=mjd)


def write(name, lines):
    path = OUT_DIR / name
    # Always LF line endings, even on Windows, so files are identical everywhere.
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines) + "\n")
    print(f"Wrote {path}")


def leap_seconds():
    text = fetch(LEAP_SECONDS)
    rows, updated, expires = [], None, None
    for line in text.splitlines():
        if line.startswith("#$"):
            updated = mjd_date(NTP_EPOCH_MJD + int(line.split()[1]) // 86400)
        elif line.startswith("#@"):
            expires = mjd_date(NTP_EPOCH_MJD + int(line.split()[1]) // 86400)
        elif line and not line.startswith("#"):
            ntp, seconds = line.split()[:2]
            mjd = NTP_EPOCH_MJD + int(ntp) // 86400
            rows.append(f"{mjd},{seconds}")
    write(
        "leap-seconds.csv",
        [
            "# Worldline leap seconds: TAI - UTC from each day on. Regenerate with: python tools/fetch_time.py",
            f"# source: IERS Bulletin C, as IANA republishes it ({LEAP_SECONDS})",
            f"# updated: {updated}; valid until: {expires}; fetched {date.today()}",
            "# mjd_utc: the Modified Julian Date (UTC) the value holds from, at 0h",
            "mjd_utc,tai_minus_utc_s",
            *rows,
        ],
    )


def ut1_minus_utc():
    rows = []
    for line in fetch(FINALS).splitlines():
        # finals2000A columns (1-based): MJD 8-15, UT1-UTC flag 58
        # (I = IERS, P = predicted), UT1-UTC 59-68 in seconds.
        if len(line) < 68 or line[57] not in "IP" or not line[58:68].strip():
            continue
        mjd = int(float(line[7:15]))
        if mjd >= FIRST_MJD:
            rows.append(f"{mjd},{line[58:68].strip()},{line[57]}")
    first, last = rows[0].split(",")[0], rows[-1].split(",")[0]
    measured = [r for r in rows if r.endswith(",I")]
    write(
        "ut1-utc.csv",
        [
            "# Worldline UT1 - UTC, daily. Regenerate with: python tools/fetch_time.py",
            f"# source: IERS Rapid Service/Prediction Center, finals2000A.data, IERS Bulletin A ({FINALS})",
            f"# fetched {date.today()}: {mjd_date(int(first))} to {mjd_date(int(last))}, measured (I) to "
            f"{mjd_date(int(measured[-1].split(',')[0]))}, predicted (P) after",
            "# mjd_utc: the Modified Julian Date (UTC), at 0h",
            "mjd_utc,ut1_minus_utc_s,flag",
            *rows,
        ],
    )


def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    leap_seconds()
    ut1_minus_utc()


if __name__ == "__main__":
    main()
