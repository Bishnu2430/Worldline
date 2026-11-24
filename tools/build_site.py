"""Builds Worldline's website, the landing page on GitHub Pages (steps 8.7 and 8.8c).

    python tools/build_site.py               # into target/site
    python tools/build_site.py --compress    # the typeface as WOFF2
    python tools/build_site.py --out DIR

The page, site/index.html, and its stylesheet are written by hand. This script:
- gives the stylesheet the app's own theme, read from the app's source
  (crates/worldline-app/src/theme.rs): every color, the corners' radii and
  the animations' length. The stylesheet may name no color of its own, so
  the site and the app can't drift apart;
- fills in the screenshots and recordings from docs/screenshots, which the
  app captured of itself (tools/capture.py), with what each shows;
- fills in the validation section from the README's table, so the site and
  the README can never disagree;
- copies every file the page uses next to it, and turns `repo:` links into
  links to the repository on GitHub;
- gives each image and video its width and height, so the page doesn't
  jump as they load.

It fails if anything the page uses or links to in the repository is missing,
if a link inside the page leads nowhere, if the theme or the README's table
has something it can't read, or if the stylesheet names a color itself.
`--compress` needs `woff2_compress` (the Ubuntu package woff2); the workflow
that publishes the site installs it.

Python's standard library only. See docs/website.md.
"""

