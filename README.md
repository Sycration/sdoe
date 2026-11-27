# sdoe — design of experiments

A design-of-experiments application with a Rust engine and a Qt Quick desktop frontend.

```
core/      sdoe-core: all mathematics and project data handling (rlib + cdylib)
app/       sdoe-app: Qt Quick (QML) desktop frontend, uses sdoe-core as a crate
include/   sdoe.h: C header for the shared library
js/        sdoe.js: JavaScript interface to the WebAssembly build
```

The engine contains the project data (factors, responses, runs, models,
settings), the undo history and every calculation. It contains no file system
code: its inputs and outputs are text (project JSON, CSV, HTML reports), and
the frontend reads and writes the files.

## Building

Requirements: Rust 1.85+, Qt 6 (Core, Gui, Qml, Quick, QuickControls2) with
`qmake` on `PATH` (or `QMAKE=/path/to/qmake6`).

```sh
cargo run --release -p sdoe-app            # desktop application
cargo run --release -p sdoe-app -- my.sdoe # open a project file
cargo build --release -p sdoe-core         # shared library only
cargo test --release -p sdoe-core          # engine tests
```

The shared library lands in `target/release/` as `libsdoe.so` (Linux),
`libsdoe.dylib` (macOS) or `sdoe.dll` + `sdoe.dll.lib` (Windows).

The WebAssembly build needs no glue code or bindings generator:

```sh
rustup target add wasm32-unknown-unknown
cargo build --release -p sdoe-core --lib --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/sdoe.wasm js/
# tests, and a comparison of every reply with the native engine:
cargo build --release -p sdoe-core --example replay
node js/test.mjs js/sdoe.wasm target/release/examples/replay
```

The interface uses the desktop's Qt Quick Controls style (Windows, macOS,
`org.kde.desktop`/Breeze on KDE Plasma, Fusion elsewhere on Linux) and colours
from the system palette, including dark mode. On Linux, the desktop style
depends on the session environment (`XDG_CURRENT_DESKTOP`, `WAYLAND_DISPLAY`),
which terminals and launchers inside the desktop session provide.

## Releases

`scripts/release.sh` runs the engine tests and builds the release assets
into `dist/v<version>/`: the application as a Linux AppImage, a Windows
zip and a universal macOS app bundle (Qt bundled), the library for Linux
x86_64 and ARM64 (glibc 2.28+), Windows x86_64, macOS (universal) and
Android (four ABIs), each with `sdoe.h`, `API.md` and
`LICENSE`, the WebAssembly module with `sdoe.js` and its TypeScript
definitions, plus `SHA256SUMS`. It also writes draft release notes and prints
the `gh release create` command. `--add` builds selected steps into an
existing `dist/v<version>/`.

The macOS steps cross-compile with [osxcross](https://github.com/tpoechtrager/osxcross).
For the application, the script downloads Qt for macOS (with aqtinstall) in
the version of the Linux Qt, whose moc, rcc and QML tools generate the code.
The bundle is signed ad hoc by the linker, not with an Apple Developer ID;
`scripts/macho-signature.py` checks the signatures that arm64 Macs require. Its header lists the steps, the required
packages (Fedora names) and the settings.

## API

Frontends drive the engine through JSON commands, from Rust
(`sdoe::Engine::call`), C (`sdoe.h`) or JavaScript (`sdoe.js`). [API.md](API.md) documents every
command, parameter, design option, sheet column, plot format and the project
file format.

## Capabilities

**Factors** — continuous, discrete numeric (fixed levels), categorical and
mixture components, each with a role:

- *Controlled*: set in the experiment and in production.
- *Noise*: set in the experiment, random in production (robust design).
- *Environmental*: cannot be set; measured at every run and modelled as a
  covariate (temperature of the day, raw-material batch, humidity…).
- *Block*: grouping created by blocked designs.

**Conditional factors** — a factor can apply only when another factor meets
a condition (`T < 0`, `M = Freeze, Flash`), e.g. a defrosting method that
only matters below freezing. Where it does not apply, its value stays blank
and its model terms are 0. See
[API.md](API.md#conditional-factors).

**Designs** — full factorial (mixed levels), 2-level fractional factorial
(minimum aberration, user generators, alias structure, fold-over),
Plackett-Burman (Hadamard orders up to 100+), definitive screening
(conference matrices), central composite (rotatable, orthogonal, face-centred,
spherical, inscribed, blocked), Box-Behnken, D/I/A-optimal coordinate exchange
for any factor mix and model (including augmentation of existing runs and
planning around known/forecast environmental values), maximin Latin
hypercube, simplex lattice and centroid mixtures, and Taguchi inner × outer
orthogonal arrays (L4–L32, L18, L25, L16 four-level).

**Environmental factors** — measured columns are highlighted for entry; models
include them as covariates; the analysis output includes a test of
environment × factor interactions with a note when they are significant;
optimisation either holds them at today's value or averages over their
distribution (robust optimisation); the *adjustment law* gives the best
controlled settings as fitted formulas of an environmental factor; *Next run*
computes the most informative (or best predicted) settings for the current
conditions.

**Analysis** — formula language (`A + B + A*B + A^2`, `(A+B+C)^2 - B*C`),
presets, hierarchical models, stepwise AICc, backward elimination, automatic
model and transform search ranked by predicted R² on the original scale,
ANOVA with lack of fit, coefficients with VIF and confidence intervals,
diagnostics, Box-Cox, Lenth/half-normal screening, canonical analysis,
profiler, contours, 3D surfaces, interaction plots, signal-to-noise analysis,
and equations in actual or coded units as plain text, LaTeX, Excel, Python or
C.

**Optimisation** — Derringer-Suich desirability over multiple responses with
goals, limits, weights and importance; held factors; desirability profiler;
confirmation runs.

**Data entry** — spreadsheet editing with typing, Enter/Tab navigation,
clipboard exchange with spreadsheets, an always-present new row, level pickers,
undo/redo, CSV import with column mapping (also for observational data), CSV
export, HTML reports and R scripts that refit every model with `lm()` and
compare the results with sdoe's.

## Development aid

With `SDOE_SCREENSHOT_DIR=/some/dir`, the application loads the reactor
example, steps through the main views, saves a PNG of each and exits (works with
`QT_QPA_PLATFORM=offscreen`).

## License

This project is licensed under the GNU General Public License v3.0. See
[LICENSE](LICENSE) for the full text.
