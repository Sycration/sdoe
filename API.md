# sdoe engine API

The engine (`sdoe-core`) holds the project data, the undo history and every
calculation. Frontends send it commands: a method name plus a JSON object of
parameters. The engine has no file system access. Commands take and return
text (project JSON, CSV, HTML), and the caller reads and writes the files.

- [Calling the engine](#calling-the-engine)
- [Common conventions](#common-conventions)
- [Project](#project)
- [Sheets](#sheets)
- [Runs, factors and responses](#runs-factors-and-responses)
- [Conditional factors](#conditional-factors)
- [Design](#design)
- [Models](#models)
- [Analysis](#analysis)
- [Evaluation](#evaluation)
- [Optimisation](#optimisation)
- [Import and export](#import-and-export)
- [Plot format](#plot-format)
- [Project file format](#project-file-format)

## Calling the engine

### C

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

| Function | Description |
|---|---|
| `SdoeEngine *sdoe_engine_new(void)` | Creates an engine with an empty project. |
| `void sdoe_engine_free(SdoeEngine *)` | Destroys an engine. `NULL` is ignored. |
| `char *sdoe_call(SdoeEngine *, const char *method, const char *params_json)` | Runs a command. `params_json` may be `NULL` or `""` for no parameters. Returns a newly allocated UTF-8 JSON string. |
| `void sdoe_string_free(char *)` | Releases a string returned by `sdoe_call` or `sdoe_version`. |
| `char *sdoe_version(void)` | Library version, e.g. `"0.1.0"`. |

Calls on one engine are serialised internally.

### JavaScript (WebAssembly)

```js
import { init, Engine } from "./sdoe.js";
await init();                       // loads sdoe.wasm next to sdoe.js
const e = new Engine();
e.call("project.example", { name: "reactor" });
const fit = e.call("analysis.fit", { response: "6" });
if (fit.ok) console.log(fit.result.stats);
e.free();
```

The module runs in browsers, Node.js 18+, Deno and workers. `sdoe.wasm`
has no imports. `sdoe.d.ts` contains the TypeScript definitions.

| Function | Description |
|---|---|
| `init(source?)` | Loads the WebAssembly module; returns a promise. Without `source`, it loads `sdoe.wasm` from the directory of `sdoe.js` (with `fetch`, or from the file system in Node.js). `source` may be a URL or path string, a `URL`, a `Response` (or a promise of one), the module bytes or a `WebAssembly.Module`. Further calls return the same promise. |
| `initSync(bytes)` | Loads the module synchronously from its bytes or a `WebAssembly.Module`. |
| `version()` | Library version, e.g. `"0.1.0"`. |
| `new Engine()` | Creates an engine with an empty project. Requires `init()`. |
| `engine.call(method, params?)` | Runs a command. `params` is an object or JSON text and may be omitted. Returns the parsed reply. |
| `engine.callJson(method, paramsJson?)` | Same with JSON text in and out. |
| `engine.free()` | Destroys the engine; further calls throw. Engines are also freed after garbage collection, and `using e = new Engine()` frees at the end of the block. |

Each engine has its own project. A Rust panic stops the WebAssembly instance
in an unknown state: the call returns `{"ok":false,"error":"internal error …"}`,
the instance and its engines are dropped, and `init()` loads a new instance.

### Rust

`sdoe::Engine::call(&mut self, method, params) -> String` takes and returns
the same JSON. `Engine::dispatch(method, &Map)` returns the result as a
`Result<serde_json::Value, String>`. The typed modules (`design`, `model`,
`optimize`, `condition`, …) are public as well.

### Replies

```json
{"ok": true, "result": <value>, "changed": <bool>, "revision": <n>}
{"ok": false, "error": "<message>"}
```

`changed` is true when the command changed the project data. `revision`
increases with every change, including undo and redo. Parameters that are not
a JSON object produce an error.

## Common conventions

- **Ids.** Factors and responses are referred to by id, given as a string
  (`"6"`) in parameters and results. `factors.list` and `responses.list` list
  them.
- **Values.** Factor values are text in actual units: numbers (`"12.5"`,
  `"1e-3"`) or categorical level names (case-insensitive).
- **Points.** Point parameters (`values`, `env`) are objects
  `{factor_id: text}`. Factors left out of the object, or given as empty
  text, are set to their centre: the midpoint of continuous factors, the level
  nearest the midpoint for discrete factors, an equal share for mixture
  components, and the first level for categorical factors.
- **Tables** are objects
  `{columns: [..], rows: [[..]], tags: [..], emphasis: [..], numeric: [bool..]}`.
  `tags` holds one string per row for frontend actions (e.g. the model term
  of a row). `emphasis` holds one entry per row: `""`, `good`, `bad`, `muted`
  or `strong`.
- **Undo.** Every command that changes the project adds an undo step. A
  failed command makes no change to the project.

## Project

| Method | Parameters | Result |
|---|---|---|
| `app.version` | — | Version string. |
| `project.new` | — | Empty project; clears undo/redo and the design preview. |
| `project.open` | `text`: project file text | Loads the project; clears undo/redo and the design preview. |
| `project.save` | — | `{text}`: project file text (see [Project file format](#project-file-format)). |
| `project.mark_saved` | — | Clears the unsaved-changes flag. |
| `project.state` | — | See below. |
| `project.set` | `title`, `notes`, `mixture_total` (> 0); all optional | Sets the given fields. |
| `project.example` | `name`: `reactor` (default) or `blend` | Loads an example project. |
| `edit.undo` | — | Error `nothing to undo` when the history is empty. |
| `edit.redo` | — | Error `nothing to redo` when nothing was undone. |

`project.state` returns:

| Key | Meaning |
|---|---|
| `title`, `notes` | Project text. |
| `dirty` | Unsaved changes. |
| `can_undo`, `can_redo` | History state. |
| `revision` | Change counter. |
| `epoch` | Increases when a different project is loaded (`new`, `open`, `example`). |
| `factors`, `responses`, `runs`, `done` | Counts (`done`: runs with status Done). |
| `mixture_total` | Total of the mixture components (default 1). |
| `has_mixture`, `has_env` | The project has mixture components / environmental factors. |
| `design` | `{method, notes}` of the last applied design, or `null`. |

## Sheets

Three spreadsheet views of the project: `factors`, `responses` and `runs`.
Each sheet ends with an empty placeholder row; typing into it adds an entry.

| Method | Parameters | Result |
|---|---|---|
| `sheet.get` | `sheet` | Sheet (see below). |
| `sheet.set` | `sheet`, `row`, `col`, `text` | Sets one cell from text. |
| `sheet.paste` | `sheet`, `row`, `col`, `text` | Pastes delimited text from the given cell onward, adding rows as needed. Returns `{errors: [..]}` for cells that could not be set. The delimiter is a tab if the text contains one, otherwise `;`, otherwise `,` for multi-line text. |
| `sheet.copy` | `sheet`, `top`, `left`, `bottom`, `right`, `header` (default false) | `{text}`: tab-separated cell texts; `header` adds the column titles. |
| `sheet.insert_row` | `sheet`, `row` | Inserts a new entry at `row`. |
| `sheet.delete_rows` | `sheet`, `rows`: array of row indices | Deletes entries. Deleting a factor also removes its run values, the model terms that contain it, its optimisation settings and conditions that refer to it. Deleting a response removes its values and model. |

A sheet is `{columns, rows, placeholder}`:

- `columns[]`: `{title, kind, tip, editor, choices, numeric}`
  - `kind`: `meta`, `factor`, `noise`, `env`, `block`, `response`, `text`
  - `editor`: `text`, `choice` (fixed list in `choices`), `combo` (list with
    free entry), `none`
- `rows[][]`: `{text, state, editable}`
  - `state`: `normal`, `readonly`, `needed` (value to enter), `warn` (outside
    the factor range), `excluded` (excluded run), `placeholder`, `error`
    (value is not a level), `inactive` (conditional factor that does not
    apply in this run)
- `placeholder`: the last row is the empty row for new entries.

### Factors sheet

| Col | Title | Input |
|---|---|---|
| 0 | Symbol | Starts with a letter; letters, digits and `_`; unique (case-insensitive). Used in formulas and conditions. |
| 1 | Name | Not empty. Typing a name in the placeholder row creates a factor. |
| 2 | Units | Free text. |
| 3 | Type | `Continuous`, `Discrete`, `Categorical` or `Mixture` (a prefix is enough). Run values are converted; model terms and conditions that no longer fit are removed. |
| 4 | Role | `Controlled`, `Noise`, `Environmental` or `Block` (a prefix is enough). |
| 5 | Low | Continuous and mixture factors only; below High. |
| 6 | High | Continuous and mixture factors only; above Low. |
| 7 | Levels | Discrete: at least two numbers separated by `,` `;` spaces or tabs. Categorical: at least two names separated by `,` `;` or tabs. With the same number of categorical levels, levels are renamed in place (in runs and conditions). |
| 8 | Condition | See [Conditional factors](#conditional-factors). Empty: the factor always applies. Not available for block factors and mixture components. |

New factors are continuous on [-1, 1], controlled, with the next free symbol
(A–Z without I and O, then AA, AB, …).

Roles:

- *Controlled*: set in the experiment and in production.
- *Noise*: set in the experiment, random in production (robust design).
- *Environmental*: cannot be set; measured at every run and modelled as a
  covariate. Unknown categorical or discrete values entered for an
  environmental factor extend its level list.
- *Block*: grouping of runs, created by blocked designs.

### Responses sheet

| Col | Title | Input |
|---|---|---|
| 0 | Name | Not empty. Typing a name in the placeholder row creates a response. |
| 1 | Units | Free text. |
| 2 | Goal | `None`, `Maximize`, `Minimize`, `Target`, `In range`. |
| 3 | Lower | Lower acceptable value (empty: observed minimum). |
| 4 | Target | Used with goal Target. |
| 5 | Upper | Upper acceptable value (empty: observed maximum). |
| 6 | Weight | Desirability shape, 0.1–10 (default 1): below 1 lenient, above 1 strict. |
| 7 | Importance | 1–5 (default 3). |

### Runs sheet

| Col | Title | Input |
|---|---|---|
| 0 | Run | Run order (read-only). |
| 1 | Std | Standard order (read-only). |
| 2 | Type | Point type, e.g. `factorial`, `axial`, `center`, `optimal`, `added` (read-only). |
| 3 | Status | `Planned`, `Done`, `Excluded`. Excluded runs are left out of all analyses. |
| 4 … | one column per factor | Value in actual units; empty for a missing value. |
| … | one column per response | Measured value. Entering a value sets a Planned run to Done. |
| last | Note | Free text. |

Typing in the placeholder row adds a run with point type `added`.

## Runs, factors and responses

| Method | Parameters | Result |
|---|---|---|
| `runs.randomize` | `seed` (optional) | Random run order (within blocks). |
| `runs.sort` | `by`: `run` (default) or `std` | Sorts the rows. |
| `runs.set_status` | `rows`: array of row indices, `status`: `Planned`, `Done` or `Excluded` | Sets the status. |
| `factors.list` | — | `[{id, symbol, name, units, kind, role, numeric, categorical, low, high, levels, center, condition}]`. `kind`: `Continuous`, `Discrete`, `Categorical`, `Mixture`. `levels`: level texts of discrete and categorical factors. `center`: centre value as text. `condition`: condition text (empty when unconditional). |
| `responses.list` | — | `[{id, name, units, goal, count}]`. `count`: runs with a value. |

## Conditional factors

A factor can apply only when another factor (its parent) meets a condition.
For example, a defrosting method only matters when the temperature is below
0. The condition is entered in column 8 (Condition) of the factors sheet:

| Parent type | Operators | Examples |
|---|---|---|
| Continuous, mixture | `<`, `<=`, `>`, `>=` with one number | `T < 0`, `Temp >= -5` |
| Discrete | `<`, `<=`, `>`, `>=` with one number; `=`, `!=` with a list of levels | `N <= 2`, `N = 1, 3` |
| Categorical | `=` (any of), `!=` (none of) with a list of levels | `M = Freeze, Flash`, `M != Air` |

- The parent is given by symbol or name (case-insensitive). The engine
  stores the parent by id, so the condition stays valid when the parent's
  symbol is renamed.
- `==` and `<>` are accepted for `=` and `!=`, and `≤`, `≥`, `≠` for
  `<=`, `>=`, `!=`. List items are separated by `,` or `;`.
- Conditions chain: a factor whose parent does not apply does not apply
  either.
- The engine rejects a condition that:
  - refers to the factor itself or to an unknown factor;
  - creates a cycle;
  - refers to a block factor, or is set on a block factor or a mixture
    component;
  - uses an operator that does not fit the parent type;
  - names values that are not levels of the parent;
  - holds for every value or for no value of the parent;
  - puts a threshold outside the parent's range. `<=` at the low end and
    `>=` at the high end are allowed.
- The condition is removed when the parent is deleted, or when it is no
  longer valid after a change of the parent's type, levels or role. It
  stays after range changes. Renaming categorical levels in place renames them in
  the condition as well.

Where a factor does not apply:

- **Runs sheet.** The cell has state `inactive`, is blank and is not
  editable, and it does not count as a missing value. Values sent to it
  through `sheet.set`, `sheet.paste` or `import.apply` produce no error.
  A valid value is kept and shows up when the factor applies again; other
  text such as `n/a` clears the cell.
- **Model.** Every model column that contains the factor is 0: the coded
  centre for numeric factors, no contrast for categorical factors. The
  factor's effects come from the runs where it applies. This holds for
  fitting, prediction, plots, design evaluation, optimal designs, *Next
  run* and optimisation.
- **Model presets.** Presets do not include the interaction of a factor with a
  parent (or further ancestor) that takes a single value wherever the
  factor applies (e.g. `M*P` for `M` with condition `P = Frozen`). That
  column repeats the factor's main effect.
- **Designs.** The factor is blank in generated runs. A full factorial
  contains each valid combination once per replicate. Optimal designs are
  built with the conditions taken into account. In other design types,
  the points are placed without regard to the conditions, and a design
  note states this. The notes also give the number of runs in which each
  conditional factor is set.
- **Equations.** In actual units, `analysis.equation` gives one equation
  per region, labelled e.g. `Temperature >= 0` and
  `Thawing = Fast, Temperature < 0`. In coded units, the text begins
  with a note that terms with the factor are 0 where it does not apply.
  The note is a comment in Python (`#`), C (`//`) and LaTeX (`%`) syntax,
  and is left out for Excel.
- **Optimisation and Next run.** Tables show `—` for the factor, and its
  entry in `values` is empty text.
- **Data plot.** Runs where the factor does not apply are not part of the
  `data` plot of `analysis.plot`.
- **Report.** The factor table gains an *Applies when* column.

## Design

| Method | Parameters | Result |
|---|---|---|
| `design.methods` | — | Method catalogue for the current factors (see below). |
| `design.preview` | `method`, `options` (object, see below) | `{runs, blocks, notes, table, model, augment}`. The preview is kept for `design.apply` and `evaluate.run`. `model`: formula of the model the design supports. `augment`: the design adds to the existing runs. |
| `design.apply` | `mode`: `replace` (default) or `append`; `randomize` (default true); `seed` (optional) | Writes the previewed runs. `replace` also clears the runs and models; augmenting designs always append. Blocked designs create or update a categorical *Block* factor. Returns `{runs}`. |
| `design.next_run` | `env`: point with the current environmental values; `mode`: `info` (default) or `best`; `formula` (mode `info`, optional) | `{table, values, predictions}`. `info`: settings with the largest prediction variance for the model (default: the model of the first response). `best`: optimum desirability for the current environment. `values`: `{factor_id: text}`. `predictions`: text lines with the predicted responses and 95% prediction intervals. |
| `design.add_run` | `values`: `{factor_id: text}`; `type` (point type, default `added`) | Adds a run. Returns `{row}`. |

`design.methods` returns
`[{id, name, group, description, options, available, reason}]`; `reason`
explains why a method is not available. Each option is
`{key, label, kind, default, min, max, choices, tooltip}` with `kind` `int`,
`float`, `bool`, `text` or `choice`, and `choices` as `[[value, label]]`.
Option values may be sent as JSON numbers, booleans or strings.

Designs set controlled and noise factors only. Environmental factors are
recorded at each run (except with known values in optimal designs).

### `full_factorial`: Full factorial

Every combination of factor levels; any mix of continuous, discrete and
categorical factors. Available with controlled or noise factors and without mixture
components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `levels` | int | 2 | 2–7: equally spaced levels per continuous factor |
| `center_points` | int | 0 | 0–50; categorical factors cycle through their levels |
| `replicates` | int | 1 | 1–20 |
| `block_replicates` | bool | false | Each replicate becomes a block |

### `fractional_factorial`: Fractional factorial (2-level)

Regular 2^(k−p) fraction with minimum-aberration generators; notes show the
resolution and alias structure. Available with at least 3 two-level factors
(categorical factors with exactly 2 levels) and without mixture components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `runs` | choice | `auto` | `auto` (smallest with the minimum resolution) or a run count `4`, `8`, `16`, … |
| `resolution` | choice | `4` | `3`, `4`, `5`; used with `runs` = `auto` |
| `generators` | text | `""` | e.g. `D=ABC, E=-ABD` |
| `center_points` | int | 0 | 0–50 |
| `replicates` | int | 1 | 1–20 |
| `foldover` | bool | false | Adds the mirror image of the fraction |

### `plackett_burman`: Plackett-Burman

Two-level orthogonal screening design for main effects in a multiple of 4
runs. Available with at least 2 two-level factors and without mixture components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `runs` | int | 0 | 0–96; 0 = smallest; otherwise a supported multiple of 4 |
| `center_points` | int | 0 | 0–50 |

### `dsd`: Definitive screening

Three-level screening design from conference matrices. Main effects are
clear of two-factor interactions, and quadratic effects are estimable.
Available with at least 3 continuous or two-level categorical factors and without mixture
components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `extra_center` | int | 0 | 0–10 |

### `ccd`: Central composite

Factorial core plus axial and centre points for quadratic models;
categorical factors are crossed with the design. Available with at least 2 numeric
factors and without mixture components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `alpha` | choice | `rotatable` | `rotatable`, `orthogonal`, `face` (α = 1), `spherical` (α = √k), `practical` (α = k^¼) |
| `inscribed` | bool | false | Scales the design so all points stay within the ranges |
| `fraction` | choice | `auto` | `auto` (resolution V fraction when smaller), `full` |
| `center_points` | int | 5/6/7/6/9/10 for k = 2/3/4/5/6/more | 0–50 |
| `blocks` | choice | `1` | `1` (none), `2` (factorial / axial) |

### `box_behnken`: Box-Behnken

Three-level response surface design without corner points. Available with at least 3
numeric factors and without mixture components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `center_points` | int | 5 (≤ 5 numeric factors), else 6 | 0–50 |

### `optimal`: Optimal (custom)

Coordinate-exchange optimal design for any combination of continuous,
discrete, categorical and mixture factors, any model and any run count.
Conditional factors are taken into account. Available with controlled or noise factors.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `criterion` | choice | `D` | `D` (parameter estimation), `I` (prediction), `A` |
| `model` | choice | `quadratic` (≤ 6 factors), else `2fi` | `linear`, `2fi`, `quadratic`, `cubic`, `custom` |
| `formula` | text | `""` | Formula for `model` = `custom`, e.g. `(A+B+C)^2 + A^2` |
| `runs` | int | 0 | 0–1000; 0 = model coefficients + 6, or the number of known environmental values |
| `augment` | bool | false | The existing runs stay, and the new runs complement them |
| `starts` | int | 20 | 1–500 random starts |
| `seed` | int | 1 | 0–1 000 000 000 |
| `env_interactions` | bool | false | Only with environmental factors: adds environment × factor interactions |
| `env_<factor id>` | text | `""` | Only with environmental factors: known or forecast values, one per run, separated by commas, spaces or new lines. With fewer values than runs, the values repeat in order. Environmental factors without values leave the design model. |

### `lhs`: Latin hypercube

Maximin space-filling design; discrete and categorical factors are balanced.
Available with controlled or noise factors and without mixture components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `runs` | int | 10 × number of factors | 2–5000 |
| `iterations` | int | 2000 | 0–200 000 maximin iterations |
| `seed` | int | 1 | 0–1 000 000 000 |

### `simplex_lattice`: Simplex lattice

Mixture blends on a regular lattice of the pseudo-component simplex.
Available with at least 2 mixture components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `degree` | int | 2 | 1–6 |
| `centroid` | bool | true | Adds the overall centroid |
| `axial` | bool | false | Adds axial check blends |
| `replicates` | int | 1 | 1–10 |

### `simplex_centroid`: Simplex centroid

All vertex, edge, face and overall centroids; supports the special cubic
model. Available with at least 2 mixture components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `axial` | bool | false | Adds axial check blends |
| `replicates` | int | 1 | 1–10 |

### `taguchi`: Taguchi (robust parameter)

Orthogonal-array inner design for controlled factors crossed with an outer
design for noise factors; analyse with `analysis.robustness`. Available with
controlled or noise factors and without mixture components.

| Option | Kind | Default | Range / choices |
|---|---|---|---|
| `levels` | choice | `2` | `2`, `3` levels per continuous factor |
| `inner` | choice | `auto` | `auto` (smallest that fits), `L4`, `L8`, `L9`, `L12`, `L16`, `L16b` (4-level), `L18`, `L25`, `L27`, `L32` |
| `outer` | choice | `auto` | `auto` (full factorial when small, else array), `full`, `array` |

## Models

Each response has its own model (terms and transform). Without a stored
model, a response uses the model of the last applied design (plus the
measured environmental and block factors), or a default model.

| Method | Parameters | Result |
|---|---|---|
| `model.get` | `response` | `{formula, transform, terms, factors, presets, syntaxes, transforms}`. `factors`: ids of the factors in the model. `presets` and `syntaxes`: `[[key, label]]`. |
| `model.set` | `response`; `formula`, `transform`, `hierarchy` (default false); all optional | Sets the formula and/or transform. `hierarchy` adds every missing lower-order term. |
| `model.preset` | `response`, `preset`, `env_interactions` (default false) | Replaces the terms with a preset; the transform stays. |
| `model.remove_term` | `response`, `term`: term label, e.g. `A*B` | Removes a term that is not part of a higher-order term in the model. |
| `model.select` | `response`, `method`, `alpha`, `pool`, `env_interactions` | Term selection; returns `{steps, formula}`. |
| `model.auto` | `response`, `env_interactions` (default true) | Automatic model and transform search; sets the best candidate and returns `{table, formula, transform}`. |

**Presets** (`linear`, `2fi`, `quadratic`, `cubic`): for designed factors,
the full polynomial structure (interactions up to order 2 or 3, squares and
cubes where the factor type allows them); for mixture components, Scheffé
products. Environmental factors enter as main effects; with
`env_interactions`, also as interactions with designed factors (and squared
terms for quadratic and cubic). Block factors enter as main effects.
`main` and `interactions` are accepted for `linear` and `2fi`.

**`model.select`**:

- `method`: `aicc` (default) runs stepwise selection by AICc from the
  linear terms; `backward` runs backward elimination by p-value.
- `alpha`: removal threshold for `backward` (default 0.10).
- `pool`: candidate terms; `current` (default) or a preset name, merged
  with the current terms.
- `env_interactions`: used when `pool` is a preset (default false).

**`model.auto`** ranks candidate models by predicted R² on the original
scale. The candidates combine the transforms None, Ln, Sqrt and Inverse
(only those valid for the data) with a quadratic term pool (cubic when the
data allow it).

**Transforms**: `None`, `Ln` (`log`), `Log10`, `Square root` (`sqrt`),
`Inverse` (`1/y`), `Power <λ>` (e.g. `Power 0.5`).

### Formula language

| Syntax | Meaning |
|---|---|
| `A + B` | Terms. |
| `A*B`, `A:B`, `A×B`, `A·B` | Product (interaction); `(A+B)*C` expands to `A*C + B*C`. |
| `A^2`, `A²`, `A³` | Power. |
| `(A+B+C)^2` | All products of up to 2 distinct terms of the group. |
| `- B*C` | Removes a term. |
| `1` | Intercept (always included when the model has one; mixture models have none). |
| `` `Feed rate` `` or `"Feed rate"` | Factor by quoted name. |

Factors are looked up by symbol (exact, then case-insensitive), then by
name (exact, then case-insensitive). Categorical factors and mixture components have no
powers, and discrete factors support powers up to their number of levels
minus 1.

## Analysis

| Method | Parameters | Result |
|---|---|---|
| `analysis.fit` | `response` | `{response, formula, transform, stats, anova, coefficients, warnings, canonical, lenth, df_resid}`. `stats`: `[[label, value]]`: runs used, standard deviation, mean, C.V. %, R², adjusted R², predicted R², adequate precision, PRESS, AICc, BIC. `anova` and `coefficients` are tables; coefficients are in coded units with standard error, t, p, 95% confidence interval and VIF. `canonical`: text lines of the canonical analysis. `lenth`: `{pse, me, sme, used}` of the Lenth screening. |
| `analysis.equation` | `response`, `units`: `actual` (default) or `coded`, `syntax`: `plain` (default), `latex`, `excel`, `python`, `c` | `{text, count}`. Actual units: one equation per combination of categorical levels (and per region of conditional factors), at most 64. With a transform, Plain and LaTeX show both the transformed and the solved form. |
| `analysis.predict` | `response`, `values` (point) | `{y, ci, pi}`: prediction with 95% confidence and prediction intervals. |
| `analysis.plot` | `response`, `kind`, … | Plot (see below). |
| `analysis.robustness` | `response`, `sn`: `nominal` (default), `larger`, `smaller` | Signal-to-noise analysis grouped by controlled-factor settings: table of groups with n, mean, standard deviation and S/N, mean S/N and mean response per level, and the best level of each factor. `nominal` needs at least 2 observations per group. |

`analysis.plot` kinds and parameters:

| `kind` | Extra parameters | Plot |
|---|---|---|
| `pred_actual` | — | Predicted vs actual. |
| `resid_fitted` | — | Residuals vs predicted. |
| `resid_normal` | — | Normal probability plot of residuals. |
| `resid_order` | — | Residuals vs run order. |
| `cooks` | — | Cook's distance. |
| `boxcox` | — | Box-Cox log-likelihood over λ. |
| `pareto` | — | Pareto chart of standardised effects. |
| `halfnormal` | — | Half-normal plot of effects. |
| `profiler` | `values` | One plot per model factor with 95% CI, the other factors at `values`. Returns `{plots, prediction}`. |
| `interaction` | `x`, `y` (factor ids), `values` | Response over `x` for levels of `y`. |
| `contour` | `x`, `y`, `values` | Contour plot (heat map) over two factors. |
| `surface` | `x`, `y`, `values`, `azimuth` (default 35), `elevation` (default 30) | Projected 3D surface. |
| `data` | `x` | Raw response values against one factor with a least-squares line; no model required. |

## Evaluation

| Method | Parameters | Result |
|---|---|---|
| `evaluate.run` | `source`: `runs` (default) or `preview`; `formula` or `preset` (+ `env_interactions`); `snr` (default 2) | Design evaluation for a model. |

Without a formula or preset, the model is the preview's model (source
`preview`) or the default model. `snr` is the effect size relative to σ
used for power. The result is
`{formula, stats, table, warnings, correlation, fds}`:

- `stats`: runs, model coefficients, residual df, D- and G-efficiency, A,
  average prediction variance.
- `table`: per term, the standard error / σ, VIF and power at α = 0.05.
- `correlation`: heat map plot of |r| between model terms.
- `fds`: fraction-of-design-space plot.

## Optimisation

Derringer-Suich desirability over the responses with a goal (Responses
sheet).

| Method | Parameters | Result |
|---|---|---|
| `optimize.get` | — | `{env, fixed, goals}`. `env`: noise and environmental factors with `{id, name, symbol, role, categorical, levels, mode, value, mean, sd}`. `fixed`: controlled factors with `{id, name, symbol, categorical, levels, value}`. `goals`: `{name, goal, lower, target, upper}`. |
| `optimize.set` | `env`, `fixed` (see below) | Stores the settings. |
| `optimize.run` | `env`, `fixed` (optional, stored first), `solutions` (default 8) | `{table, solutions, notes, robust}`. `solutions`: `[{values, d}]` best first. `robust`: desirability averaged over random factors. |
| `optimize.profile` | `values` (point) | Desirability profiler: `{rows, desirability, values}`. `rows`: plot rows, one per response plus the desirability, with one plot per decision factor. `values`: predicted responses at the point. |
| `optimize.control_law` | `factor`: id of an environmental factor | Best controlled settings over the range of the environmental factor: `{plots, formulas, table}`. `formulas`: fitted polynomial adjustment laws, e.g. `Feed = 12 + 0.3*Humidity (R² = 0.998)`. |

Settings:

- `env`: `{factor_id: {mode, value, mean, sd}}` for noise and environmental
  factors.
  - `mode`: `fixed` holds the factor at `value` (default: its centre).
    `random` averages over a distribution: numeric factors use `mean` and
    `sd` (default: observed values), categorical factors weight their
    levels equally.
  - The default mode is `random` for noise factors and `fixed` for
    environmental factors.
- `fixed`: `{factor_id: text}` holds controlled factors at a value. Empty
  text frees the factor.

## Import and export

| Method | Parameters | Result |
|---|---|---|
| `import.preview` | `text`: CSV or other delimited text | `{delimiter, rows, columns, targets}`. `columns[]`: `{name, numeric, distinct, min, max, suggestion, sample}`. `targets`: `[[key, label]]`. |
| `import.apply` | `text`, `mapping`: one target key per column, `mode`: `append` (default) or `replace` | Imports runs; returns `{runs, new_factors, new_responses, skipped_cells, messages}`. |
| `export.csv` | — | `{text}`: the runs sheet as comma-separated text with a header line. |
| `export.report` | — | `{html}`: HTML report with the factors, the design notes, run counts and, for each response, the model, statistics, ANOVA, coefficients and actual-unit equations. |
| `export.r` | — | `{text}`: R script (base R only), see below. |

The R script contains the runs in actual units, a `code()` function that
converts actual values to the engine's coded model columns (including
conditional factors), and for each response with a fit the model as an
`lm()` call with the response's transform. Running it prints the statistics,
the analysis of variance (partial tests, lack of fit), the coefficients with
confidence intervals and VIF, and the prediction at the centre point with
95% confidence and prediction intervals. Its last section holds the engine's
own values and lists those that differ from the R results by more than 1e-6
(relative); without differences it prints `<response>: all <n> values agree
with sdoe`. Model terms that are aliased in every column are left out of the
R formula.

Import target keys: `ignore`, `new_factor`, `new_env`, `new_noise`,
`new_response`, `meta:run`, `meta:std`, `meta:status`, `meta:note`,
`factor:<id>`, `response:<id>`. Headers in the export format
(`S: Name [units]`) map back to existing factors and responses. New factors
are continuous over the observed range when the column is numeric, and
categorical otherwise. Cells that do not fit their factor count as
`skipped_cells`, except in columns of conditional factors.

## Plot format

Plots are geometry for any frontend to draw:

```
Plot    {title, x_label, y_label, x: Axis, y: Axis, series: [Series],
         heatmap: Heatmap|null, kind, legend, value_range, note}
Axis    {min, max, ticks: [{v, label}], hidden}
Series  {name, kind, x, y, labels, values, ids, color, dashed}
Heatmap {nx, ny, x0, x1, y0, y1, values, vmin, vmax, diverging,
         row_labels, col_labels}
```

- Plot `kind`: `xy`, `3d` (projected polygons) or `matrix`.
- Series `kind`: `points`, `line`, `linepoints`, `bars`, `band`, `hline`,
  `vline`, `segments`, `labels`, `polygons`.
- `values`: per-point or per-polygon values for colour mapping, with
  `value_range` as the colour scale.
- `ids`: run numbers for click-through.
- `color`: palette slot (0… series colours, −1 muted, −2 reference,
  −3 highlight).
- Heat map `values` are row-major from (x0, y0).

## Project file format

`project.save` produces JSON (format version 1):

```json
{
  "format": 1,
  "title": "Frozen samples",
  "notes": "",
  "factors": [
    {"id": 1, "symbol": "T", "name": "Temperature", "units": "°C",
     "kind": {"type": "continuous", "low": -20.0, "high": 20.0},
     "role": "controlled"},
    {"id": 2, "symbol": "D", "name": "Defrosting", "units": "",
     "kind": {"type": "categorical", "levels": ["Air", "Water"]},
     "role": "controlled",
     "condition": {"factor": 1, "op": "<", "values": [0.0]}}
  ],
  "responses": [
    {"id": 3, "name": "Texture", "units": "", "goal": "maximize",
     "lower": null, "target": null, "upper": null, "weight": 1.0, "importance": 3.0}
  ],
  "runs": [
    {"id": 4, "std_order": 1, "run_order": 1, "point_type": "optimal", "status": "planned",
     "settings": {"1": -20.0, "2": "Air"}, "responses": {"3": 7.5}, "note": ""}
  ],
  "models": {"3": {"terms": [[[1, 1]], [[2, 1]]], "transform": {"type": "none"}}},
  "mixture_total": 1.0,
  "design": {"method": "Optimal (custom)", "notes": ["…"], "model": [[[1, 1]]]},
  "optimize": {"env": {}, "fixed": {}},
  "next_id": 5
}
```

- `kind.type`: `continuous` and `mixture` (`low`, `high`), `discrete`
  (`levels`: numbers), `categorical` (`levels`: names).
- `role`: `controlled`, `noise`, `environmental`, `block`.
- `condition` (optional): `factor` is the parent id; `op` is one of `<`,
  `<=`, `>`, `>=`, `=`, `!=`; `values` holds numbers, or level names for
  categorical parents.
- `goal`: `none`, `maximize`, `minimize`, `target`, `in_range`.
- `status`: `planned`, `done`, `excluded`.
- `settings`, `responses`: values keyed by factor or response id; a missing
  key is a missing value.
- Model terms: lists of `[factor_id, power]` pairs; the empty list is the
  intercept.
- `transform.type`: `none`, `ln`, `log10`, `sqrt`, `inverse`, or `power`
  with `lambda`.
- `optimize.env`: `{factor_id: {mode, value, mean, sd}}`;
  `optimize.fixed`: `{factor_id: value}`.
- Top-level fields other than `format` and `next_id` may be left out and
  take their defaults.
