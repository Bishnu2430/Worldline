"""Takes Worldline's screenshots and recordings again (step 8.8b).

    python tools/capture.py                     # every one
    python tools/capture.py earth hulse-taylor  # just these
    python tools/capture.py --list              # what there is

Each is the app itself, run with the options listed below plus `--capture`
or `--record`: its window opens off the screen, without taking the keyboard
and mouse, and the frames are saved as its GPU drew them, on a fixed clock
(see crates/worldline-app/src/capture.rs). So every image shows the
interface as it is now, and the same command makes the same picture.

Into docs/screenshots/ go:
- each screenshot as WebP;
- each recording as an animated WebP, 800 pixels wide, for the README (a
  GIF in full color, at a tenth of a GIF's size), and as an MP4, 1280 wide,
  for the website;
- screenshots.json and README.md, listing each with what it shows and the
  command that makes it.

Needs the release build, which it makes first (`cargo build --release`), and
ffmpeg. Python's standard library only.
"""

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "screenshots"
EXE = ROOT / "target" / "release" / ("worldline.exe" if sys.platform == "win32" else "worldline")


@dataclass
class Shot:
    """A screenshot: the frame `at` seconds of the app's time in."""

    name: str
    shows: str
    args: list
    at: float = 2.0


@dataclass
class Film:
    """A recording: `length` seconds from `at`, `fps` frames a second."""

    name: str
    shows: str
    args: list
    at: float
    length: float
    fps: float = 15.0
    extra: dict = field(default_factory=dict)


SHOTS = [
    Shot("interface", "Saturn up close: along the top, time, the scenarios, eclipses, the sky, the manual and the tools to add bodies; on the right, every body, and the one selected with the physics that moves it",
         ["--focus", "Saturn", "--zoom", "8", "--pitch", "20", "--paused"]),
    Shot("solar-system", "The real solar system five months after 1 January 2025, with relativistic gravity: the planets' orbits, the asteroid belt and the comets",
         [], at=5.0),
    Shot("earth", "Earth on 1 January 2025: clouds, blue Rayleigh haze, the Pacific in daylight and night falling over Asia",
         ["--focus", "Earth", "--paused"]),
    Shot("saturn-2032", "Saturn in April 2032 from below its rings: Cassini's measured ring structure, and Saturn's shadow across the rings",
         ["--focus", "Saturn", "--advance", "7.3", "--pitch", "-12", "--paused"]),
    Shot("belts", "The asteroid belt and Jupiter's two Trojan clouds, 60° ahead of and behind it, from 28,331 real orbits, in the glow of the zodiacal dust",
         ["--focus", "Sun", "--zoom", "2500", "--pitch", "60", "--paused"]),
    # Bodies added from the command line are announced for 6 s: these are
    # taken after that.
    Shot("black-hole", "Gaia BH1 from 570 km, nearly edge on to a disk drawn around it: its shadow, with the disk's far side bent over the top and under the bottom by its gravity, rays traced one by one. The disk is an illustration, a thin disk as Novikov and Thorne model one: Gaia BH1 has none that's been seen",
         ["--add", "Gaia BH1:1", "--focus", "Gaia BH1", "--zoom", "20", "--pitch", "3", "--paused"], at=7.0),
    Shot("spinning-black-hole", "Cygnus X-1, spinning at 0.9985, from 9° above the disk drawn around it: the shadow flattened where light circles with the spin, and the disk's far side lensed over the top",
         ["--add", "Cygnus X-1:3", "--focus", "Cygnus X-1", "--zoom", "28", "--pitch", "9", "--paused"], at=7.0),
    Shot("neutron-star", "The Hulse–Taylor pulsar from close by: its gravity bends its light, so it looks 1.25 times its size with more than half its surface in view, and its two radio beams",
         ["--add", "PSR B1913+16 pair:30", "--focus", "PSR B1913+16", "--zoom", "6", "--paused"], at=7.0),
    Shot("gravitational-waves", "GW150914's two black holes at 20 Hz: their gravitational waves' crests spiral out at the speed of light, and the clock-rate map glows where clocks run slowest",
         ["--add", "GW150914 pair:30", "--focus", "GW150914 heavier hole", "--zoom", "600", "--pitch", "60", "--paused", "--overlays", "waves,clocks,models"], at=7.0),
    Shot("tides", "Earth's tides and the shape of space around it: arrows stretched toward and away from the Moon and the Sun, and space dipping toward Earth's center",
         ["--focus", "Earth", "--zoom", "90", "--pitch", "25", "--paused", "--overlays", "tides,embedding"]),
    Shot("eclipse-path", "The total solar eclipse of 12 August 2026 at its greatest, found by the eclipse finder in the app's own simulation: the Moon's shadow on Earth and its path of totality",
         ["--eclipse", "7"], at=150.0),
    Shot("totality", "The same eclipse from Reykjavík at totality: the Moon's black disk over the Sun, with the corona around it",
         ["--stand", "64.1466,-21.9426", "--advance", "1.611890", "--paused", "--fov", "4"]),
    Shot("crater", "Chicxulub's estimated impactor, sent at Earth: the crater it digs, 25 hours later in the noon Sun",
         ["--add", "Chicxulub impactor:0.002@Earth", "--aim", "12,0.5", "--advance", "0.00289", "--focus", "Earth",
          "--zoom", "1.12", "--yaw", "-75", "--pitch", "39", "--speed", "real"]),
    Shot("collision", "A second Earth strikes Earth at 30 km/s, a little off center: the fragments it throws off, and what's left",
         ["--add", "earth:0.0003@Earth", "--aim", "30,0.3", "--focus", "Earth", "--zoom", "330", "--speed", "hour"], at=20.0),
    Shot("alpha-centauri", "α Centauri A and B, 4.4 light-years away, on their 80-year orbit",
         ["--focus", "Alpha Centauri A", "--zoom", "4000", "--pitch", "85", "--speed", "decade"], at=4.0),
    Shot("clusters", "The Milky Way's globular clusters, and Omega Centauri, the largest",
         ["--focus", "Omega Centauri", "--zoom", "9000", "--pitch", "40", "--paused"]),
    Shot("milky-way", "The Milky Way from 35 kiloparsecs, drawn as light: its disk and bulge, with the Sun on its path round the center",
         ["--focus", "Sun", "--zoom", "1.5e12", "--pitch", "50", "--paused"]),
    Shot("scenarios", "The Scenarios window: twelve famous events, each played as a film",
         ["--scenarios", "--paused"]),
    Shot("manual", "The manual inside the app, at its beginning",
         ["--manual", "--paused"]),
    Shot("film", "The Hulse–Taylor pulsar's film: the orbit turning as general relativity says, with a card measuring it against the pulsar's timing",
         ["--scenario", "hulse-taylor"], at=58.0),
]

