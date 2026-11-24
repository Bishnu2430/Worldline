# The website

Step 8.7. Worldline's landing page, on GitHub Pages: **https://bishnu2430.github.io/Worldline/**

**Code:**
- `site/index.html`, `site/style.css` and `site/favicon.svg`: the page, written by hand;
- `tools/build_site.py`: builds it (Python's standard library only);
- `.github/workflows/pages.yml`: builds it on every pull request, and publishes it from `main`.

## What's on it

From top to bottom:
1. **The opening:** what Worldline is, the interface (Saturn up close), and four numbers.
2. **What it does:** nine features with a screenshot each, then how it labels its own limits.
3. **More screenshots.**
4. **Validated against reality:** six checks shown large, then all of them, each with its plot, and a box to find one.
5. **Run it:** building from source, and commands to start with.
6. **How it's built:** links to the docs.

The colors are the app's own, from `theme.rs`. So is the typeface, Inter, and the icon, `theme::icon` drawn again as an SVG. There's no framework and no build step beyond the script, and the only script on the page is the one that finds a check as you type. Without it, every check shows.

## One source for every claim

The page doesn't repeat the README's validation table by hand. `build_site.py` reads it and writes a card for each row:
- the claim;
- what's expected;
- what it measured, with a badge: passing, in part, or failing;
- the plot;
- links to the test and the plots.

It also counts them for the page ("83 of 84 pass in full; 1 passes in part, and lists what it misses"). A row the README changes, adds or downgrades changes on the site at the next deploy, and nothing on the site can claim more than the README does.

The features' numbers are written by hand, each copied from a README row or the physics doc it links. Most are passing checks; a few, like the counts of moons and belt objects, are what the data holds.

## What the build checks

`build_site.py` fails, and so does the workflow, if:
- a screenshot, plot, font or other file the page uses isn't in the repository;
- a `repo:` link leads to a file or folder that isn't there (the page writes `repo:docs/SCOPE.md`; the script turns it into a GitHub link);
- a link within the page leads to no element on it;
- the README's table has a row it can't read, a status that isn't passing, in part or failing, or a test or plot it links that isn't there;
- a featured plot belongs to no check;
- a `{{…}}` placeholder is left unfilled.

It also gives every image its width and height, read from the file, so the page doesn't jump as they load.

## Where the files come from

The page refers to files by short paths, and the script copies each from where it lives:

| The page uses | From |
|---|---|
| `images/…` | `docs/images/`, the screenshots |
| `validation/…` | `docs/validation/`, the plots |
| `fonts/…` | `crates/worldline-app/assets/fonts/`, the app's typeface and its license |
| anything else | `site/` |

So nothing is stored twice. With `--compress`, which the workflow uses, the screenshots become WebP and the typeface WOFF2. That's about a tenth of the size: the opening screenshot goes from 1.3 MB to 0.1 MB. Without it, they're copied as they are, which is enough to look at the page locally.

The screenshots are the ones taken at each step, so the older ones show the interface as it was before step 8.3's redesign. The 3D view in them is the same.

## How it's published

`.github/workflows/pages.yml`:
- **On a pull request:** builds the site, compressed, so a broken page fails the pull request.
- **On a push to `main`:** builds it and publishes it with GitHub's own Pages actions (`upload-pages-artifact`, then `deploy-pages`).

Pages must be set once, in the repository's **Settings → Pages**, to take its site from **GitHub Actions**.

## Looking at it locally

```
python tools/build_site.py
python -m http.server 8765 --directory target/site
```

Then open http://localhost:8765.

## Credits on the page

The screenshots show planet, Moon and Sun maps from Solar System Scope (CC BY 4.0) and the Milky Way from ESA/Gaia/DPAC (CC BY-SA 3.0 IGO). The footer names both licenses and links [CREDITS.md](../CREDITS.md). Inter's license (SIL Open Font License) is published beside the font.
