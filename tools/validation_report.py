"""Draws the validation report's plots (step 8.2).

Each validation test records what it checked as a figure, in
docs/validation/data/<id>.json, when WORLDLINE_REPORT is set. This script
draws each as docs/validation/<id>.svg and writes docs/validation/README.md,
which lists them with the tests they come from. See docs/validation/README.md.

  python tools/validation_report.py          draws the plots from the data
  python tools/validation_report.py --run    runs every test first, the slow
                                             ones too, recording figures:
                                             about two hours, and a GPU

Standard library only.
"""

import json
import math
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FOLDER = ROOT / "docs" / "validation"
DATA = FOLDER / "data"

WIDTH = 760
FONT = "font-family=\"Segoe UI, Helvetica, Arial, sans-serif\""
INK = "#1f2933"
MUTED = "#6b7785"
GRID = "#e4e7eb"
PASS = "#2f9e44"
FAIL = "#d9480f"
LIMIT = "#c92a2a"
COLORS = ["#1c7ed6", "#e8590c", "#2b8a3e", "#9c36b5", "#c2255c", "#0b7285", "#5c940d"]


def escape(text):
    return (
        str(text).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")
    )


def text(x, y, content, size=12, anchor="start", color=INK, weight="normal"):
    return (
        f'<text x="{x:.1f}" y="{y:.1f}" {FONT} font-size="{size}" fill="{color}" '
        f'text-anchor="{anchor}" font-weight="{weight}">{escape(content)}</text>'
    )


def power(x, y, p, anchor):
    """10 to the power p, as a tick label: 1 and 10 written out."""
    if p in (0, 1):
        return text(x, y, "1" if p == 0 else "10", 11, anchor, MUTED)
    exponent = str(p).replace("-", "−")
    return (
        f'<text x="{x:.1f}" y="{y:.1f}" {FONT} font-size="11" fill="{MUTED}" text-anchor="{anchor}">10'
        f'<tspan dy="-5" font-size="8">{exponent}</tspan></text>'
    )


def svg(width, height, body):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}">\n<rect width="100%" height="100%" fill="white"/>\n'
        + "\n".join(body)
        + "\n</svg>\n"
    )


def wrap(content, width):
    """Splits text into lines of at most `width` characters."""
    lines, line = [], ""
    for word in str(content).split():
        if line and len(line) + 1 + len(word) > width:
            lines.append(line)
            line = word
        else:
            line = f"{line} {word}".strip()
    if line:
        lines.append(line)
    return lines


def margins(figure):
    """For each check, its distance from what's expected as a share of its
    limit, on a logarithmic scale: 1, the limit, at the red line."""
    checks = figure["checks"]
    shares = [c["off"] / c["limit"] for c in checks if c["limit"] > 0 and c["off"] > 0]
    low = min([-6] + [math.floor(math.log10(s)) for s in shares])
    low = max(low, -16)
    high = max([1] + [math.ceil(math.log10(s)) for s in shares])
    left, right = 250, WIDTH - 30
    # Many checks: one line of label each, closer together.
    compact = len(checks) > 16
    row, top = (34, 64) if compact else (46, 64)
    height = top + row * len(checks) + 56
    body = [text(20, 30, figure["title"], 16, weight="bold")]
    body.append(
        text(20, 50, "how far each result is from what's expected, as a share of the limit it must stay within", 12, color=MUTED)
    )

    def at(share):
        value = math.log10(share) if share > 0 else low
        value = min(max(value, low), high)
        return left + (value - low) / (high - low) * (right - left)

    axis = top + row * len(checks) + 8
    for p in range(low, high + 1):
        x = at(10.0**p)
        body.append(f'<line x1="{x:.1f}" y1="{top - 6}" x2="{x:.1f}" y2="{axis}" stroke="{GRID}"/>')
        body.append(power(x, axis + 16, p, "middle"))
    limit_x = at(1.0)
    body.append(
        f'<line x1="{limit_x:.1f}" y1="{top - 10}" x2="{limit_x:.1f}" y2="{axis}" stroke="{LIMIT}" stroke-width="2"/>'
    )
    body.append(text(limit_x, top - 14, "limit", 11, "middle", LIMIT))
    for k, c in enumerate(checks):
        y = top + row * k
        share = c["off"] / c["limit"] if c["limit"] > 0 else float("inf")
        color = PASS if share <= 1 else FAIL
        x = at(share) if share > 0 else left
        label = wrap(c["label"], 36)
        if compact and len(label) > 1:
            label = [label[0] + " …"]
        for j, line in enumerate(label[:2]):
            body.append(text(20, y + 14 + 13 * j, line, 12))
        body.append(f'<rect x="{left}" y="{y + 4}" width="{max(x - left, 1.5):.1f}" height="14" fill="{color}" rx="2"/>')
        if share == 0:
            body.append(text(left + 6, y + 15, "exact", 11, color=MUTED))
        shown = c["shown"] if len(c["shown"]) <= 86 else c["shown"][:85] + "…"
        body.append(text(left, y + (30 if compact else 34), shown, 11, color=MUTED))
    body.append(text(left, axis + 34, "share of the limit (logarithmic): under 1 passes", 11, color=MUTED))
    return svg(WIDTH, height, body)