FILMS = [
    # From the start of a shot of its film (scripts.rs).
    Film("hulse-taylor", "The Hulse–Taylor pulsar's film, its fourth shot: at a month a second, the orbit's closest point turns 4.2° a year, as general relativity says",
         ["--scenario", "hulse-taylor"], at=43.0, length=8.0),
    # The camera flies in first, so this starts after the flight.
    Film("saturn-moons", "Saturn's moons at a day a second, each on its own orbit in the plane of the rings",
         ["--focus", "Saturn", "--zoom", "40", "--pitch", "60", "--speed", "day"], at=4.0, length=8.0),
]


def run_app(args, timeout):
    """Runs the app; its output, or the reason it failed."""
    began = time.monotonic()
    try:
        done = subprocess.run([str(EXE), *args], capture_output=True, text=True, timeout=timeout,
                              encoding="utf-8", errors="replace")
    except subprocess.TimeoutExpired:
        return False, f"didn't finish in {timeout} s"
    output = (done.stdout + done.stderr).strip()
    took = time.monotonic() - began
    return done.returncode == 0, f"{output}\n({took:.0f} s)"


def ffmpeg(*args):
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *args], check=True)


def command(args, extra):
    quoted = [f'"{a}"' if " " in a else a for a in [*args, *extra]]
    return "cargo run --release -- " + " ".join(quoted)


def take_shot(shot, work):
    png = work / f"{shot.name}.png"
    ok, said = run_app([*shot.args, "--capture", str(png), "--at", str(shot.at)], timeout=900)
    if not ok:
        return None, said
    webp = OUT / f"{shot.name}.webp"
    ffmpeg("-i", str(png), "-c:v", "libwebp", "-quality", "90", "-compression_level", "6", str(webp))
    return {
        "name": shot.name,
        "file": webp.name,
        "shows": shot.shows,
        "command": command(shot.args, ["--capture", f"{shot.name}.png", "--at", f"{shot.at:g}"]),
    }, said


