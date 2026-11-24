"""Builds Worldline's website, the landing page on GitHub Pages (step 8.7).

    python tools/build_site.py               # into target/site
    python tools/build_site.py --compress    # screenshots as WebP, fonts as WOFF2
    python tools/build_site.py --out DIR

The page, site/index.html, and its stylesheet are written by hand. This script:
- fills in the validation section from the README's table, so the site and the
  README can never disagree;
- copies every file the page uses next to it: screenshots from docs/images,
  plots from docs/validation, and the app's own typeface;
- turns `repo:` links into links to the repository on GitHub;
- gives each image its width and height, so the page doesn't jump as they load.

It fails if anything the page uses or links to in the repository is missing,
if a link inside the page leads nowhere, or if the README's table has a row it
can't read. `--compress` needs `cwebp` and `woff2_compress` (the Ubuntu
packages webp and woff2); the workflow that publishes the site installs them.

Python's standard library only. See docs/website.md.
"""

import argparse
import html
import re
import shutil
import struct
import subprocess
import sys
from html.parser import HTMLParser
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "site"
README = ROOT / "README.md"
REPOSITORY = "https://github.com/Bishnu2430/Worldline"
# The page's files, by the first folder of the path the page uses for them.
FOLDERS = {
    "images": ROOT / "docs" / "images",
    "validation": ROOT / "docs" / "validation",
    "fonts": ROOT / "crates" / "worldline-app" / "assets" / "fonts",
}
# Every page this script writes says so, and only such a folder is replaced.
GENERATOR = '<meta name="generator" content="tools/build_site.py">'
# How a status in the README's table begins: its badge's class and words.
STATUSES = [
    ("✅ passing", "pass", "passing"),
    ("🟨", "partly", "in part"),
    ("❌", "fail", "failing"),
]


def fail(message):
    sys.exit(f"build_site: {message}")


# --- The README's validation table ------------------------------------------


class Check:
    """One row of the README's table: a claim and its evidence."""

    def __init__(self, test, expected, status, evidence):
        self.test = test
        self.expected = expected
        for prefix, kind, words in STATUSES:
            if status.startswith(prefix):
                self.kind, self.words = kind, words
                self.measured = status[len(prefix) :].lstrip(" ,:")
                break
        else:
            fail(f"the README's row '{test[:60]}' has a status that isn't passing, in part or failing: {status[:40]}")
        self.tests, self.plots = [], []
        for label, target in re.findall(r"\[([^\]]+)\]\(([^)\s]+)\)", evidence):
            if label == "test":
                self.tests.append(target)
            elif label == "plot":
                self.plots.append(target)
            else:
                fail(f"the README's row '{test[:60]}' links '{label}', neither a test nor a plot")
        if not self.tests or not self.plots:
            fail(f"the README's row '{test[:60]}' needs a test and a plot")
        for target in self.tests + self.plots:
            if not (ROOT / target).is_file():
                fail(f"the README's row '{test[:60]}' links {target}, which isn't there")


def read_checks():
    text = README.read_text(encoding="utf-8")
    heading = "\n## Validated against reality\n"
    if heading not in text:
        fail("the README has no section 'Validated against reality'")
    section = text.split(heading, 1)[1].split("\n## ", 1)[0]
    rows = [line.strip() for line in section.splitlines() if line.startswith("|")]
    if len(rows) < 3 or not rows[0].startswith("| Test |"):
        fail("the README's validation table isn't where it should be")
    checks = []
    for row in rows[2:]:
        cells = [cell.strip() for cell in row.strip("|").split(" | ")]
        if len(cells) != 4:
            fail(f"a row of the README's table has {len(cells)} cells, not 4: {row[:80]}")
        checks.append(Check(*cells))
    return checks


def inline(text):
    """The little Markdown the README's cells use, as HTML."""
    out = html.escape(text, quote=False).replace("\\*", "\0")
    out = re.sub(r"`([^`]+)`", r"<code>\1</code>", out)
    out = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", out)
    out = re.sub(r"\*([^*]+)\*", r"<em>\1</em>", out)
    out = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", lambda m: f'<a href="{link(m.group(2))}">{m.group(1)}</a>', out)
    return out.replace("\0", "*")


def link(target):
    """A link from the README, which may be to the web or into the repository."""
    return target if re.match(r"https?:", target) else repository_link(target)


