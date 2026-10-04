# Worldline: working notes for Claude

## The collaboration

- The owner supervises and Claude writes all the code. The owner does not read Rust.
- The owner has under 5 hours a week. Keep each step small enough to verify in about an hour.
- Every step must end with:
  1. a command the owner can run (`cargo test ...` or `cargo run ...`),
  2. a physics number or visible behavior to check,
  3. a plain-language summary of the physics and design decisions, not the code.
- Worldline is a portfolio project. The owner must be able to explain the design in interviews, so keep `docs/` current.

## Git workflow

- Each roadmap step gets its own branch, `step/<number>-<short-name>` (for example `step/1.1-engine-foundation`).
- When every check passes, commit, push and open a pull request into `main`. The owner reviews and merges.
- The pull request description must include a "How to verify" section with the commands to run and the numbers to expect.
- Start the next step from an up-to-date `main`. If the previous pull request isn't merged yet, branch from it and say so in the new pull request.

## Sources of truth

- `docs/SCOPE.md`: what v1 includes and excludes. Don't add v2 features without asking.
- `docs/ROADMAP.md`: steps and status. Update the status markers when a step is done.
- `docs/ARCHITECTURE.md`: crate layout and the rules for choosing a model.
- `docs/physics/<model>.md`: one file per physics model, with its equations, source paper, validity range and validation test.

## Physics rules

- All physics is `f64` in SI units and lives in `worldline-core`, which must never depend on graphics crates.
- The GPU (Intel Iris Xe, no native `f64`) is only for rendering and bulk approximate work in local coordinates.
- Every model cites its source, declares its validity range and has a validation test. Approximations are labeled in the app.
- Never invent physics where it is unknown (singularities, neutron star interiors beyond published equations of state). Label it instead.

## Code conventions

- Cargo workspace under `crates/`. Add a crate only when its milestone starts.
- Before a step is done, all of these must pass: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`.
- Validation tests go in `crates/worldline-core/tests/validation_*.rs`. Mark slow ones `#[ignore]` and run them with `cargo test --release -- --ignored`.
- Environment: Windows 11, stable Rust (MSVC toolchain), PowerShell.