def take_film(film, work):
    frames = work / film.name
    ok, said = run_app([*film.args, "--record", str(frames), "--at", str(film.at), "--for", str(film.length),
                        "--fps", str(film.fps)], timeout=1800)
    if not ok:
        return None, said
    pattern = str(frames / "frame-%05d.png")
    moving, mp4 = OUT / f"{film.name}.webp", OUT / f"{film.name}.mp4"
    ffmpeg("-framerate", str(film.fps), "-i", pattern, "-vf", "scale=800:-1:flags=lanczos",
           "-c:v", "libwebp_anim", "-quality", "70", "-compression_level", "4", "-loop", "0", str(moving))
    ffmpeg("-framerate", str(film.fps), "-i", pattern, "-vf", "scale=1280:-2:flags=lanczos,format=yuv420p",
           "-c:v", "libx264", "-crf", "23", "-preset", "slow", "-movflags", "+faststart", "-an", str(mp4))
    return {
        "name": film.name,
        "file": moving.name,
        "video": mp4.name,
        "shows": film.shows,
        "command": command(film.args, ["--record", film.name, "--at", f"{film.at:g}",
                                       "--for", f"{film.length:g}", "--fps", f"{film.fps:g}"]),
    }, said


def write_index(entries):
    (OUT / "screenshots.json").write_text(json.dumps(entries, indent=2, ensure_ascii=False) + "\n",
                                          encoding="utf-8", newline="\n")
    lines = [
        "# Screenshots and recordings",
        "",
        "Step 8.8b. Each is the app itself, capturing its own frames, made by `python tools/capture.py`.",
        "Running the command under one makes it again (`--capture` saves a PNG, which the tool turns into WebP;",
        "`--record` saves a folder of frames, which it turns into an animated WebP and an MP4).",
        "",
    ]
    for entry in entries:
        lines += [f"## {entry['name']}", "", entry["shows"] + ".", ""]
        lines += [f"![{entry['shows']}]({entry['file']})", ""]
        if "video" in entry:
            lines += [f"As video: [{entry['video']}]({entry['video']}).", ""]
        lines += ["```", entry["command"], "```", ""]
    (OUT / "README.md").write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main():
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(errors="backslashreplace")
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("names", nargs="*", help="only these (default: every one)")
    parser.add_argument("--list", action="store_true", help="list them and stop")
    args = parser.parse_args()
    every = [*SHOTS, *FILMS]
    names = [item.name for item in every]
    if len(set(names)) != len(names):
        sys.exit("capture: two share a name, and would overwrite each other's file")
    if args.list:
        for item in every:
            print(f"{item.name:22} {item.shows[:90]}")
        return
    unknown = set(args.names) - {item.name for item in every}
    if unknown:
        sys.exit(f"capture: no such screenshot or recording: {', '.join(sorted(unknown))}")
    if shutil.which("ffmpeg") is None:
        sys.exit("capture: needs ffmpeg on the PATH")
    subprocess.run(["cargo", "build", "--release", "-p", "worldline-app"], cwd=ROOT, check=True)
    OUT.mkdir(parents=True, exist_ok=True)

    index = OUT / "screenshots.json"
    entries = json.loads(index.read_text(encoding="utf-8")) if index.is_file() else []
    by_name = {e["name"]: e for e in entries}
    chosen = [item for item in every if not args.names or item.name in args.names]
    failed = []
    with tempfile.TemporaryDirectory(prefix="worldline-capture-") as folder:
        work = Path(folder)
        for item in chosen:
            print(f"{item.name}…", flush=True)
            made, said = (take_film if isinstance(item, Film) else take_shot)(item, work)
            print("  " + said.replace("\n", "\n  "), flush=True)
            if made is None:
                failed.append(item.name)
            else:
                by_name[item.name] = made
    # In the order listed above, keeping any made before that aren't remade.
    order = [item.name for item in every]
    write_index([by_name[n] for n in order if n in by_name])
    if failed:
        sys.exit(f"capture: failed: {', '.join(failed)}")
    print(f"{len(chosen)} made, into {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
