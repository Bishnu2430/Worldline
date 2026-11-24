# The Windows release

Step 8.8a. Worldline for Windows is a zip with one program in it, `worldline.exe`, which needs nothing but Windows to run.

**Code:**
- `.github/workflows/release.yml`: builds the release, checks it, and publishes it from a version tag;
- `tools/check_exe.py`: reads the program's header to check what it needs (Python's standard library only);
- `crates/worldline-app/src/capture.rs`: the smoke test the program runs on a clean machine (and, since step 8.8b, its screenshots and recordings);
- `tools/release/`: the zip's `README.txt` and the release's notes.

## What's in the zip

| File | What it is |
|---|---|
| `worldline.exe` | the program, about 57 MB |
| `README.txt` | how to run it, what it needs, and some places to start |
| `LICENSE.txt` | its license, the PolyForm Noncommercial License 1.0.0: free for non-commercial use |
| `CREDITS.md` | where its data, maps and images come from, and their licenses |
| `THIRD-PARTY-LICENSES.txt` | the 165 libraries the program is built with, and their licenses' texts (since 1.0.1) |

**Nothing to install.** The program carries everything it uses:
- **Data and images:** the solar system's data, the planets' maps, the sky and the typeface are built into it (`include_bytes!` and `include_str!`), so it reads no files except the saves you make.
- **The C runtime:** it's linked in (`-C target-feature=+crt-static`), so no Visual C++ redistributable is needed. A fresh Windows doesn't have one unless some other program installed it.

**No console window.** A release build is a windowed program (`windows_subsystem = "windows"`), so nothing opens behind it. With no console to print to, it reports trouble in a message box instead:
- **Can't start:** if no graphics adapter works, the box says so and suggests `--software`.
- **Stops unexpectedly:** if it panics, the box says why.

## The checks

The **Release** workflow checks the program before anything is published.

**1. It needs nothing but Windows** (`tools/check_exe.py`). This reads the program's header, the PE format Windows loads it by, and fails unless:
- it's a 64-bit x86 program;
- it's windowed;
- every DLL it loads is Windows' own: in System32, or one of the API sets Windows resolves itself.

It names each DLL. The Visual C++ runtime (`vcruntime140.dll` and the like), or the C runtime loaded from outside the program, fails it. A normal debug build fails both ways, which is how the check was tried out. The release build loads 18 DLLs, all of them Windows': kernel32, user32, dxgi and so on.

**2. It starts on a clean machine.** A second job runs on a fresh Windows virtual machine, with no copy of the source and no build tools: only the zip, as someone who downloads it has. It unzips it and runs `worldline.exe --smoke-test frame.png`, which:
1. opens the solar system as a first launch does, its window off the screen;
2. lets time run for 60 frames;
3. saves the last frame;
4. passes if two things hold.

| The smoke test checks | Why that, and not a number |
|---|---|
| the 3D view isn't one flat color | a frame either has a picture in it, or nothing reached it but the color it's cleared to |
| the simulation's clock moved on | time either ran or it didn't |

It prints what it saw, and the frame is kept with the run.

These virtual machines have no graphics card. The program finds Windows' software renderer, WARP, and draws with that. The same run on this project's Intel Iris Xe:

| Drawn by | 60 frames took | The 3D view's colors | Simulated |
|---|---|---|---|
| Intel Iris Xe (Vulkan) | 2.1 s | 18,596 | 63 days |
| Windows' WARP (`--software`), on the same computer | 15.7 s | 17,112 | 96 days |

The machine isn't bare: GitHub's Windows images come with many programs installed. What shows that the program uses none of them is the first check, which lists everything it loads.

**Not shown:** a run on Windows 10, and on other graphics cards. It's been run on Windows 11 with an Intel Iris Xe, and on GitHub's Windows Server images with WARP.

## The libraries' licenses

The program is built with 165 Rust libraries: egui, wgpu, winit and what they use. Most are under the MIT or Apache 2.0 licenses, which ask that their copyright notices and terms go with every copy of the program. So the zip carries `THIRD-PARTY-LICENSES.txt`, which `tools/third_party_licenses.py` writes from `Cargo.lock`:
- **Which libraries:** those the program needs on 64-bit Windows, not those only its tests or build scripts use.
- **Their texts:** for each, the license files it ships. Libraries with the same text share one copy, which keeps the file to about 400 kB.
- **The 17 that ship no license file** (egui among them) get the standard text of a license they offer:
  - MIT's, with their authors as the copyright holders;
  - or one another library in the program ships word for word (Apache 2.0's, Boost's).

Where a library offers a choice of licenses, MIT comes first.

**It also guards the release.** CI runs it with `--check`, which fails if a library is under a license the release can't carry. Only permissive ones pass: MIT, Apache 2.0, BSD, ISC, Zlib, Boost, Unicode's, the Unlicense, 0BSD, CC0, and the fonts' OFL and Ubuntu Font License. So a dependency under the GPL, say, can't slip into this non-commercial release unnoticed. Today all 165 pass: 145 taken under MIT and 12 under Apache 2.0; two each under Zlib and Boost; one under ISC; and three under MIT together with Apache 2.0's, Unicode's or the fonts' licenses.

## Publishing a release

1. Set the version in `Cargo.toml` (`[workspace.package] version`), for example `1.0.0`, and merge that into `main`.
2. Tag `main` with it and push the tag:
   ```
   git tag v1.0.0
   git push origin v1.0.0
   ```
3. The workflow builds and checks the release as above. A tag that doesn't match the version fails it. If both checks pass, it publishes the release, with:
   - the zip;
   - its SHA-256;
   - notes from `tools/release/notes.md`.

The workflow also runs on pull requests that change the release's files, and by hand (**Actions → Release → Run workflow**). Then it builds and checks without publishing, and the zip can be downloaded from the run's page.

## What someone downloading it sees

The program isn't signed with a code-signing certificate, which costs money every year. So the first time, Windows SmartScreen may say *Windows protected your PC*. **More info → Run anyway** starts it. The zip's README and the release's notes say so.
