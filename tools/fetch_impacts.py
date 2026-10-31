"""Fetch the simulations behind the collision outcomes of step 6.1.

From Leinhardt & Stewart (2012), "Collisions between Gravity-dominated
Bodies. I. Outcome Regimes and Scaling Laws", ApJ 745, 79, as the accepted
manuscript on arXiv (1106.6084), read through ar5iv's HTML rendering of it,
which keeps the tables as text:

- Table 1: selected simulations, with each series' fitted disruption energy
  Q'*RD, the interacting mass fraction alpha, and the paper's own predicted
  largest and second-largest remnants.
- Table A.3: all the simulations, which holds the same runs and more.

Digits are kept as published. Writes impact-simulations-selected.csv and
impact-simulations-all.csv under crates/worldline-data/data/.

Run from anywhere:  python tools/fetch_impacts.py
Uses only the Python standard library. ar5iv can take a few minutes to answer.
"""

import html
import re
import urllib.request
from datetime import date
from pathlib import Path

URL = "https://ar5iv.labs.arxiv.org/html/1106.6084"
OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-data" / "data"
SOURCE = "Leinhardt & Stewart 2012, ApJ 745, 79 (arXiv:1106.6084), via " + URL


def fetch():
    request = urllib.request.Request(URL, headers={"User-Agent": "Worldline tools/fetch_impacts.py"})
    with urllib.request.urlopen(request, timeout=300) as response:
        return response.read().decode(errors="replace")


def tables(text):
    """Every data table, as rows of cells; equations become their LaTeX."""
    found = []
    for table in re.findall(r'<table[^>]*class="ltx_tabular[^"]*"[\s\S]*?</table>', text):
        rows = []
        for row in re.findall(r"<tr[\s\S]*?</tr>", table):
            cells = []
            for cell in re.findall(r"<t[dh][^>]*>([\s\S]*?)</t[dh]>", row):
                cell = re.sub(r'<math[^>]*alttext="([^"]*)"[\s\S]*?</math>', lambda m: m.group(1), cell)
                cells.append(re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", "", cell))).strip())
            rows.append(cells)
        found.append(rows)
    return found


def exponent(cell):
    """7.2\\times 10^{1} as 7.2e1, keeping the published digits."""
    m = re.fullmatch(r"([0-9.]+)\\times 10\^\{([0-9]+)\}", cell)
    return f"{m.group(1)}e{m.group(2)}" if m else cell


def mass_ratio(cell):
    """The published ratio, and the marks after it."""
    m = re.match(r"([0-9.]+)(.*)", cell)
    return m.group(1), marks(m.group(2))


def marks(text):
    """The paper's marks, in words safe for a CSV."""
    text = text.replace("{", "").replace("}", "").replace("^", "").replace("\\dagger", "†").replace("*", "∗")
    return text.replace("†", "d").replace("∗", "s").strip()


def selected(rows):
    out = []
    for cells in rows:
        if len(cells) < 11 or not re.match(r"[0-9]", cells[0]):
            continue
        ratio, note = mass_ratio(cells[0])
        predicted = re.match(r"[0-9.]*", cells[9]).group()
        predicted_note = marks(cells[9][len(predicted):])
        if cells[9] == "super-cat":
            predicted_note = "supercat"
        second = re.match(r"[0-9.]*", cells[10]).group()
        alpha = re.match(r"[0-9.]*", cells[8]).group()
        beta = cells[5] if cells[5] != "–" else ""
        out.append(",".join([ratio, cells[1], cells[2], cells[3], cells[4], beta, exponent(cells[6]),
                             exponent(cells[7]), alpha, predicted, second, note, predicted_note]))
    return out


def everything(rows):
    out = []
    for cells in rows:
        if len(cells) >= 6 and re.fullmatch(r"[0-9.]+", cells[0]):
            out.append(",".join(cells[:6]))
    return out


def write(name, lines):
    path = OUT_DIR / name
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines) + "\n")
    print(f"Wrote {path}: {len(lines)} lines")


def main():
    found = tables(fetch())
    first = found[0]
    assert "M_{\\rm p}" in first[0][0], first[0][:3]
    assert found[3][0][0] == first[0][0], found[3][0][:3]
    rows = selected(first)
    assert len(rows) == 48, len(rows)
    write(
        "impact-simulations-selected.csv",
        [
            "# Selected simulations of collisions between 10-km rubble piles: Table 1. Regenerate with: python tools/fetch_impacts.py",
            f"# source: {SOURCE}; fetched {date.today()}",
            "# target: radius 10 km, mass 4.2e15 kg, density 1000 kg/m^3 in every run",
            "# mass_ratio: projectile/target; b: impact parameter, the sine of the impact angle; speed: m/s; lr, slr: largest and second-largest remnant over the total mass",
            "# beta: slope of the fragments' size distribution; q_r: J/kg; q_star: the series' fitted Q'*RD, J/kg, printed on one row of each series of three; alpha: interacting mass fraction",
            "# predicted_lr, predicted_slr: the paper's own predictions from q_star",
            "# note, marks after the mass ratio: s, shown in the paper's Figure 5; ds, in its Figure 7",
            "# predicted_note: d, two equal remnants (N_lr = 2, N_slr = 4); ss, erosive hit-and-run, where the disruption model doesn't apply; supercat, super-catastrophic",
            "mass_ratio,b,speed_m_s,lr,slr,beta,q_r,q_star,alpha,predicted_lr,predicted_slr,note,predicted_note",
        ]
        + rows,
    )
    rows = sum((everything(table) for table in found[3:7]), [])
    assert len(rows) == 189, len(rows)
    write(
        "impact-simulations-all.csv",
        [
            "# All the simulations of collisions between 10-km rubble piles: Table A.3. Regenerate with: python tools/fetch_impacts.py",
            f"# source: {SOURCE}; fetched {date.today()}",
            "# columns as in impact-simulations-selected.csv",
            "mass_ratio,b,speed_m_s,lr,slr,q_r",
        ]
        + rows,
    )


if __name__ == "__main__":
    main()
