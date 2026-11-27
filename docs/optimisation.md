# Optimisation

On the Optimize page, the program searches the factor space for the settings that give the best compromise between all response goals, based on the models fitted on the Analysis page and the goals set on the Responses sheet. This page also covers environmental and noise factors, adjustment laws, and the Next run dialog on the Runs page.

Related pages:

- Goals, limits, weights and importances are entered on the Responses sheet: [setup-and-data.md](setup-and-data.md).
- Model terms, transformations and fit statistics: [models.md](models.md) and [analysis.md](analysis.md).
- Factor roles and design construction: [designs.md](designs.md).
- The JSON method names behind the page are listed in [../API.md](../API.md#optimisation).

## Requirements

- At least one response has a goal other than `None`.
- Each response with a goal has a fitted model. A response without a model is skipped and the Solutions tab shows a note.
- Hold values lie inside the factor range. A value outside it is rejected with an error message.

The program predicts every response on the original scale. If a response model uses a transformation, the prediction is transformed back before the desirability is computed. With block factors, the prediction is the average over all block levels.

## Goals and desirability

Each response with a goal is converted to an individual desirability `d` between 0 (unacceptable) and 1 (fully acceptable). The overall desirability `D` combines them. The method is the one of Derringer and Suich.

### Goal settings

These columns are set on the Responses sheet. The Goals list at the top of the Optimize page shows them read-only, as `name: goal [lower - upper]`, with `-> target` for goal Target.

| Option | Meaning | Default |
|---|---|---|
| Goal | `None`, `Maximize`, `Minimize`, `Target` or `In range`. | `None` |
| Lower | Lower limit of acceptable values. | Smallest observed value of the response |
| Target | Ideal value. Used by goal `Target` only. | Midpoint of Lower and Upper |
| Upper | Upper limit of acceptable values. | Largest observed value of the response |
| Weight | Shape exponent `w` of the desirability curve, 0.1 to 10. Below 1 the curve is lenient, above 1 it is strict. | 1 |
| Importance | Relative importance of the response in `D`, 1 to 5. | 3 |

Lower, Target and Upper must be in ascending order. Weight must be positive.

### Individual desirability

`y` is the predicted response. `L`, `T`, `U` are Lower, Target, Upper. `w` is Weight.

| Goal | Formula |
|---|---|
| `Maximize` | `d = 0` for `y <= L`; `d = ((y - L) / (U - L))^w` for `L < y < U`; `d = 1` for `y >= U` |
| `Minimize` | `d = 1` for `y <= L`; `d = ((U - y) / (U - L))^w` for `L < y < U`; `d = 0` for `y >= U` |
| `Target` | `d = 0` for `y < L` or `y > U`; `d = ((y - L) / (T - L))^w` for `L <= y <= T`; `d = ((U - y) / (U - T))^w` for `T < y <= U`; `d = 1` at `y = T` |
| `In range` | `d = 1` for `L <= y <= U`; `d = 0` outside |
| `None` | The response does not enter `D`. Its prediction is still shown in the result table. |

The weight bends the curve between the limits:

- `w = 1`: `d` rises linearly.
- `w > 1`: `d` stays low until `y` is close to the goal. A response must be near its target to score well.
- `w < 1`: `d` rises quickly. A response far from the goal still scores acceptably.

Example, goal `Minimize`, `L = 50`, `U = 250`, `y = 150`:

```
w = 1:   d = (250 - 150) / 200        = 0.50
w = 2:   d = 0.50^2                   = 0.25
w = 0.5: d = 0.50^0.5                 = 0.71
```

Limits that are too narrow make `d` zero for most of the factor space. Limits left at the observed range make the best observed value the only point with `d = 1` for goals `Maximize` and `Minimize`. Set Lower and Upper to the values that are acceptable in practice.

### Overall desirability

`D` is the weighted geometric mean of the individual desirabilities, taken over the responses with a goal:

```
D = d1^(I1/S) * d2^(I2/S) * ... * dk^(Ik/S),   S = I1 + I2 + ... + Ik
```

`Ii` is the Importance of response `i`. The exponents add up to 1, so `D` lies between 0 and 1.

- `D = 0` as soon as one response has `d = 0`. A single unacceptable response makes the whole setting unacceptable.
- A response with higher Importance has a larger exponent, so its `d` has more influence on `D`.
- `D = 1` only if every response is fully acceptable.

Worked example (project `field-heat-pump-heating-curve`). Electricity: `Minimize`, `L = 50`, `U = 250`, Importance 3. Discomfort: `Minimize`, `L = 0`, `U = 6`, Importance 4. At the point with Electricity 237.1 and Discomfort 3.0613:

```
d(Electricity) = (250 - 237.1) / 200  = 0.0645
d(Discomfort)  = (6 - 3.0613) / 6     = 0.4898
D = 0.0645^(3/7) * 0.4898^(4/7)       = 0.2054
```

The program reports `D = 0.2054` for this point.

## The Optimize page

The page has a settings panel on the left and the results on the right.

### Settings panel

| Control | Meaning |
|---|---|
| Goals | Read-only list of the goals and limits of every response. Goals set to `None` are greyed. |
| Environment and noise | One entry per environmental and noise factor. See [Environmental and noise factors](#environmental-and-noise-factors). |
| Hold factors | One check box per controlled factor. When checked, the factor is held at the value in its field and is not searched. When unchecked, the factor is free within its range. |
| Optimize | Runs the search. |
| Adjustment law | Factor list and the Compute button. See [Adjustment law](#adjustment-law). Shown only when the project has a numeric environmental factor. |

Hold factors:

- A numeric factor takes a value inside its range. A categorical factor takes one of its levels.
- Mixture components can be held. The held components reduce the sum that the free components must reach. The program reports an error if the held components leave no valid blend within the component bounds.
- Settings in the panel are stored with the project when Optimize or Compute is pressed.

### Search

Pressing Optimize runs the following search:

1. The program builds the list of free decision factors: controlled factors that are not held.
2. Categorical factors are enumerated over their levels. Discrete factors are also enumerated when the number of level combinations is at most 48. With more combinations, 48 random combinations of categorical levels are used and discrete factors are searched as continuous factors and rounded to the nearest level afterwards.
3. For each combination, 30 random starting points are scored. The four best starts are refined with a Nelder-Mead simplex search. A warm start from a previous solution is used by the adjustment law.
4. The levels of discrete and categorical factors are then adjusted one factor at a time, and the continuous factors are searched again with the levels held. This repeats up to three times while the score improves.
5. The results are sorted by `D`. Settings closer than 0.04 coded units on every numeric factor, with identical levels, count as the same solution. Only the best of such a group is kept.

The search uses a fixed random seed. The same project and settings give the same result. The result is a local search from many starts and does not prove that no better setting exists.

The Solutions tab lists up to 8 solutions. The list can be shorter, because duplicates are removed.

### Solutions tab

| Column | Meaning |
|---|---|
| `#` | Rank by `D`. Row 1 is highlighted. |
| Desirability | Overall desirability `D` of the setting. |
| One column per free decision factor | Setting in original units. A factor that a condition makes inactive shows `—`. |
| One column per modelled response | Predicted value. In robust mode, the mean over the scenarios. |
| `<response> SD` | Robust mode only. Standard deviation of the prediction over the scenarios. |

Notes above the table:

- `Robust optimisation: desirability averaged over N combinations ...`: at least one environmental or noise factor is set to Random.
- `No setting satisfies all goals; the table lists the closest settings. Widen the limits on the Responses page.`: the best `D` is 0. The search then ranks settings by closeness to the goals, so the list is still ordered.
- `<response> has no fitted model`: the response is left out.

Click a row to select a solution. The profiler below the table shows that solution.

Example, project `field-heat-pump-heating-curve`, environmental factors at their defaults (Outdoor temperature 1, Solar irradiation 1.85):

```
#  Desirability  Supply temperature  Fan speed  Preheat start  Electricity  Discomfort
1  0.7158        46.732              55.277     3.5            158.35       0
```

Electricity gives `d = (250 - 158.35) / 200 = 0.458`. Discomfort is 0, so `d = 1`. Then `D = 0.458^(3/7) * 1^(4/7) = 0.7157`.

## Desirability profiler

Below the solution table the page shows a grid of small plots for the selected solution.

- One column per free decision factor. One row per response with a goal or a model, plus a last row, Desirability.
- Each plot shows the predicted response (or `D`) while one factor varies over its range. All other factors stay at the selected solution.
- The dashed vertical line marks the current setting of the factor.
- Categorical factors show one point per level. Discrete factors show points at their levels. Mixture components vary over their feasible range, and the other free components rescale in proportion so that the blend keeps its total.
- The bold line above the plots, Desirability, gives `D` at the current setting.

Click in a plot to move that factor to the clicked value. All plots and `D` update. For a categorical factor the click selects the nearest level. The modified setting replaces the selected solution until the next Optimize run. Use this to test how much `D` falls when a factor is moved away from the optimum, for example to a rounder value.

The plots show the response curves that the optimum depends on. A flat curve means the factor has little effect on that response near the solution. A curve of `D` with a sharp peak means that the solution is sensitive to the factor.

## Confirmation runs

The button Add as confirmation run appears above the profiler. It adds one planned run to the Runs sheet:

- The point type is `confirmation`.
- The factor values are those of the selected solution, including any change made in the profiler.
- Environmental factors set to Fixed take their fixed value. Environmental and noise factors set to Random take the centre of their range.
- The run has status `planned` and no response values.

The program shows the message `Added a planned run on the Runs page.`

To compare a confirmation run with the prediction:

1. Perform the run and enter the measured responses on the Runs page.
2. Compare them with the predicted values in the Solutions table (the response columns of the same row).
3. A measured value outside the prediction interval of the model indicates that the model is inadequate at that point. Prediction intervals are shown for a setting by the Next run dialog (see below) and by the prediction tools in [analysis.md](analysis.md).

The Optimize page itself does not compute the interval and does not mark the difference.

## Environmental and noise factors

Factors with the role Environmental or Noise are not set by the experimenter during use. The Environment and noise section lists them with their role colour. Each has a mode selector.

| Factor role | Meaning | Default mode |
|---|---|---|
| Environmental | A condition that varies and can be measured when the process runs (outdoor temperature, load, batch of material). The controlled factors can be adjusted to it. | Fixed |
| Noise | A condition that varies and is not adjusted to. | Random |

### Fixed

The factor is held at one value for the search. The value editor shows the current value. The default is the centre of the factor range. Use Fixed to optimise for today's conditions.

Example, same project, cold and dull day (Outdoor temperature -5, Solar irradiation 0.5):

```
#  Desirability  Supply temperature  Fan speed  Preheat start  Electricity  Discomfort
1  0.2054        46.741              68.164     0.5            237.1        3.0613
```

The same controlled factors reach only `D = 0.2054` in this condition, against 0.7158 in the default condition.

### Random

The optimisation averages over a distribution of the factor and is robust to its variation.

| Field | Meaning | Default |
|---|---|---|
| Mean | Mean of the distribution (numeric factors). | Mean of the observed values in the runs |
| SD | Standard deviation of the distribution (numeric factors). | Standard deviation of the observed values in the runs |

If no values are observed, the mean is the centre of the factor range and the SD is a quarter of its range.

How the distribution is represented:

- A numeric factor is represented by three values with weights 1/6, 2/3, 1/6: `mean - sqrt(3)*SD`, `mean`, `mean + sqrt(3)*SD`. These match the mean and the SD of the distribution.
- A categorical factor is represented by all its levels, each with equal weight.
- Each combination of values of the random factors is a scenario. With 243 combinations or fewer, all combinations are used. With more, the program uses 200 random scenarios drawn with a fixed seed.

For each scenario the program computes the predicted responses and the overall desirability `D`. The result for a setting is the weighted mean of `D` over the scenarios. The program maximises this mean.

In the table, the predicted response is the mean over the scenarios, and `<response> SD` is the standard deviation over the scenarios. The SD column measures how much the response varies with the random factors at that setting. A solution with a low SD is robust.

The mean of `D` is not the same as `D` of the mean responses. A setting with a large spread has some scenarios with `D = 0`, which lowers the mean.

Example, Outdoor temperature Random with Mean 0 and SD 4, Solar irradiation Fixed at 1:

```
Robust optimisation: desirability averaged over 3 combinations of the random environmental and noise factors.
#  Desirability  Supply temperature  Fan speed  Preheat start  Electricity  Electricity SD  Discomfort  Discomfort SD
1  0.5740        45.832              53.92      3.5            179.01       47.67           0.7206      1.611
```

### Noise factors and robust design

Noise factors are random by default, so the search is robust to them without further setting. The program does not minimise the variance as a separate goal. It treats the variation indirectly: at a setting where a change of a noise factor moves a response out of its limits, some scenarios have a low `d`, and the mean `D` falls. Settings that make the response insensitive to the noise factor therefore score higher. This is effective only if the model contains interaction terms between the noise factor and the controlled factors. Without such terms the response varies with the noise factor in the same way at every setting. The SD column then differs little between solutions. For model terms see [models.md](models.md). For designs that include noise factors see [designs.md](designs.md).

A noise factor can be changed to Fixed to optimise for one noise condition.

## Adjustment law

An adjustment law gives the best setting of each controlled factor as a formula of one environmental factor. It replaces repeated optimisation runs when the condition changes during operation.

Select a factor in the list under Adjustment law and press Compute. The list contains the numeric environmental factors. The program stores the current settings of the panel and then:

1. Takes 13 equally spaced values of the selected factor from the lower to the upper end of its range.
2. At each value, optimises with that factor Fixed. Other environmental and noise factors keep their settings in the panel, and held factors stay held. Each search starts from the previous solution.
3. For each numeric controlled factor that is not held, fits a polynomial in the environmental factor to the 13 best settings by least squares. The polynomial has degree 2 (constant, linear and squared terms). Terms that change the setting by less than a millionth of its variation over the range are dropped.

The results appear on the Adjustment law tab, which opens when the computation finishes:

| Part | Meaning |
|---|---|
| Formulas | One line per numeric controlled factor that is not held, in the form `Setting = a + b*Env + c*Env^2    (R² = value)`. Spaces and hyphens in names become underscores. |
| Plots | One plot per formula. The points are the best settings (`Optimum`). The dashed line is the fitted law (`Fitted law`). |
| Table | The 13 values of the environmental factor, the desirability reached, and the best setting of each free controlled factor. Categorical factors appear in the table only. |

A factor whose best setting does not change over the range is shown as `Setting = value    (independent of Env)`.

How to read a law:

- To use it, insert the measured value of the environmental factor in the formula and set the controlled factor to the result.
- `R²` is the share of the variation of the best settings that the formula explains. A value near 1 means the formula follows the table closely.
- The desirability column shows how well any setting can do at each condition. If it falls at one end of the range, no adjustment compensates for that condition.
- A law is valid only inside the range of the environmental factor.

Example, project `field-datacentre-cooling-log`, adjustment on Outdoor wet-bulb (IT load Fixed at its default):

```
Chilled_water_setpoint = 14.5648 - 0.0726474*Outdoor_wet_bulb + 0.0014439*Outdoor_wet_bulb^2    (R² = 0.9624)
CRAH_fan_speed = 89.866 - 0.987713*Outdoor_wet_bulb + 0.0358334*Outdoor_wet_bulb^2    (R² = 0.7046)

Outdoor wet-bulb  Desirability  Chilled-water setpoint  CRAH fan speed
0                 1.0000        14.607                  92.265
11                0.8627        13.964                  83.996
22                0.6012        13.619                  84.286
```

The setpoint law fits well. The fan law has a lower R², so the table is the better guide for the fan speed.

The best setting can jump between two distinct regions when the model has more than one good optimum. A polynomial cannot follow a jump. Example, same project, adjustment on IT load (Outdoor wet-bulb Fixed at its default of 11):

```
Chilled_water_setpoint = -8.92461 + 0.0807187*IT_load - 6.70655e-5*IT_load^2    (R² = 0.8039)
CRAH_fan_speed = 146.473 - 0.197588*IT_load + 0.000151916*IT_load^2    (R² = 0.6197)

IT load  Desirability  Chilled-water setpoint  CRAH fan speed
841.67   0.8575        12.65                   85.025
870.83   0.8587        12.305                  85.252
900      0.8587        7                       97.491
```

At 900 kW the best setting jumps to a setpoint of 7 and a fan speed of 97.5, with the same desirability. Check the table and the plot before relying on a law with a low R².

## Next run

The Next run dialog is opened with the button Next run… on the Runs page. It suggests the settings of one more run for today's environmental conditions. The suggestion can be added to the Runs sheet.

| Control | Meaning | Default |
|---|---|---|
| Environmental factor fields | Today's value of each environmental factor, with units. Absent when the project has no environmental factor. | Centre of the factor range. The values are kept until the project changes. |
| Most informative | Suggests the settings that improve the model most. | Selected |
| Best predicted | Suggests the settings with the best predicted outcome. | |
| Suggest | Computes the suggestion for the entered conditions. | |
| Add as planned run | Appends the suggestion to the Runs sheet. Shown after a suggestion exists. | |

The table lists every factor and its suggested value. Environmental factors are shown in muted colour, since they are the conditions that were entered. Factors made inactive by a condition show `—`. Below the table, the program lists the predicted value of every response with a 95% prediction interval. Changing an environmental value clears the suggestion.

### Most informative

The program selects the settings of the designed factors at which a new run would add the most information about the model. The criterion is the prediction variance at the candidate point `x`:

```
v(x) = x' (X'X + 0.001 I)^-1 x
```

`X` is the model matrix of all runs that are not excluded. `x` is the model row of the candidate. The small term `0.001 I` keeps the matrix invertible when there are few runs. The program picks the point with the largest `v(x)`. At that point the current runs give the least information about the response. The program searches 40 random starting points, with coordinate exchange over a grid of factor levels. The environmental factors are held at the entered values.

The model is the model of the first response in the project.

Prediction variance is also a design quality measure: see [designs.md](designs.md). The suggestion usually lies at an edge or corner of the factor region, away from the existing runs.

Example, heat pump project, Outdoor temperature -5, Solar irradiation 0.5:

```
A: Supply temperature   30
B: Fan speed            72
C: Preheat start        1.7
D: Outdoor temperature  -5
E: Solar irradiation    0.5
Electricity: 188.64 (95% PI 169 to 208.3)
Discomfort: 27.633 (95% PI 16.31 to 38.96)
```

The suggestion lies at the edge of the region (Supply temperature 30, the lowest value). The predictions show that the run is for information, not for a good outcome.

### Best predicted

The program runs the desirability search for the entered conditions: every environmental factor is Fixed at the entered value, and the best solution is used. Hold factors, noise factor settings and the goals are those of the Optimize page. Goals must exist, as for Optimize.

Example, same conditions, with Fan speed held at 70 on the Optimize page:

```
A: Supply temperature   46.538
B: Fan speed            70
C: Preheat start        0.5
D: Outdoor temperature  -5
E: Solar irradiation    0.5
Electricity: 237.33 (95% PI 219.5 to 255.2)
Discomfort: 3.0335 (95% PI -7.335 to 13.4)
```

The prediction interval is the interval for one new observation: it covers the model uncertainty at the point plus the residual variation of a single run. It is wider than the confidence interval of the mean. A negative lower limit for a quantity that cannot be negative shows that the model is not precise at that point.

### Added runs

Add as planned run appends a run with status `planned`. The point type is `sequential` for Most informative and `best` for Best predicted. After the run is done, enter the responses on the Runs page. Repeat Suggest with the next set of conditions for sequential experimentation.

## Settings that persist

The settings of the Optimize page (mode, value, mean and SD of environmental and noise factors, and held values) are saved in the project file when Optimize or Compute is pressed. They apply to Optimize, the profiler, the adjustment law and Best predicted. Deleting a factor removes its settings. The page clears the result list, the profiler and the law when another project is opened.