def nice_ticks(low, high, count=6):
    span = high - low
    if span <= 0:
        return [low]
    raw = span / count
    magnitude = 10 ** math.floor(math.log10(raw))
    step = min((m * magnitude for m in (1, 2, 2.5, 5, 10) if m * magnitude >= raw), default=magnitude * 10)
    start = math.ceil(low / step) * step
    ticks, t = [], start
    while t <= high + 1e-12 * abs(step):
        ticks.append(round(t, 12))
        t += step
    return ticks


def tick_label(value):
    if value == 0:
        return "0"
    a = abs(value)
    if a >= 1e5 or a < 1e-3:
        return f"{value:.3g}"
    return f"{value:.6g}"


def lines(figure):
    """Curves: simulated solid, expected dashed, samples as dots."""
    left, right, top, bottom = 90, WIDTH - 30, 64, 400
    note = wrap(figure.get("note", ""), 110)
    # A series without a name (a second limit line, say) isn't in the legend.
    named = [s for s in figure["series"] if s["name"]]
    legend_rows = len(named)
    height = bottom + 60 + 16 * legend_rows + 16 * len(note)
    log_x, log_y = figure.get("log_x"), figure.get("log_y")

    def scale(v, log):
        return math.log10(v) if log else v

    points = [
        (scale(x, log_x), scale(y, log_y))
        for s in figure["series"]
        for x, y in s["points"]
        if (not log_x or x > 0) and (not log_y or y > 0)
    ]
    if not points:
        points = [(0, 0), (1, 1)]
    x0, x1 = min(p[0] for p in points), max(p[0] for p in points)
    y0, y1 = min(p[1] for p in points), max(p[1] for p in points)
    if x1 == x0:
        x0, x1 = x0 - 1, x1 + 1
    if y1 == y0:
        pad = abs(y0) * 0.1 or 1
        y0, y1 = y0 - pad, y1 + pad
    pad = 0.05 * (y1 - y0)
    y0, y1 = y0 - pad, y1 + pad

    def px(x):
        return left + (scale(x, log_x) - x0) / (x1 - x0) * (right - left)

    def py(y):
        return bottom - (scale(y, log_y) - y0) / (y1 - y0) * (bottom - top)

    body = [text(20, 30, figure["title"], 16, weight="bold")]
    for t in (range(math.ceil(x0), math.floor(x1) + 1) if log_x else nice_ticks(x0, x1)):
        x = left + (t - x0) / (x1 - x0) * (right - left)
        body.append(f'<line x1="{x:.1f}" y1="{top}" x2="{x:.1f}" y2="{bottom}" stroke="{GRID}"/>')
        if log_x:
            body.append(power(x, bottom + 16, t, "middle"))
        else:
            body.append(text(x, bottom + 16, tick_label(t), 11, "middle", MUTED))
    for t in (range(math.ceil(y0), math.floor(y1) + 1) if log_y else nice_ticks(y0, y1)):
        y = bottom - (t - y0) / (y1 - y0) * (bottom - top)
        body.append(f'<line x1="{left}" y1="{y:.1f}" x2="{right}" y2="{y:.1f}" stroke="{GRID}"/>')
        if log_y:
            body.append(power(left - 6, y + 4, t, "end"))
        else:
            body.append(text(left - 6, y + 4, tick_label(t), 11, "end", MUTED))
    body.append(f'<rect x="{left}" y="{top}" width="{right - left}" height="{bottom - top}" fill="none" stroke="{MUTED}"/>')
    body.append(text((left + right) / 2, bottom + 36, figure["x"], 12, "middle"))
    body.append(
        f'<text x="18" y="{(top + bottom) / 2:.1f}" {FONT} font-size="12" fill="{INK}" text-anchor="middle" '
        f'transform="rotate(-90 18 {(top + bottom) / 2:.1f})">{escape(figure["y"])}</text>'
    )
    k = -1
    for s in figure["series"]:
        # An unnamed series continues the one before: its color, no legend.
        if s["name"] or k < 0:
            k += 1
        color = COLORS[k % len(COLORS)]
        kept = [(x, y) for x, y in s["points"] if (not log_x or x > 0) and (not log_y or y > 0)]
        if s.get("dots"):
            for x, y in kept:
                body.append(f'<circle cx="{px(x):.1f}" cy="{py(y):.1f}" r="2.6" fill="{color}"/>')
        elif kept:
            path = " ".join(f"{'M' if j == 0 else 'L'}{px(x):.1f},{py(y):.1f}" for j, (x, y) in enumerate(kept))
            dash = ' stroke-dasharray="6 4"' if s.get("expected") else ""
            body.append(f'<path d="{path}" fill="none" stroke="{color}" stroke-width="1.8"{dash}/>')
        if not s["name"]:
            continue
        y = bottom + 56 + 16 * k
        if s.get("dots"):
            body.append(f'<circle cx="{left + 10}" cy="{y - 4}" r="3" fill="{color}"/>')
        else:
            dash = ' stroke-dasharray="6 4"' if s.get("expected") else ""
            body.append(f'<line x1="{left}" y1="{y - 4}" x2="{left + 20}" y2="{y - 4}" stroke="{color}" stroke-width="2"{dash}/>')
        body.append(text(left + 28, y, s["name"], 12))
    for j, line in enumerate(note):
        body.append(text(20, bottom + 60 + 16 * legend_rows + 16 * j, line, 11, color=MUTED))
    return svg(WIDTH, height, body)


