# The website

Steps 8.7 and 8.8c. Worldline's landing page, on GitHub Pages: **https://bishnu2430.github.io/Worldline/**

**Code:**
- `site/index.html`, `site/style.css` and `site/favicon.svg`: the page, written by hand;
- `tools/build_site.py`: builds it (Python's standard library only);
- `.github/workflows/pages.yml`: builds it on every pull request, and publishes it from `main`.

## What's on it

From top to bottom:
1. **The opening:** what Worldline is, the download, and the Hulse–Taylor film playing in a window like the app's, recorded by the app itself. Then four numbers.
2. **What it simulates:** nine tiles, each a capture's 3D view with its numbers, copied from the README or the physics doc it links. One of them is a recording.
3. **Honest about its limits:** the app's model indicator, the three verdicts in its colors, beside one of its inspector cards drawn as the app draws it.
4. **Validated against reality:** six checks shown large, then all of them, each with its plot, and a box to find one.
5. **Gallery:** twelve more captures, each opening full size.
6. **Run it:** the download, building from source, and commands to start with.
7. **How it's built:** links to the docs.

There's no framework. The page's one script:
- filters the checks as you type;
- opens the gallery's pictures full size;
- plays the videos only while they're on screen;
- eases the sections in as they arrive.

Without the script, everything shows and nothing moves. Anyone who has asked their system for less motion gets the videos paused with their controls, and no easing.

## One theme: the app's

The site's colors aren't the site's. `build_site.py` reads them from the app's own source, `crates/worldline-app/src/theme.rs`, and writes them into the stylesheet where it says `/* THEME */`:
- every color the app draws its interface with: background, surface, raised, border, text, the labels' text, muted, accent, the accent's fill, and the model indicator's three, within range, approximate and beyond;
- the corners' radii: the windows' (12 px), the cards' (10) and the controls' (7);
- the animations' length (0.12 s).

The stylesheet may name no color of its own: no `#…`, no `rgb(` with numbers, no `white` or `black`. The build fails if it does. It uses `var(--accent)`, or `rgb(var(--accent-rgb) / 0.3)` for one seen through. So when a color changes in the app, it changes on the site at the next deploy, and the two can't drift apart. The page's one other color, the browser's bar (`theme-color`), comes from the theme too. The typeface is the app's, Inter, and the icon is the app's, `theme::icon` drawn again as an SVG.

## One source for every picture

Every picture and video is the app capturing its own frames (step 8.8b). `tools/capture.py` makes them into `docs/screenshots`, with a list, `screenshots.json`, of what each shows and the command that makes it.

The page names a capture, and the build fills it in from that list:

| The page writes | It becomes |
|---|---|
| `{{shot:black-hole}}` | an image, with what it shows as its text for screen readers |
| `{{video:saturn-moons}}` | a silent, looping video |
| `{{shows:hulse-taylor}}` | what it shows, as a caption |

So no picture on the site is older than the app, and none needs describing twice.

**The tiles show only the 3D view.** The app's panel takes the right third of each frame. The full interface is in the opening's film and the gallery.

## One source for every claim

The page doesn't repeat the README's validation table by hand. `build_site.py` reads it and writes a card for each row:
- the claim;
- what's expected;
- what it measured, with a badge: passing, in part, or failing;
- the plot;
- links to the test and the plots.

It also counts them ("83 of 84 pass in full; 1 passes in part, and lists what it misses"). A row the README changes, adds or downgrades changes on the site at the next deploy.

## What the build checks

`build_site.py` fails, and so does the workflow, if:
- a capture, plot, font or other file the page uses isn't in the repository;
- the page names a capture `screenshots.json` doesn't have, or a video one that has none;
- `theme.rs` no longer has a color, radius or the animation length the site reads;
- the stylesheet names a color of its own;
- a `repo:` link leads to a file or folder that isn't there (the page writes `repo:docs/SCOPE.md`; the script turns it into a GitHub link);
- a link within the page leads to no element on it;
- the README's table has a row it can't read, a status that isn't passing, in part or failing, or a test or plot it links that isn't there;
- a featured plot belongs to no check;
- a `{{…}}` placeholder is left unfilled.

It also gives every image and video its width and height, read from the file (WebP, PNG, SVG or MP4), so the page doesn't jump as they load.

## Where the files come from

| The page uses | From |
|---|---|
| `screenshots/…` | `docs/screenshots/`, the captures |
| `validation/…` | `docs/validation/`, the plots |
| `fonts/…` | `crates/worldline-app/assets/fonts/`, the app's typeface and its license |
| anything else | `site/` |

Nothing is stored twice. The captures are already small (WebP and MP4), and with `--compress`, which the workflow uses, the typeface becomes WOFF2. The whole site is about 6 MB: 3.9 MB of captures, 1.3 MB of plots and 0.8 MB of typeface before compressing.

A shared link shows `docs/screenshots/social.jpg`, the interface cropped to 1200 by 630, which `capture.py` makes beside the WebP. Some sites won't show WebP there.

## How it's published

`.github/workflows/pages.yml`:
- **On a pull request:** builds the site, so a broken page fails the pull request.
- **On a push to `main`:** builds it and publishes it with GitHub's own Pages actions (`upload-pages-artifact`, then `deploy-pages`).

The repository's **Settings → Pages** takes the site from **GitHub Actions**.

## Looking at it locally

```
python tools/build_site.py
python -m http.server 8765 --directory target/site
```

Then open http://localhost:8765.

## Credits on the page

The captures show planet, Moon and Sun maps from Solar System Scope (CC BY 4.0) and the Milky Way from ESA/Gaia/DPAC (CC BY-SA 3.0 IGO). The footer names both licenses and links [CREDITS.md](../CREDITS.md). Inter's license (SIL Open Font License) is published beside the font.