import argparse
import html
import json
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
THEME = ROOT / "crates" / "worldline-app" / "src" / "theme.rs"
SCREENSHOTS = ROOT / "docs" / "screenshots"
REPOSITORY = "https://github.com/Bishnu2430/Worldline"
# The page's files, by the first folder of the path the page uses for them.
FOLDERS = {
    "screenshots": SCREENSHOTS,
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
# The app's colors the site uses, by their names in theme.rs, and the CSS
# variable each becomes.
COLORS = {
    "BACKGROUND": "background",
    "SURFACE": "surface",
    "RAISED": "raised",
    "BORDER": "border",
    "TEXT": "text",
    "MUTED": "muted",
    "ACCENT": "accent",
    "ACCENT_FILL": "accent-fill",
    "WITHIN": "within",
    "APPROXIMATE": "approximate",
    "BEYOND": "beyond",
}
# Where the stylesheet's theme goes.
THEME_MARKER = "/* THEME */"


def fail(message):
    sys.exit(f"build_site: {message}")


# --- The app's theme --------------------------------------------------------------


def read_theme():
    """The app's colors, radii and animation length, from theme.rs, as CSS."""
    source = THEME.read_text(encoding="utf-8")
    found = {
        name: (int(r), int(g), int(b))
        for name, r, g, b in re.findall(
            r"pub const (\w+): Color32 = Color32::from_rgb\((\d+),\s*(\d+),\s*(\d+)\);", source
        )
    }
    missing = [name for name in COLORS if name not in found]
    if missing:
        fail(f"theme.rs no longer has {', '.join(missing)}, which the site's colors come from")
    colors = {COLORS[name]: found[name] for name in COLORS}

    def one(pattern, what):
        match = re.search(pattern, source)
        if not match:
            fail(f"theme.rs no longer says {what} the way this script reads it")
        return match.groups()

    # Labels' and body text's color: the noninteractive widgets' text.
    colors["soft"] = tuple(int(x) for x in one(
        r"widgets\.noninteractive\.fg_stroke = Stroke::new\([\d.]+,\s*Color32::from_rgb\((\d+),\s*(\d+),\s*(\d+)\)\)",
        "its labels' text color"))
    radii = {
        "radius-window": one(r"visuals\.window_corner_radius = CornerRadius::same\((\d+)\)", "its windows' corners")[0],
        "radius-card": one(r"visuals\.menu_corner_radius = CornerRadius::same\((\d+)\)", "its menus' corners")[0],
        "radius-control": one(r"let radius = CornerRadius::same\((\d+)\);", "its widgets' corners")[0],
    }
    animation = one(r"pub const ANIMATION: f32 = ([\d.]+);", "its animations' length")[0]

    lines = ["/* The app's own theme, from crates/worldline-app/src/theme.rs, written in by tools/build_site.py. */",
             ":root {", "  color-scheme: dark;"]
    for name, (r, g, b) in colors.items():
        lines.append(f"  --{name}: rgb({r} {g} {b});")
        lines.append(f"  --{name}-rgb: {r} {g} {b};")
    for name, pixels in radii.items():
        lines.append(f"  --{name}: {pixels}px;")
    lines.append(f"  --animation: {float(animation):g}s;")
    lines.append("}")
    return "\n".join(lines), colors


def check_no_colors_of_its_own(style):
    """The stylesheet names no color itself: they all come from the app."""
    without_comments = re.sub(r"/\*.*?\*/", "", style, flags=re.S)
    own = re.findall(
        r"#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?)\(\s*\d[^)]*\)|(?<![\w-])(?:white|black)(?![\w-])", without_comments
    )
    if own:
        fail(f"site/style.css names colors of its own ({', '.join(sorted(set(own)))}): "
             "use the app's, var(--name) or rgb(var(--name-rgb) / alpha)")


# --- The screenshots ----------------------------------------------------------------


def read_screenshots():
    index = SCREENSHOTS / "screenshots.json"
    if not index.is_file():
        fail("docs/screenshots/screenshots.json isn't there: run tools/capture.py")
    return {entry["name"]: entry for entry in json.loads(index.read_text(encoding="utf-8"))}


def fill_screenshots(page, shots):
    """{{shot:name}}, {{video:name}} and {{shows:name}}, from what the app captured."""

    def entry(name, key="file"):
        if name not in shots or key not in shots[name]:
            fail(f"the page uses the screenshot {name!r}{' as video' if key == 'video' else ''}, "
                 "which docs/screenshots doesn't have")
        return shots[name]

    def shot(match):
        e = entry(match.group(1))
        return f'<img src="screenshots/{e["file"]}" alt="{html.escape(e["shows"])}" loading="lazy" decoding="async">'

    def video(match):
        e = entry(match.group(1), "video")
        return (f'<video src="screenshots/{e["video"]}" aria-label="{html.escape(e["shows"])}" '
                f'autoplay muted loop playsinline preload="metadata"></video>')

    page = re.sub(r"\{\{shot:([\w-]+)\}\}", shot, page)
    page = re.sub(r"\{\{video:([\w-]+)\}\}", video, page)
    page = re.sub(r"\{\{shows:([\w-]+)\}\}", lambda m: html.escape(entry(m.group(1))["shows"]), page)
    return page


# --- The README's validation table ------------------------------------------------


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
  <a class="plot" href="validation/{name}"><img src="validation/{name}" alt="{html.escape(title)}" loading="lazy" decoding="async"></a>
  <div class="check-body">
    <h3>{inline(check.test)}</h3>
    <p class="expected"><span class="label">Expected</span>{inline(check.expected)}</p>
    <p class="measured"><span class="label">Measured</span><span class="badge {check.kind}">{check.words}</span>{inline(check.measured)}</p>
    <p class="evidence">{'Tests' if len(check.tests) > 1 else 'Test'}: {tests} · {'Plots' if len(check.plots) > 1 else 'Plot'}: {plots}</p>
  </div>
</article>"""


# --- Files ------------------------------------------------------------------------


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


def webp_size(path):
    """A WebP's size, from whichever of its three headers it has."""
    with open(path, "rb") as f:
        head = f.read(32)
    if head[:4] != b"RIFF" or head[8:12] != b"WEBP":
        fail(f"{path.name} isn't a WebP")
    chunk = head[12:16]
    if chunk == b"VP8X":  # extended (animated, alpha): the canvas, 24-bit, less one
        return (int.from_bytes(head[24:27], "little") + 1, int.from_bytes(head[27:30], "little") + 1)
    if chunk == b"VP8 ":  # lossy: after the frame tag and start code, 14 bits each
        width, height = struct.unpack("<HH", head[26:30])
        return width & 0x3FFF, height & 0x3FFF
    if chunk == b"VP8L":  # lossless: 14 bits each, less one, after the signature
        bits = int.from_bytes(head[21:25], "little")
        return (bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1
    fail(f"{path.name} has a WebP header this script doesn't read: {chunk!r}")


def mp4_size(path):
    """An MP4's picture size, from its video track's header (tkhd)."""
    data = path.read_bytes()
    at = 0
    while True:
        at = data.find(b"tkhd", at)
        if at < 0:
            fail(f"{path.name} has no track with a picture")
        version = data[at + 4]
        # After the version and flags: times, track and duration (wider in
        # version 1), reserved, layer, group, volume, reserved and matrix.
        offset = at + 4 + 4 + (32 if version == 1 else 20) + 8 + 8 + 36
        width, height = struct.unpack(">II", data[offset : offset + 8])
        if width and height:
            return width >> 16, height >> 16
        at += 4


def size_of(source):
    return {".svg": svg_size, ".png": png_size, ".webp": webp_size, ".mp4": mp4_size}[source.suffix](source)


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
    found = re.findall(r'\b(?:src|href|poster)="([^"]+)"', page) + re.findall(r'url\("([^"]+)"\)', style)
    seen = []
    for reference in found:
        if re.match(r"[a-z]+:|#|//", reference):
            continue
        if reference not in seen:
            seen.append(reference)
    return seen


def sized(page):
    """Each image and video given its width and height, read from the file."""

    def size(match):
        tag = match.group(0)
        src = re.search(r'\bsrc="([^"]+)"', tag)
        # One already sized, or with no picture yet (the lightbox's).
        if " width=" in tag or src is None:
            return tag
        width, height = size_of(source_of(src.group(1)))
        return tag[:-1] + f' width="{width}" height="{height}">'

    return re.sub(r"<(?:img|video)\b[^>]*>", size, page)


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
            if name == "href" and value and value.startswith("#") and len(value) > 1:
                self.links.add(value[1:])


def run(command):
    try:
        subprocess.run(command, check=True, capture_output=True)
    except FileNotFoundError:
        fail(f"--compress needs {command[0]} (Ubuntu: apt install woff2)")
    except subprocess.CalledProcessError as error:
        fail(f"{' '.join(command)} failed: {error.stderr.decode(errors='replace')[:300]}")


# --- The build --------------------------------------------------------------------


def build(out, compress):
    checks = read_checks()
    shots = read_screenshots()
    theme, colors = read_theme()
    page = (SITE / "index.html").read_text(encoding="utf-8")
    style = (SITE / "style.css").read_text(encoding="utf-8")
    if GENERATOR not in page:
        fail("site/index.html lost its generator line, which marks what this script may replace")
    check_no_colors_of_its_own(style)
    if style.count(THEME_MARKER) != 1:
        fail(f"site/style.css needs one {THEME_MARKER} line, where the app's theme goes")
    style = style.replace(THEME_MARKER, theme)

    # The screenshots and recordings, from what the app captured, and the
    # theme's colors where the page names one (the browser's bar).
    page = fill_screenshots(page, shots)
    page = re.sub(r"\{\{color:([\w-]+)\}\}",
                  lambda m: "#{:02x}{:02x}{:02x}".format(*colors[m.group(1)]) if m.group(1) in colors
                  else fail(f"the theme has no color {m.group(1)!r}"), page)

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
    left = re.search(r"\{\{[\w:-]*\}\}", page)
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
    references = [r for r in local_references(page, style) if r != "style.css"]
    for reference in references:
        source = source_of(reference)
        target = out / reference
        target.parent.mkdir(parents=True, exist_ok=True)
        if compress and source.suffix == ".ttf":
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
    used = sum(1 for name in shots if f"screenshots/{shots[name]['file']}" in references
               or f"screenshots/{shots[name].get('video', '')}" in references)
    print(f"{len(checks)} checks ({tally(checks)}), {len(plots)} plots; {used} of {len(shots)} captures; "
          f"the theme from theme.rs")
    print(f"{len(references) + 2} files, {total / 1e6:.1f} MB, in {out}")
    return page


def main():
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(errors="backslashreplace")
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", type=Path, default=ROOT / "target" / "site", help="where to write it (default target/site)")
    parser.add_argument("--compress", action="store_true", help="the typeface as WOFF2 (needs woff2_compress)")
    args = parser.parse_args()
    build(args.out.resolve(), args.compress)


if __name__ == "__main__":
    main()
