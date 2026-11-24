"""Lists the libraries built into worldline.exe, with their licenses' texts.

    python tools/third_party_licenses.py --check
    python tools/third_party_licenses.py --out THIRD-PARTY-LICENSES.txt

The Windows program is Worldline's own code and the Rust libraries it's
built with. Most of those are under the MIT or Apache 2.0 licenses, which
ask that their copyright notices and terms go with every copy. The release
puts this file, THIRD-PARTY-LICENSES.txt, in the zip beside the program.

It works from Cargo.lock, through `cargo metadata`:
- the libraries are those worldline-app needs on 64-bit Windows, at build
  time and after (not those only the tests or build scripts use);
- each one's license is what its own Cargo.toml says, and its texts are the
  license files it ships (LICENSE, COPYING, NOTICE and the like);
- a library that ships no license file gets the standard text of a license
  it offers: MIT's, with its authors as the copyright holders, or one that
  another library in the program ships word for word.

`--check` fails if a library's license isn't one the release can carry
(permissive ones: MIT, Apache 2.0, BSD, ISC, Zlib, Boost and the like), or
if one would go without a text. CI runs it, so a dependency under, say, the
GPL can't slip into a non-commercial release unnoticed.

Python's standard library only. See docs/release.md.
"""

import argparse
import collections
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-pc-windows-msvc"
PROGRAM = "worldline-app"

# Licenses the release can carry, the first that a library offers being the
# one its text is taken from when it ships none.
PREFERRED = [
    "MIT", "Apache-2.0", "Zlib", "BSD-3-Clause", "BSD-2-Clause", "ISC", "BSL-1.0", "Unlicense", "0BSD",
    "CC0-1.0", "Unicode-3.0", "Unicode-DFS-2016", "OFL-1.1", "Ubuntu-font-1.0",
]
ALLOWED = set(PREFERRED)
# Exceptions that only widen what a license allows.
EXCEPTIONS = {"LLVM-exception"}

# License files a library keeps somewhere other than its top folder.
ELSEWHERE = {
    "epaint_default_fonts": ["fonts/OFL.txt", "fonts/UFL.txt", "fonts/Hack-Regular.txt",
                             "fonts/emoji-icon-font-mit-license.txt"],
}
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|notice|unlicense|copyright)", re.I)

