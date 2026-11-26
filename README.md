# sdoe — design of experiments

A design-of-experiments application with a Rust engine and a Qt Quick desktop frontend.

```
core/      sdoe-core: all mathematics and project data handling (rlib + cdylib)
app/       sdoe-app: Qt Quick (QML) desktop frontend, uses sdoe-core as a crate
include/   sdoe.h: C header for the shared library
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

The interface uses the desktop's Qt Quick Controls style (Windows, macOS,
`org.kde.desktop`/Breeze on KDE Plasma, Fusion elsewhere on Linux) and colours
from the system palette, including dark mode. On Linux, the desktop style
depends on the session environment (`XDG_CURRENT_DESKTOP`, `WAYLAND_DISPLAY`),
which terminals and launchers inside the desktop session provide.

## C interface

```c
#include "sdoe.h"
SdoeEngine *e = sdoe_engine_new();
char *r = sdoe_call(e, "project.example", "{\"name\":\"reactor\"}");
sdoe_string_free(r);
r = sdoe_call(e, "analysis.fit", "{\"response\":\"6\"}");   /* JSON result */
sdoe_string_free(r);
sdoe_engine_free(e);
```

```sh
cc demo.c -Iinclude -Ltarget/release -lsdoe
```

Every call returns `{"ok":true,"result":…,"changed":bool,"revision":n}` or
`{"ok":false,"error":"…"}`. Rust programs use `sdoe::Engine::call` (same JSON)
or the typed modules (`design`, `model`, `optimize`, …) directly.

### Commands

| Area | Commands |
|---|---|
| Project | `project.new`, `project.open {text}`, `project.save` → `{text}`, `project.mark_saved`, `project.state`, `project.set {title, notes, mixture_total}`, `project.example {name: reactor\|blend}`, `edit.undo`, `edit.redo` |
| Sheets | `sheet.get {sheet}`, `sheet.set {sheet,row,col,text}`, `sheet.paste {sheet,row,col,text}`, `sheet.copy {sheet,top,left,bottom,right}`, `sheet.insert_row`, `sheet.delete_rows {rows}` — sheets: `factors`, `responses`, `runs` |
| Runs | `runs.randomize`, `runs.sort {by: run\|std}`, `runs.set_status {rows,status}`, `factors.list`, `responses.list` |
| Design | `design.methods`, `design.preview {method, options}`, `design.apply {mode: replace\|append, randomize}`, `design.next_run {env, mode: info\|best}`, `design.add_run {values}` |
| Models | `model.get`, `model.set {formula, transform, hierarchy}`, `model.preset {preset, env_interactions}`, `model.remove_term {term}`, `model.select {method: aicc\|backward}`, `model.auto` |
| Analysis | `analysis.fit`, `analysis.equation {units: actual\|coded, syntax: plain\|latex\|excel\|python\|c}`, `analysis.predict {values}`, `analysis.plot {kind,…}`, `analysis.robustness {sn}` |
| Evaluation | `evaluate.run {source: runs\|preview, preset\|formula, snr}` |
| Optimisation | `optimize.get`, `optimize.set {env, fixed}`, `optimize.run`, `optimize.profile {values}`, `optimize.control_law {factor}` |
| Exchange | `import.preview {text}`, `import.apply {text, mapping, mode}`, `export.csv`, `export.report` |

Plot kinds: `pred_actual`, `resid_fitted`, `resid_normal`, `resid_order`,
`cooks`, `boxcox`, `pareto`, `halfnormal`, `profiler`, `contour`, `surface`,
`interaction`, `data`. Plots are returned as geometry (axes, ticks, series,
heat maps, projected 3D polygons) for any frontend to draw.

## Capabilities

**Factors** — continuous, discrete numeric (fixed levels), categorical and
mixture components, each with a role:

- *Controlled*: set in the experiment and in production.
- *Noise*: set in the experiment, random in production (robust design).
- *Environmental*: cannot be set; measured at every run and modelled as a
  covariate (temperature of the day, raw-material batch, humidity…).
- *Block*: grouping created by blocked designs.

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
export and HTML reports.

## Development aid

With `SDOE_SCREENSHOT_DIR=/some/dir`, the application loads the reactor
example, steps through the main views, saves a PNG of each and exits (works with
`QT_QPA_PLATFORM=offscreen`).

## License

This project is licensed under the GNU General Public License v3.0. See
[LICENSE](LICENSE) for the full text.