def tally(checks):
    """The sentence that counts them."""
    counts = {kind: sum(1 for c in checks if c.kind == kind) for _, kind, _ in STATUSES}
    parts = [f"{counts['pass']} of {len(checks)} pass in full"]
    if counts["partly"] == 1:
        parts.append("1 passes in part, and lists what it misses")
    elif counts["partly"]:
        parts.append(f"{counts['partly']} pass in part, and list what they miss")
    if counts["fail"]:
        parts.append(f"{counts['fail']} {'fails' if counts['fail'] == 1 else 'fail'}")
    return "; ".join(parts) + "."


def card(check, plot=None):
    """A check as a card: its plot, the claim, what's expected and what it measured."""
    plot = plot or check.plots[0]
    name = Path(plot).name
    title = svg_title(FOLDERS["validation"] / name)
    tests = ", ".join(f'<a href="{repository_link(t)}">{Path(t).name}</a>' for t in check.tests)
    plots = ", ".join(f'<a href="validation/{Path(p).name}">{Path(p).stem}</a>' for p in check.plots)
    return f"""<article class="check">
  <a class="plot" href="validation/{name}"><img src="validation/{name}" alt="{html.escape(title)}" loading="lazy"></a>
  <div class="check-body">
    <h3>{inline(check.test)}</h3>
    <p class="expected"><span class="label">Expected</span>{inline(check.expected)}</p>
    <p class="measured"><span class="label">Measured</span><span class="badge {check.kind}">{check.words}</span>{inline(check.measured)}</p>
    <p class="evidence">{'Tests' if len(check.tests) > 1 else 'Test'}: {tests} · {'Plots' if len(check.plots) > 1 else 'Plot'}: {plots}</p>
  </div>
</article>"""


# --- Files --------------------------------------------------------------------


def svg_size(path):
    head = path.read_text(encoding="utf-8")[:400]
    width = re.search(r'<svg[^>]*\bwidth="([\d.]+)"', head)
    height = re.search(r'<svg[^>]*\bheight="([\d.]+)"', head)
    if not width or not height:
        fail(f"{path.name} doesn't say its size")
    return round(float(width.group(1))), round(float(height.group(1)))


def svg_title(path):
    """A plot's title: its first line of text, as validation_report.py draws it."""
    match = re.search(r"<text[^>]*>([^<]+)</text>", path.read_text(encoding="utf-8"))
    return html.unescape(match.group(1)) if match else path.stem


def png_size(path):
    with open(path, "rb") as f:
        head = f.read(24)
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        fail(f"{path.name} isn't a PNG")
    return struct.unpack(">II", head[16:24])


def source_of(reference):
    """Where in the repository a file the page uses comes from."""
    path = reference.split("#")[0].split("?")[0]
    if ".." in path.split("/") or path.startswith("/"):
        fail(f"the page uses {reference}, outside the site")
    first, _, rest = path.partition("/")
    source = FOLDERS[first] / rest if rest and first in FOLDERS else SITE / path
    if not source.is_file():
        fail(f"the page uses {reference}, but {source.relative_to(ROOT)} isn't there")
    return source


def repository_link(target):
    """A link to a file or folder of the repository on GitHub, which must be there."""
    path, _, anchor = target.partition("#")
    path = path.strip("/")
    if not (ROOT / path).exists():
        fail(f"the page links {path} in the repository, which isn't there")
    kind = "tree" if (ROOT / path).is_dir() else "blob"
    return f"{REPOSITORY}/{kind}/main/{path}" + (f"#{anchor}" if anchor else "")


def local_references(page, style):
    """Every file the page and its stylesheet use, in the order they appear."""
    found = re.findall(r'\b(?:src|href)="([^"]+)"', page) + re.findall(r'url\("([^"]+)"\)', style)
    seen = []
    for reference in found:
        if re.match(r"[a-z]+:|#|//", reference):
            continue
        if reference not in seen:
            seen.append(reference)
    return seen


def sized(page):
    """Each image given its width and height, read from the file."""

    def size(match):
        tag = match.group(0)
        if " width=" in tag:
            return tag
        src = re.search(r'\bsrc="([^"]+)"', tag).group(1)
        source = source_of(src)
        width, height = svg_size(source) if source.suffix == ".svg" else png_size(source)
        return tag[:-1] + f' width="{width}" height="{height}">'

    return re.sub(r"<img\b[^>]*>", size, page)


class Anchors(HTMLParser):
    """The page's ids, and the links within it, to check none leads nowhere."""

    def __init__(self):
        super().__init__()
        self.ids, self.links = set(), set()

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name == "id":
                if value in self.ids:
                    fail(f"the page has two elements with id {value}")
                self.ids.add(value)
            if name == "href" and value and value.startswith("#"):
                self.links.add(value[1:])