# The MIT License's text, word for word, with the copyright line left to fill.
MIT = """MIT License

Copyright (c) {holders}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""


def fail(message):
    sys.exit(f"third_party_licenses: {message}")


# --- License expressions ----------------------------------------------------------


def choices(expression):
    """A license expression as its alternatives, each a set of licenses that
    must all be met: "(MIT OR Apache-2.0) AND Unicode-3.0" is
    [{MIT, Unicode-3.0}, {Apache-2.0, Unicode-3.0}]. The old "MIT/Apache-2.0"
    means OR."""
    tokens = re.findall(r"\(|\)|[A-Za-z0-9.+-]+", expression.replace("/", " OR "))
    at = 0

    def peek():
        return tokens[at] if at < len(tokens) else None

    def take():
        nonlocal at
        at += 1
        return tokens[at - 1]

    def primary():
        token = take()
        if token == "(":
            inner = either()
            if take() != ")":
                fail(f"unbalanced parentheses in the license {expression!r}")
            return inner
        name = token[:-1] if token.endswith("+") else token
        if peek() == "WITH":
            take()
            exception = take()
            if exception not in EXCEPTIONS:
                return [{f"{name} WITH {exception}"}]
        return [{name}]

    def both():
        result = primary()
        while peek() == "AND":
            take()
            right = primary()
            result = [a | b for a in result for b in right]
        return result

    def either():
        result = both()
        while peek() == "OR":
            take()
            result += both()
        return result

    if not tokens:
        fail(f"an empty license expression: {expression!r}")
    result = either()
    if at != len(tokens):
        fail(f"can't read the license {expression!r}")
    return result


def best(expression):
    """The alternative the release can carry, the most preferred first, or None."""
    allowed = [c for c in choices(expression) if c <= ALLOWED]
    if not allowed:
        return None
    return min(allowed, key=lambda c: sorted(PREFERRED.index(name) for name in c))


# --- The libraries ----------------------------------------------------------------


def libraries():
    """The packages built into the program for Windows, with their license
    files' texts."""
    try:
        done = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=True)
    except subprocess.CalledProcessError as error:
        fail(f"cargo metadata failed: {error.stderr[-400:]}")
    metadata = json.loads(done.stdout)
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    members = set(metadata["workspace_members"])
    program = next((i for i in members if packages[i]["name"] == PROGRAM), None)
    if program is None:
        fail(f"no {PROGRAM} in the workspace")
    # What the program needs to run, not what only its tests or build
    # scripts do.
    seen, stack = set(), [program]
    while stack:
        here = stack.pop()
        if here in seen:
            continue
        seen.add(here)
        for dependency in nodes[here]["deps"]:
            if any(kind["kind"] is None for kind in dependency["dep_kinds"]):
                stack.append(dependency["pkg"])
    found = []
    for package in sorted((packages[i] for i in seen - members), key=lambda p: (p["name"], p["version"])):
        folder = Path(package["manifest_path"]).parent
        if not folder.is_dir():
            fail(f"{package['name']} {package['version']}'s source isn't here: run cargo fetch first")
        names = [f.name for f in sorted(folder.iterdir()) if f.is_file() and LICENSE_FILE.match(f.name)]
        names += ELSEWHERE.get(package["name"], [])
        if package.get("license_file") and package["license_file"] not in names:
            names.append(package["license_file"])
        texts = []
        for name in names:
            path = folder / name
            if not path.is_file():
                fail(f"{package['name']} {package['version']} has no {name}")
            texts.append(path.read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n").strip() + "\n")
        found.append({"package": package, "files": names, "texts": texts})
    return found


def standard_texts(found):
    """Each license's text as libraries under that license alone ship it,
    the most common version: for libraries that ship none."""
    counts = collections.defaultdict(collections.Counter)
    for entry in found:
        license = entry["package"].get("license") or ""
        if re.fullmatch(r"[A-Za-z0-9.-]+", license) and license != "MIT":
            for text in entry["texts"]:
                counts[license][text] += 1
    return {license: texts.most_common(1)[0][0] for license, texts in counts.items()}


def holders(package):
    """A library's copyright holders, from its authors (names, not emails)."""
    authors = [re.sub(r"\s*<[^>]*>", "", a).strip() for a in package.get("authors") or []]
    authors = [a for a in authors if a]
    return ", ".join(authors) if authors else f"the {package['name']} authors"


def assign(found):
    """Every library's texts; for one that ships none, the standard text of
    the license chosen for it. Fails if one can't be carried or has no text."""
    standard = standard_texts(found)
    problems = []
    for entry in found:
        package = entry["package"]
        label = f"{package['name']} {package['version']}"
        license = package.get("license")
        if not license:
            problems.append(f"{label} declares no license")
            continue
        chosen = best(license)
        if chosen is None:
            problems.append(f"{label} is under {license}, which the release can't carry")
            continue
        entry["chosen"] = " AND ".join(sorted(chosen, key=PREFERRED.index))
        # Files kept elsewhere (the fonts' licenses) cover what's there, not
        # the library's own code: that gets its license's text as well.
        own = [n for n in entry["files"] if n not in ELSEWHERE.get(package["name"], [])]
        if own:
            continue
        code = sorted(chosen, key=PREFERRED.index)
        if entry["texts"]:
            code = code[:1]
        for name in code:
            if name == "MIT":
                entry["texts"].append(MIT.format(holders=holders(package)))
            elif name in standard:
                entry["texts"].append(standard[name])
            else:
                problems.append(f"{label} ships no license file, and no other library has {name}'s text")
        entry["generated"] = True
    if problems:
        fail("; ".join(problems))


# --- The file -----------------------------------------------------------------------


def write(found, out, version):
    groups = collections.OrderedDict()
    for entry in found:
        for text in entry["texts"]:
            groups.setdefault(text, []).append(entry)
    width = max(len(f"{e['package']['name']} {e['package']['version']}") for e in found) + 2
    lines = [
        f"Worldline {version}: the software it's built with",
        "=" * len(f"Worldline {version}: the software it's built with"),
        "",
        "worldline.exe is Worldline's own code (its license is LICENSE.txt) and the",
        f"{len(found)} libraries below, each under the license its authors chose. Their",
        "notices and terms follow, as those licenses ask. Where a library offers a",
        "choice, the release takes the one shown after its license.",
        "",
        "The data, maps, images and typeface Worldline uses are in CREDITS.md.",
        "",
        f"Made by tools/third_party_licenses.py from Cargo.lock, for {TARGET}.",
        "",
        "",
        "The libraries",
        "-------------",
        "",
    ]
    for entry in found:
        package = entry["package"]
        name = f"{package['name']} {package['version']}"
        note = "" if not entry.get("generated") else "  (ships no license file: its standard text is below)"
        lines.append(f"{name:<{width}}{package['license']}  [{entry['chosen']}]{note}")
    lines += ["", "", "The licenses", "------------", ""]
    for number, (text, entries) in enumerate(groups.items(), 1):
        names = ", ".join(f"{e['package']['name']} {e['package']['version']}" for e in entries)
        lines += [f"=== {number}. For {names}", "", text.rstrip("\n"), "", ""]
    out.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    return len(groups)


def main():
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(errors="backslashreplace")
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", type=Path, help="write the file here")
    parser.add_argument("--check", action="store_true", help="check only (the default without --out)")
    args = parser.parse_args()
    found = libraries()
    assign(found)
    licenses = collections.Counter(entry["chosen"] for entry in found)
    shipped = sum(1 for e in found if not e.get("generated"))
    print(f"{len(found)} libraries in the program for Windows, all under licenses the release can carry: "
          + ", ".join(f"{name} {n}" for name, n in licenses.most_common()))
    print(f"{shipped} ship their license files; {len(found) - shipped} get a standard text")
    if args.out:
        version = json.loads(subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=ROOT, capture_output=True,
            text=True, encoding="utf-8", check=True).stdout)
        version = next(p["version"] for p in version["packages"] if p["name"] == PROGRAM)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        texts = write(found, args.out, version)
        print(f"wrote {args.out}: {texts} distinct texts, {args.out.stat().st_size / 1e3:.0f} kB")


if __name__ == "__main__":
    main()
