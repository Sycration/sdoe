# Projects, setup and data

This page covers the project file, the Setup page (factors and responses), the Runs page (data entry), importing and exporting data, and the exported reports.

Related pages: [designs](designs.md) for run generation, [models](models.md) for model terms, [analysis](analysis.md) for the statistics and [optimisation](optimisation.md) for goals and the Next run dialog.

## Projects

A project holds the title, notes, factors, responses, runs, the model chosen for each response, the last design and the optimisation settings. It is stored in one file with the extension `.sdoe` (JSON text). Developers can find the layout in [API.md](../API.md#project-file-format).

### File menu

| Item | Shortcut | Effect |
|---|---|---|
| New | Ctrl+N | Starts an empty project and shows the Setup page. |
| Open… | Ctrl+O | Opens a `.sdoe` file. |
| Open example | | Loads one of the two built-in examples (see below). |
| Save | Ctrl+S | Saves to the current file. Asks for a name when the project has none. |
| Save As… | Ctrl+Shift+S | Saves under a new name. The window then uses that file. |
| Import data… | | Reads a CSV file. See [Importing data](#importing-data). |
| Import from clipboard | | Reads delimited text from the clipboard. |
| Export runs (CSV)… | | See [Exporting runs as CSV](#exporting-runs-as-csv). |
| Export report (HTML)… | | See [HTML report](#html-report). |
| Export analysis (R)… | | See [R script](#r-script). |
| Quit | Ctrl+Q | Closes the program. |

The shortcuts are the standard ones of the platform. The window title shows the file name, or "Untitled", and a dot when there are unsaved changes. New, Open, Open example and Quit then ask "Save changes to the current project?" with Save, Discard and Cancel.

In the web build, files are uploaded and downloaded through the browser instead of file dialogs, and Quit is absent.

Opening a file fails with a message when the file is not a project, was saved by a newer version of the program, or breaks a rule the sheets enforce (for example a continuous factor with Low not below High, a response with a lower limit above the upper limit, or a condition that refers to a missing factor).

### Pages

The tabs are Setup, Design, Runs, Analysis, Optimize and Evaluate. The View menu and Ctrl+1 to Ctrl+6 switch between them.

### Examples

File > Open example offers two projects with simulated data:

| Example | Content |
|---|---|
| Reactor with measured humidity | Temperature, Time, Catalyst (categorical), Stirring (discrete) and Humidity (environmental), responses Yield and Impurity, 26 runs from an optimal design, and a fitted Yield model. |
| Polymer blend (mixture) | Three mixture components (Resin, Hardener, Filler) with lower bounds, response Strength, and a simplex lattice design with data. |

An example opens on the Analysis page. The `examples/` directory of the source tree holds more projects, which File > Open… loads. The names end in `-planned` (a design without data), `-template` (factors and responses only) or neither (a design with data). `field-datacentre-cooling-log.sdoe` has no design: it holds observational data.

### Undo and redo

Edit > Undo (Ctrl+Z) and Edit > Redo (Ctrl+Shift+Z or Ctrl+Y, depending on the platform) step through the changes to the project. Each of these is one step:

- one cell edit;
- one paste, one clear, one row insertion or deletion;
- one title or notes edit;
- one design application, import, randomisation or sort.

The history holds up to 200 steps. New, Open and Open example clear it. Saving does not clear it. After an undo the project counts as changed.

## Setup page

The Setup page has the project title, the Factors sheet, the Responses sheet and a Notes field. The title is saved when the field loses focus; so is the notes text. The report and the R script include the title and the notes.

When the project has a mixture factor, a Mixture total field appears next to the title. It holds the sum that the mixture components must reach (default 1). It must be a positive number. Other text is rejected with an error, and the total keeps its value.

Both sheets end with an empty row. Typing a name in that row adds a factor or response. Other cells of that row add an entry with a default name when a value is typed.

### Editing sheets

The Factors, Responses and Runs sheets share the same editing keys.

| Action | Effect |
|---|---|
| Type a character | Starts editing the current cell with that character. |
| Enter, F2 or double-click | Edits the current cell with its text selected. |
| Enter | Commits and moves down. |
| Tab, Shift+Tab | Commits and moves right or left. |
| Up, Down (while editing) | Commits and moves up or down. |
| Esc | Cancels the edit. |
| Arrow keys | Move the current cell. |
| Click, Shift+click, Ctrl+click | Select cells. |
| Click a row number | Selects the whole row. |
| Ctrl+A | Selects all rows. |
| Delete, Backspace | Clears the selected cells. |
| Ctrl+C, Ctrl+X, Ctrl+V | Copy, cut and paste (see below). |
| Right-click | Opens the context menu. |
| Drag a column border | Resizes the column. |

Cells of fixed choices (Type, Role, Goal, Status, categorical level columns) open a list. Typing a letter in such a cell selects the first entry that starts with that letter. Hovering over a column header shows a tooltip.

A rejected entry shows its reason in the status bar and leaves the cell unchanged.

**Context menu.** Copy, Paste, Clear, Insert row and Delete rows exist on every sheet. Insert row adds an entry at the current row. Delete rows deletes the selected entries. On the Runs sheet, Mark planned, Mark done and Exclude from analysis set the status of the selected runs.

**Clipboard.** Copy produces tab-separated text of the selected cells without a header, so the text pastes into spreadsheet programs. Paste reads text from the current cell onward. Rows beyond the end of the sheet add entries. The delimiter is a tab if the text contains one, otherwise a semicolon, otherwise a comma when the text has several lines. Cells that cannot be set are listed in the status bar (six at most); the other cells of the paste are applied.

Deleting rows from the Factors or Responses sheet also removes the data that depends on them:

- deleting a factor removes its values from all runs, the model terms that contain it, its optimisation settings, and the conditions of other factors that refer to it;
- deleting a response removes its values and its model.

### Factors

One row per factor. The columns are:

| Column | Meaning | Input |
|---|---|---|
| Symbol | Short name used in model formulas and conditions. | Starts with a letter; letters, digits and `_` only; unique without regard to case. New factors take A, B, C, … (I and O are skipped), then AA, AB, …. |
| Name | Full name. | Not empty. Appears in column headers, plots and reports. |
| Units | Free text, such as `°C`. | Optional. Shown in square brackets in headers. |
| Type | Continuous, Discrete, Categorical or Mixture. | Choice; see below. |
| Role | Controlled, Noise, Environmental or Block. | Choice; see below. |
| Low, High | Range of continuous and mixture factors. | Numbers with Low below High. Read-only for other types. |
| Levels | Values of discrete and categorical factors. | Comma-separated list. Read-only for other types. |
| Condition | Rule that makes the factor apply in some runs only. | See [Conditional factors](#conditional-factors). |

A new factor is Continuous on -1 to 1 with the role Controlled. Numbers accept a decimal point or a single decimal comma (`1,5`).

For a continuous factor, a Low at or above the current High moves High by the same amount, and a High at or below the current Low moves Low by the same amount. A range can therefore be entered in either order: to change -1 to 1 into 30 to 70, enter 30 in Low (High becomes 32), then 70 in High. For a mixture component, Low must stay below High and High above Low.

#### Types

| Type | Values | Notes |
|---|---|---|
| Continuous | Any number from Low to High. | |
| Discrete | One of at least two listed numbers, such as `200, 400, 600`. | Levels are sorted and duplicates are removed. Low and High show the smallest and largest level. Treated as a numeric factor with the range of its levels. |
| Categorical | One of at least two named levels, such as `X, Y`. | Level names may be separated by commas, semicolons or tabs. Duplicates are removed. Matching ignores case. |
| Mixture | A proportion between Low and High. | The components of a mixture must add up to the Mixture total. See [designs](designs.md) and [models](models.md) for how mixtures are designed and modelled. |

A type can also be entered by its first letters (`cat`, `mix`). Changing the type converts the existing values:

| Change | Result |
|---|---|
| Continuous to Discrete | Levels are Low, the midpoint and High. |
| Any type to Mixture | Low is 0 and High is the Mixture total. |
| Discrete to Categorical | The levels become their number texts. |
| Anything else to Categorical | Levels are `L1` and `L2`. |
| Categorical to a numeric type | The range is 0 to 1 (Discrete: 0, 0.5, 1). Run values that are not numbers are removed. |
| Categorical run values | A value that is not a level of the new list is kept and marked as an error on the Runs sheet. |

Model terms that no longer fit the new type are removed from the models, and so are conditions that no longer fit.

Editing the Levels of a categorical factor works in two ways:

- With the same number of levels, the levels are renamed by position. Run values and conditions follow the new names. Changing `Air, Water` to `Air, Steam` renames Water to Steam everywhere.
- With a different number of levels, run values keep their text. A value that is no longer a level is marked as an error.

#### Roles

| Role | Meaning |
|---|---|
| Controlled | The experimenter sets the factor in the experiment and in production. |
| Noise | The experimenter sets the factor in the experiment, but it varies at random in production. Used in robust design. |
| Environmental | The experimenter cannot set the factor. Its value is measured and entered for each run. It enters the model as a covariate. |
| Block | A grouping of runs, such as a day or a batch. Designs with blocks create this factor. |

Differences that follow from the role:

- Designs assign values to Controlled and Noise factors. Environmental and Block factors are not designed ([designs](designs.md)).
- Environmental factors accept new values. A categorical or discrete level that is entered or imported for an environmental factor is added to its level list. For other roles, a value outside the levels is rejected.
- The Low and High of an environmental factor give the expected range. Importing data widens a continuous environmental range to cover the imported values.
- Block factors and mixture components cannot be conditional, and other factors cannot depend on a block factor.

The Runs page colours the columns by role: Environmental (measure), Noise and Response each have their own tint and header accent. Block columns have a tint without accent.

### Responses

One row per response. Name and Units describe the measured quantity. The remaining columns set the goal that the optimiser uses. See [optimisation](optimisation.md) for how they combine into a desirability.

| Column | Meaning | Default |
|---|---|---|
| Name | Not empty. | `R1`, `R2`, … for responses created from another column. |
| Units | Free text. | Empty. |
| Goal | None, Maximize, Minimize, Target or In range. | None. |
| Lower | Lower limit of acceptable values. | Empty: the observed minimum. |
| Target | Value to reach. Editable only when the Goal is Target. | Empty. |
| Upper | Upper limit of acceptable values. | Empty: the observed maximum. |
| Weight | Shape of the desirability curve. Below 1 is lenient, above 1 is strict. Limited to 0.1 to 10. | 1 |
| Importance | Relative importance in the overall desirability, 1 to 5. | 3 |

Checks: Lower must not exceed Upper, Target must lie between Lower and Upper, and Weight must be positive. Empty Weight and Importance fall back to the defaults. Values outside the limits of Weight and Importance are brought to the nearest limit.

### Conditional factors

A conditional factor applies only when another factor (its parent) meets a condition. A defrosting method matters only when the temperature is below 0, and a catalyst loading matters only when a catalyst is used.

Type the condition in the Condition column.

| Parent type | Operators | Examples |
|---|---|---|
| Continuous, Mixture | `<`, `<=`, `>`, `>=` with one number | `T < 0`, `Temp >= -5` |
| Discrete | `<`, `<=`, `>`, `>=` with one number; `=`, `!=` with a list of levels | `N <= 2`, `N = 1, 3` |
| Categorical | `=` (any of the levels) and `!=` (none of the levels) | `M = Freeze, Flash`, `M != Air` |

Rules for the text:

- The parent is named by symbol or by name. Case is ignored.
- `==` and `<>` are accepted for `=` and `!=`; `≤`, `≥` and `≠` for `<=`, `>=` and `!=`.
- List items are separated by commas or semicolons.
- The sheet shows the condition with the parent's symbol, whatever name was typed. If the parent's symbol changes later, the condition follows the factor.
- A condition can depend on a factor that is itself conditional. The factor then applies only where all of its ancestors apply.
- Clearing the cell removes the condition.

An entry is rejected when the condition:

- refers to the factor itself, an unknown factor or a block factor, or creates a cycle;
- sits on a block factor or a mixture component;
- uses an operator that does not fit the parent (`<` on a categorical factor, `=` on a continuous one);
- names a value that is not a level of the parent;
- holds for every value or for no value of the parent;
- has a threshold outside the parent's range. `<=` at the low end and `>=` at the high end are allowed.

Messages from a project with Temperature (A, -20 to 20), Defrosting (B, categorical: Air, Water) and Pressure (C):

| Condition typed for Pressure | Message |
|---|---|
| `Temperature < 40` | the value must lie inside the range of A (-20 to 20) |
| `Temperature = 0` | A is continuous; use <, <=, > or >= |
| `Defrosting < 1` | B is categorical; use = or != with its levels |
| `Temperature` | write a condition such as T < 0 or M = A, B |

The program removes a condition when its parent is deleted, or when a change of the parent's type, levels or role makes the condition invalid. A change of the parent's range keeps the condition.

#### Example

```
Symbol  Name         Type         Low/High or Levels   Condition
A       Temperature  Continuous   -20 to 20
B       Defrosting   Categorical  Air, Water           A < 0
```

Defrosting applies in runs with a temperature below 0 only.

#### What happens where the factor does not apply

- **Runs sheet.** The cell is blank, has the background of the page and cannot be edited. It does not count as a missing value. Text typed or pasted into it does not raise an error. A value that fits the factor is kept and reappears when the factor applies again. Other text, such as `n/a`, leaves the cell empty.
- **Model.** Every model column that contains the factor is 0 in that run. For a numeric factor, 0 is the coded centre. For a categorical factor, 0 means no contrast. The effects of the factor come from the runs where it applies. See [models](models.md).
- **Designs.** Designs leave the factor blank in the runs where it does not apply ([designs](designs.md)).
- **Equations, optimisation, reports.** Equations in actual units come as one equation per region. Tables show a dash for the factor. The HTML report gains an Applies when column. The R script gives NA.

#### Example of entered runs

With the factors above, a paste starting at the Temperature column of the Runs sheet:

```
-20	Air
-20	Water
20	Air
```

The first two rows are kept. In the third row the temperature is 20, so Defrosting does not apply and the pasted `Air` is not stored.

## Runs page

The Runs page is the run sheet: one row per run, and an empty row at the end for new runs.

### Columns

| Column | Content | Editable |
|---|---|---|
| Run | Run order. | No |
| Std | Standard order, as given by the design. | No |
| Type | Point type: `factorial`, `axial`, `center`, `optimal`, … from a design, `added` for a run typed in or inserted, `imported` for a run from a CSV file. | No |
| Status | Planned, Done or Excluded. | Yes |
| One column per factor | Value in actual units. The header reads `Symbol: Name [units]`. | Yes |
| One column per response | Measured value. The header reads `Name [units]`. | Yes |
| Note | Free text. | Yes |

Typing in the empty row adds a run with the point type `added`. The Run, Std and Type cells are read-only.

### Cell colours

| Appearance | Meaning |
|---|---|
| Yellow tint | A value is missing: a factor setting that is empty, or a response without a measurement. |
| Warning colour text | A continuous or mixture value outside the range of its factor. The value is still used. |
| Error colour text | A categorical value that is not a level. |
| Grey text, struck through for responses | The run is Excluded. |
| Empty, flat cell | The factor does not apply in this run. |
| Grey text | Read-only cell. |

### Entering values

| Column type | Accepted input |
|---|---|
| Continuous, mixture | A number. A value outside the range is accepted and flagged. |
| Discrete | One of the levels. The editor is a drop-down with free entry. A value that is not a level is rejected, except for an environmental factor, which gains the level. |
| Categorical | One of the levels, chosen from a list. Matching ignores case. A value that is not a level is rejected, except for an environmental factor, which gains the level. |
| Response | A number. |

Clearing a cell removes the value. Entering a response value sets a Planned run to Done. Clearing the response does not set it back to Planned.

The status bar below the sheet shows "N of M runs done", with a progress bar. Only runs with the status Done count.

### Excluded runs

Status Excluded leaves a run out of all analyses. The row stays in the sheet and in the CSV export. In the R script the run stays in the `runs` table and is dropped from the fit by the `keep` vector. Use Exclude from analysis in the context menu, or set the Status cell, for outliers and failed runs. Set the status back to Done to include the run again.

### Buttons

| Button | Effect |
|---|---|
| Randomize order | Assigns a random run order and sorts the sheet by it. When the project has a block factor, runs stay grouped by block and the order is random within each block. Each use gives a new order. |
| Sort > By run order | Sorts the rows by the Run column. |
| Sort > By standard order | Sorts the rows by the Std column. |
| Next run… | Suggests settings for one more run given the current environmental conditions. Enabled when the project has factors. See [optimisation](optimisation.md). |
| Import… | Opens the import dialog. |
| Export… | Exports the runs as CSV. Enabled when there are runs. |

Sorting changes the row order only. It does not renumber the runs.

## Importing data

File > Import data… reads a CSV file. File > Import from clipboard reads text from the clipboard, for example cells copied from a spreadsheet program. The Import… button of the Runs page is the same as Import data…. Both open the Import data dialog.

Requirements for the text:

- a header line and at least one data line;
- a delimiter that the program detects from the first lines: tab, semicolon or comma;
- double quotes around fields that contain the delimiter or line breaks; `""` inside quotes stands for one quote;
- empty lines are skipped, and a leading byte-order mark is ignored.

The decimal separator is a point. A single decimal comma is also read (`1,5`) when the text has no point, so with a comma delimiter, write such numbers in quotes or use a semicolon as the delimiter.

### The dialog

The dialog shows the row and column counts and one line per column: the header, up to three sample values and a list of targets. Choose the target for each column.

| Target | Effect |
|---|---|
| Ignore | The column is not imported. |
| New controlled factor | Creates a factor with the role Controlled. |
| New environmental factor | Creates a factor with the role Environmental. |
| New noise factor | Creates a factor with the role Noise. |
| New response | Creates a response. |
| Run order, Standard order | Sets the order numbers of the runs. |
| Status | Sets the status (Planned, Done or Excluded; the first letters suffice). |
| Note | Sets the run note. |
| Factor *symbol*: *name* | Fills an existing factor. |
| Response *name* | Fills an existing response. |

Each existing factor, response or run field accepts one column. If two columns share a target, the dialog shows "More than one column has the same target." and the Import button stays disabled. Several columns may be ignored or create new factors.

Choose Append runs (default) to add the rows after the existing runs, or Replace runs to remove all existing runs first. Replace runs keeps the factors, responses and models. The status bar then reports "Imported N runs." followed by messages.

### Suggested targets

The dialog suggests a target for each column. Before matching, the program removes a `Symbol: ` prefix and a ` [units]` suffix from the header.

| Header (case ignored) | Suggestion |
|---|---|
| Name of an existing factor | That factor |
| Name of an existing response | That response |
| `run`, `run order`, `runorder` | Run order |
| `std`, `std order`, `standard order`, `stdorder` | Standard order |
| `status` | Status |
| `note`, `notes`, `comment` | Note |
| `type`, `point type` | Ignore |
| A column with no values | Ignore |
| The last numeric column, when the project has no response yet | New response |
| Anything else | New controlled factor |

A file exported with Export runs (CSV)… therefore maps back onto the same factors and responses. The suggestions are only a start: change them in the dialog.

### What the import creates

- A new factor from a numeric column is Continuous from the smallest to the largest value of the column. If all values are equal, High is Low + 1.
- A new factor from any other column is Categorical. Its levels are the distinct values in order of first appearance. A column with a single value gets an extra level `Other`. A numeric column with one stray entry such as `n/a` therefore becomes categorical, with every value as a level.
- New factors get the next free symbol. The name is the header, without `Symbol: ` prefix and ` [units]` suffix. Units are left empty and must be filled in afterwards. Text in parentheses, such as `(C)`, stays in the name.
- A new response takes the header as its name and the default goal settings.
- Imported runs have the point type `imported`.
- Empty cells leave the value missing.
- Without a Status column, a run with a response value is Done and any other run is Planned.
- Without Run order or Standard order columns, the numbers continue after the highest existing number (Append) or start at 1 (Replace).

For an existing factor, a cell that cannot be read is left empty and counted. After the import the status bar reports the count ("N cells could not be read and were left empty."). A cell is unreadable when:

- a number column holds text, or a response cell is not a number;
- a categorical or discrete value is not one of the levels. An environmental factor gains the level instead, and the cells of a conditional factor are skipped without a count (marks such as `n/a`).

These are the same rules as for entering a value on the Runs sheet.

Ranges: an environmental factor widens to cover the imported values. For a controlled or noise factor the range stays. The status bar then shows "Some values of *name* lie outside its range." and the Runs sheet marks those cells.

### Example: importing observational data

A cooling log recorded without a design:

```
Date,Inlet temp (C),Fan speed,Mode,Power kW
2026-01-03,18.5,40,Eco,52.1
2026-01-04,21,55,Eco,58.4
2026-01-05,24.5,70,Boost,71.9
2026-01-06,19,55,Boost,66.0
```

The program suggests New controlled factor for the first four columns and New response for `Power kW`. A mapping that fits the log:

| Column | Target | Result |
|---|---|---|
| Date | Ignore | |
| Inlet temp (C) | New environmental factor | Continuous, 18.5 to 24.5. Measured, not set. |
| Fan speed | New controlled factor | Continuous, 40 to 70. |
| Mode | New controlled factor | Categorical: Eco, Boost. |
| Power kW | New response | Response `Power kW`. |

The import creates 4 runs of the point type `imported` with the status Done. Rename the factors, enter their units and set the roles and ranges on the Setup page.

If the Fan speed column held `n/a` in one row, it would become a categorical factor with the levels `40, 55, 70, n/a`. Change the cell to blank before the import to keep it numeric.

Observational data has no design: the factor settings are whatever occurred. The run table then lacks the balance of a designed experiment, and the factors can be correlated with each other. [Analysis](analysis.md) explains the variance inflation factors that show this.

## Exporting runs as CSV

File > Export runs (CSV)… (also the Export… button of the Runs page) writes the run sheet as a comma-delimited file with a header line.

- The columns are those of the Runs sheet: Run, Std, Type, Status, the factors (`A: Temperature [°C]`), the responses (`Texture [unit]`) and Note.
- All runs are written, including Excluded runs.
- Numbers are written at full precision.
- Cells that are missing or where a conditional factor does not apply are empty.
- Fields that contain a comma, a quote or a line break are quoted.

Example for the project with the conditional Defrosting factor:

```
Run,Std,Type,Status,A: Temperature [°C],B: Defrosting,Texture,Note
1,1,added,Done,-20,Air,7.5,
2,2,added,Done,-20,Water,8.1,
3,3,added,Done,20,,6.2,
4,4,added,Done,20,,6,
5,5,added,Planned,5,,,
```

The file imports again through the import dialog, which maps its headers back to the factors and responses of the project. Units are not recovered for new factors (see above).

## HTML report

File > Export report (HTML)… writes one self-contained HTML file. It has no external files and shows in any browser. The plots are embedded as SVG. Print the page from the browser to produce a PDF.

The report contains:

- the project title and notes;
- a table of the factors (symbol, name, units, type, role, range or levels), with an Applies when column if any factor is conditional;
- the design method and the design notes, when a design was applied;
- the number of runs and how many are Done;
- for each response, the model formula (and the transform when one is set), warnings about the fit, the model statistics, the analysis of variance, the coefficients in coded units, the equation in actual units (one per region for conditional factors), a stationary-point or canonical analysis line when one applies, and four plots: predicted against actual, normal plot of residuals, Pareto chart of effects and residuals against fitted values.

The report uses the model that is currently set for each response (the default model of the design unless it was changed on the Analysis page). A response without a usable fit shows the reason in place of the results. Excluded runs are left out of the fits. The report does not contain the run table, the optimisation results or the Evaluate page. The statistics are explained in [analysis](analysis.md).

## R script

File > Export analysis (R)… writes a script, `analysis.R` by default, that repeats the model fits in R and checks the results of sdoe against R. Use it to confirm the numbers independently or to continue the analysis in R. The script requires R 4.0 or newer and base R only.

Run it with `Rscript analysis.R`, or source it in an R session. It prints the results for each response and ends with a comparison.

### Structure

| Section | Content |
|---|---|
| Header comments | Title, notes, factor and response list with their R names, the design method. |
| Runs (actual units) | The data frame `runs`, written as an inline table. Factor columns are named by symbol (`A`, `B`, …), response columns by name made into valid R identifiers (`Tensile strength (MPa)` becomes `Tensile_strength_MPa`). Values of conditional factors are `NA` where the factor does not apply. |
| Coding | The function `code()`, which converts actual values to the coded model columns exactly as sdoe does: continuous and discrete factors to -1 … +1, mixture components to pseudo-components, categorical factors to sum-to-zero effect coding, conditional factors to 0 where they do not apply. The vector `keep` removes Excluded runs. |
| Analysis | The function `analyse()`, then for each response a call of `lm()` with the model of sdoe (including its transform and the back-transformation of predictions). Terms that sdoe aliases are left out of the formula and listed in a comment. |
| Comparison with sdoe | The values that sdoe computed, and `check()` calls that compare them with the R results. |

Responses without a fit in sdoe get a comment `# No fit in sdoe: …` and no R model.

### Output of one response

For each response, the script prints:

1. Statistics: runs, standard deviation, mean, coefficient of variation, R², adjusted R², predicted R², adequate precision, PRESS, AICc and BIC;
2. the analysis of variance with partial tests per term, lack of fit and pure error where replicates exist, and the corrected total;
3. the coefficients in coded units with standard errors, t values, p values, 95% confidence intervals and variance inflation factors;
4. the prediction at the centre point with 95% confidence and prediction intervals. The centre point uses the first level of each categorical factor.

[Analysis](analysis.md) defines these quantities.

### Reading the comparison

The last lines of the output come from `check()`. It compares every value that the program computed (statistics, ANOVA table, coefficient table, prediction) with the value from R. Two values agree when they differ by less than 1e-6, relative to the value, or relative to the largest value of the same kind when the value is near 0.

Output for the reactor example:

```
Yield: all 156 values agree with sdoe
Impurity: all 189 values agree with sdoe
```

The number is the count of values compared. When some values differ, the line reads `Yield: 3 of 156 values differ from sdoe`, followed by a table with the columns `value` (which table), `term`, `sdoe` and `R`.

The objects `fit_<response>` (the `lm` fit), `res_<response>` (the results list), `runs`, `coded` and `points` stay available in the R session after the script runs.