def run(command):
    try:
        subprocess.run(command, check=True, capture_output=True)
    except FileNotFoundError:
        fail(f"--compress needs {command[0]} (Ubuntu: apt install webp woff2)")
    except subprocess.CalledProcessError as error:
        fail(f"{' '.join(command)} failed: {error.stderr.decode(errors='replace')[:300]}")


# --- The build ----------------------------------------------------------------


def build(out, compress):
    checks = read_checks()
    page = (SITE / "index.html").read_text(encoding="utf-8")
    style = (SITE / "style.css").read_text(encoding="utf-8")
    if GENERATOR not in page:
        fail("site/index.html lost its generator line, which marks what this script may replace")

    # The validation section, from the README.
    featured = re.search(r"<!-- FEATURED ([^>]*) -->", page)
    if not featured:
        fail("site/index.html has no <!-- FEATURED ... --> line")
    cards = []
    for name in featured.group(1).split():
        plot = f"docs/validation/{name}.svg"
        check = next((c for c in checks if plot in c.plots), None)
        if check is None:
            fail(f"no check in the README has the featured plot {name}")
        cards.append(card(check, plot))
    page = page.replace(featured.group(0), "\n".join(cards))
    if "<!-- VALIDATION -->" not in page:
        fail("site/index.html has no <!-- VALIDATION --> line")
    page = page.replace("<!-- VALIDATION -->", "\n".join(card(c) for c in checks))
    plots = {p for c in checks for p in c.plots}
    page = page.replace("{{CHECKS}}", str(len(checks)))
    page = page.replace("{{PLOTS}}", str(len(plots)))
    page = page.replace("{{TALLY}}", tally(checks))
    left = re.search(r"\{\{\w*\}\}", page)
    if left:
        fail(f"site/index.html has {left.group(0)}, which this script doesn't fill in")

    # Links into the repository, and the images' sizes.
    page = re.sub(r'href="repo:([^"]*)"', lambda m: f'href="{repository_link(m.group(1))}"', page)
    page = sized(page)
    anchors = Anchors()
    anchors.feed(page)
    for missing in sorted(anchors.links - anchors.ids):
        fail(f"the page links #{missing}, which isn't on it")

    # A fresh folder, replacing only one this script made.
    if out.exists():
        made = out / "index.html"
        if made.is_file() and GENERATOR in made.read_text(encoding="utf-8", errors="replace"):
            shutil.rmtree(out)
        elif any(out.iterdir()):
            fail(f"{out} has files this script didn't make; give an empty or new folder")
    out.mkdir(parents=True, exist_ok=True)

    # The files the page uses, copied, or made smaller.
    total = 0
    references = local_references(page, style)
    for reference in references:
        if reference == "style.css":
            continue  # written below, with its own references renamed
        source = source_of(reference)
        target = out / reference
        target.parent.mkdir(parents=True, exist_ok=True)
        if compress and source.suffix == ".png":
            target = target.with_suffix(".webp")
            run(["cwebp", "-quiet", "-q", "85", "-m", "6", "-sharp_yuv", str(source), "-o", str(target)])
        elif compress and source.suffix == ".ttf":
            shutil.copyfile(source, target)
            run(["woff2_compress", str(target)])
            target.unlink()
            target = target.with_suffix(".woff2")
        else:
            shutil.copyfile(source, target)
        renamed = target.relative_to(out).as_posix()
        if renamed != reference:
            page = page.replace(f'"{reference}"', f'"{renamed}"')
            style = style.replace(f'"{reference}"', f'"{renamed}"')
        total += target.stat().st_size

    (out / "index.html").write_text(page, encoding="utf-8", newline="\n")
    (out / "style.css").write_text(style, encoding="utf-8", newline="\n")
    total += (out / "index.html").stat().st_size + (out / "style.css").stat().st_size
    print(f"{len(checks)} checks ({tally(checks)}), {len(plots)} plots")
    print(f"{len(references) + 1} files, {total / 1e6:.1f} MB, in {out}")


def main():
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(errors="backslashreplace")
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", type=Path, default=ROOT / "target" / "site", help="where to write it (default target/site)")
    parser.add_argument("--compress", action="store_true", help="screenshots as WebP and fonts as WOFF2 (needs cwebp and woff2_compress)")
    args = parser.parse_args()
    build(args.out.resolve(), args.compress)


if __name__ == "__main__":
    main()