def readme_order():
    """A sort key: figures in the order the README links them, the rest
    after, by id."""
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    order = re.findall(r"\]\(docs/validation/([a-z0-9-]+)\.svg\)", readme)
    place = {}
    for k, figure in enumerate(order):
        place.setdefault(figure, k)
    return lambda f: (place.get(f["id"], len(order)), f["id"])


def run_tests():
    env = dict(os.environ, WORLDLINE_REPORT="1")
    command = ["cargo", "test", "--release", "--workspace", "--no-fail-fast", "--", "--include-ignored"]
    print("running", " ".join(command), "(about two hours)")
    return subprocess.run(command, cwd=ROOT, env=env).returncode


def main():
    if "--run" in sys.argv[1:] and run_tests() != 0:
        print("some tests failed: their figures may be stale")
    figures = []
    for path in sorted(DATA.glob("*.json")):
        figure = json.loads(path.read_text(encoding="utf-8"))
        drawn = margins(figure) if figure["kind"] == "margins" else lines(figure)
        (FOLDER / f"{figure['id']}.svg").write_text(drawn, encoding="utf-8", newline="\n")
        figures.append(figure)
    index = [
        "# Validation plots",
        "",
        "Step 8.2. Every claim in the [README](../../README.md#validated-against-reality) links to the test that checks "
        "it and to a plot here, drawn from what that test measured the last time it ran. This page is written by "
        "`tools/validation_report.py`; don't edit it by hand.",
        "",
        "## Reading them",
        "",
        "- **Margin plots**: one bar for each check a test makes, its result's distance from what's expected as a share "
        "of the limit it must stay within, on a logarithmic scale. Under 1, left of the red line, passes. Under each "
        "bar, the result itself. A bar past the line, in orange, is a check the test allows to miss and says so: the "
        "small moons' listed residuals, and the one disruption threshold Leinhardt & Stewart's \"usually\" allows.",
        "- **Curves**: what was simulated or measured, solid or as dots, against what's expected, dashed, with the "
        "limits as dashed lines where they're a curve's.",
        "",
        "Every limit is the test's own, from theory, rounding or the roadmap; the line under each curve says which.",
        "",
        "## How they're made",
        "",
        "1. Each validation test describes what it checked as a figure, with the small `worldline-report` crate, and saves "
        "it. When the `WORLDLINE_REPORT` environment variable is set, saving writes the figure's numbers to "
        "`data/<id>.json`, with the test's name and file; otherwise it does nothing, so ordinary test runs change no files.",
        "2. `python tools/validation_report.py` draws each as `<id>.svg` and writes this page. It uses Python's standard "
        "library only.",
        "3. `cargo test -p worldline-report` checks the README: every row links at least one test and one plot, every "
        "plot exists with its data, and the data names a test function in a file the row links.",
        "",
        "To draw them all again: `python tools/validation_report.py --run` runs every test with `WORLDLINE_REPORT` set, "
        "the slow ones too (about two hours, and a GPU), then draws. Without `--run` it only draws.",
        "",
        "## The plots",
        "",
        "In the README's order.",
        "",
    ]
    for figure in sorted(figures, key=readme_order()):
        test = figure["test"].split("::")[-1]
        index += [
            f"### {figure['title']}",
            "",
            f"`{figure['id']}`, from [`{test}`](../../{figure['file']}).",
            "",
            f"![{escape(figure['title'])}]({figure['id']}.svg)",
            "",
        ]
    (FOLDER / "README.md").write_text("\n".join(index), encoding="utf-8", newline="\n")
    print(f"drew {len(figures)} plots in {FOLDER}")


if __name__ == "__main__":
    main()
