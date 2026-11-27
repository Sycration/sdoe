# Models

This page describes how a model is specified and selected for a response, how coding and transforms work, and how a fitted model is written out as an equation. The Analysis page holds all model controls. Setting up factors and entering data are covered in [setup-and-data.md](setup-and-data.md), design generation in [designs.md](designs.md), the ANOVA and coefficient tables, diagnostics and plots in [analysis.md](analysis.md), and the use of a model for optimisation in [optimisation.md](optimisation.md).

Each response has its own model: a list of terms and a response transform. The terms are fitted by least squares, and the intercept is estimated unless the model is a mixture model.

## Contents

- [Controls on the Analysis page](#controls-on-the-analysis-page)
- [The default model](#the-default-model)
- [Formula language](#formula-language)
- [Model presets](#model-presets)
- [Hierarchy](#hierarchy)
- [Coded and actual units](#coded-and-actual-units)
- [Response transforms](#response-transforms)
- [Model selection](#model-selection)
- [Automatic model and transform search](#automatic-model-and-transform-search)
- [Equations](#equations)
- [Environmental factors in models](#environmental-factors-in-models)
- [Conditional factors in models](#conditional-factors-in-models)

## Controls on the Analysis page

The row at the top of the Analysis page holds the model controls.

| Control | Meaning |
|---|---|
| Response list | Selects the response. Every other control on the page applies to this response. |
| Formula field | Shows the model formula. The placeholder reads `Model formula, e.g. A + B + A*B + A^2`. Press Enter or click **Apply** to set the formula. |
| **Apply** | Parses the formula and replaces the terms of the model. The transform stays. |
| **Model** menu | Presets, selection and the automatic search (see below). |
| **Hierarchical** | When checked (default), **Apply** adds every missing lower-order term. See [Hierarchy](#hierarchy). |
| Transform list | Sets the response transform: `None`, `Ln`, `Log10`, `Square root`, `Inverse`. |

Items of the **Model** menu:

| Item | Effect |
|---|---|
| **Linear** | Replaces the terms with the linear preset. |
| **Two-factor interactions** | Replaces the terms with the two-factor interaction preset. |
| **Quadratic** | Replaces the terms with the quadratic preset. |
| **Cubic** | Replaces the terms with the cubic preset. |
| **Include environment × factor interactions** | Check box. When checked, the four presets above and the pool of **Find best model and transform** also contain environment × factor interactions. Default: unchecked. |
| **Stepwise selection (AICc)** | Stepwise term selection by AICc. |
| **Backward elimination (p > 0.10)** | Backward elimination by p-value with a threshold of 0.10. |
| **Find best model and transform** | Automatic search over transforms and terms, ranked by predicted R². |

Every change refits the model at once. The fit statistics, tables, equation and plots on the page show the new model. **Edit > Undo** reverts a model change.

In the coefficient and ANOVA tables, the row action **Remove term** deletes one term from the model. A term that is part of a higher-order term in the model (for example `A` when `A*B` is present) cannot be removed until the higher-order term is removed.

## The default model

A response without a stored model uses:

1. The model of the last applied design (see [designs.md](designs.md)), plus a main effect for every environmental and block factor.
2. If no design was applied: the largest of Quadratic, Two-factor interactions and Linear that leaves at least 4 more runs than terms. Linear applies regardless of the number of runs.

## Formula language

A formula is a sum of terms. The intercept is not written: it is part of every model that is not a mixture model.

| Syntax | Meaning | Example | Terms produced |
|---|---|---|---|
| `A + B` | Terms are joined with `+`. | `A + B + C` | `A`, `B`, `C` |
| `A*B` | Product (interaction). `:`, `×` and `·` are accepted in place of `*`. | `A*B` | `A*B` |
| `A*B*C` | Product of three or more factors. | `A*B*C` | `A*B*C` |
| `A^2` | Power of one factor. The exponent is a positive integer up to 10. `A²` and `A³` are accepted. | `A^2` | `A^2` |
| `A*A` | Same as `A^2`: powers of a shared factor add. | `A*A` | `A^2` |
| `(A+B)*C` | Distribution over a group. | `(A+B)*C` | `A*C`, `B*C` |
| `(A+B+C)^2` | All products of up to 2 distinct terms of the group. No squares. | `(A+B+C)^2` | `A`, `B`, `C`, `A*B`, `A*C`, `B*C` |
| `(A+B+C)^3` | All products of up to 3 distinct terms of the group. | `(A+B+C)^3` | the six above and `A*B*C` |
| `- B*C` | Removes the term from the formula, wherever the formula adds it. | `(A+B+C)^2 - B*C` | `A`, `B`, `C`, `A*B`, `A*C` |
| `1` | Intercept. Accepted and ignored. A formula cannot remove the intercept. | `1 + A` | `A` |
| `` `Feed rate` `` or `"Feed rate"` | Factor by quoted name. Required for names with spaces or operators. | `` `Print speed` + C `` | `C`, `D` (in the example below `Print speed` has the symbol `D`) |

Rules:

- A factor is found by symbol (exact, then ignoring case), then by name (exact, then ignoring case). `c + d` and `C + D` are equal.
- `A^2*B` is one term: the square of `A` multiplied by `B`.
- `(A+B)^2` is not the same as `A^2 + B^2`. The group power builds interactions. Write squares explicitly.
- A minus sign removes the terms that follow it up to the next `+` or `-`. The position does not matter: `A*B + C - A` and `- A + A*B + C` both leave `A*B` and `C`.
- Terms are listed by degree: main effects first, then two-factor terms, and so on.
- Parentheses nest at most 64 deep. A formula expands to at most 10000 terms.
- An empty formula and a formula whose terms all cancel are errors: a model must contain at least one term.

### Rules by factor type

| Factor type | Terms allowed |
|---|---|
| Continuous | Powers up to 10 and all products. |
| Discrete | Powers up to the number of levels minus 1. A factor with 4 levels supports `A^3`, not `A^4`. |
| Categorical | Main effect and products only. No powers. A categorical term adds one column per level except the last (see [Coded and actual units](#coded-and-actual-units)). |
| Mixture component | Linear blending terms and products (Scheffé terms). No powers. |

Examples on a project with the factors `A` Layer height (4 levels), `B` Material (categorical), `C` Nozzle temperature offset, `D` Print speed:

```
formula                    terms in the model
A*C                        A*C
(A+C+D)^2                  A + C + D + A*C + A*D + C*D
(A+C+D)^3                  A + C + D + A*C + A*D + C*D + A*C*D
(A + C)*D                  A*D + C*D
`Print speed` + C          C + D
B*C*D - B                  B*C*D
(A+B)^2 - A*B              A + B
A^3                        A^3
B^2                        error: B is categorical and has no powers
A^4                        error: A has 4 levels, which supports powers up to 3
Q                          error: unknown factor 'Q'
```

A mixture model has no intercept. A model has no intercept when any term is a single mixture component (`A`, with `A` a component).

## Model presets

A preset replaces the terms of the model and keeps the transform. Designed factors are the controlled and noise factors that are not mixture components.

| Preset | Designed factors | Mixture components | Environmental and block factors |
|---|---|---|---|
| Linear | Main effects. | Linear blending terms. | Main effects. |
| Two-factor interactions | Main effects and all two-factor products. | Linear terms and all two-component products. | Main effects. |
| Quadratic | As two-factor interactions, plus the square of each factor that supports powers (continuous, and discrete with at least 3 levels). | As two-factor interactions. | Main effects. |
| Cubic | As quadratic, plus all three-factor products, cubes, and the products `A^2*B` of a square with another factor (where the factor supports the power). | Linear terms, products of two and of three components. | Main effects. |

Notes:

- Categorical factors have main effects and products, never powers.
- Products of a mixture component with a process factor are not in any preset. Write them in the formula, for example `A*D`.
- With **Include environment × factor interactions** checked, Two-factor interactions, Quadratic and Cubic also contain every product of an environmental factor with a designed factor. Quadratic and Cubic add the squares of environmental factors. See [Environmental factors in models](#environmental-factors-in-models).
- Block factors enter as main effects only.
- Where a conditional factor applies only for a single value of a parent factor, the preset leaves out the product of the two (see [Conditional factors in models](#conditional-factors-in-models)).

Example: response `Ester yield` of the esterification example. Factors `A` Temperature, `B` Reaction time and `C` Catalyst loading are controlled. Factor `D` Feedstock water is environmental.

```
preset                              environment interactions   formula
Linear                              either                     A + B + C + D
Two-factor interactions             unchecked                  A + B + C + D + A*B + A*C + B*C
Two-factor interactions             checked                    A + B + C + D + A*B + A*C + A*D + B*C + B*D + C*D
Quadratic                           unchecked                  A + B + C + D + A*B + A*C + B*C + A^2 + B^2 + C^2
Quadratic                           checked                    A + B + C + D + A*B + A*C + A*D + B*C + B*D + C*D + A^2 + B^2 + C^2 + D^2
Cubic                               unchecked                  A + B + C + D + A*B + A*C + B*C + A^2 + B^2 + C^2
                                                               + A*B*C + A*B^2 + A^2*B + A*C^2 + A^2*C + B*C^2 + B^2*C
                                                               + A^3 + B^3 + C^3
```

The cubic formula is one line in the application. A full cubic model has many terms. A fit with more coefficients than runs leaves some columns aliased (see [analysis.md](analysis.md)).

## Hierarchy

A model is hierarchical when every lower-order term that makes up a higher-order term is also in the model. `A*B` requires `A` and `B`. `A^2` requires `A`. `A^2*B` requires `A*B`, `A^2`, `A` and `B`.

| Where | Behaviour |
|---|---|
| **Hierarchical** check box | When checked, **Apply** adds the missing lower-order terms to the parsed formula. When unchecked, the formula is used as typed. The check box has no effect on presets, selection or the automatic search. |
| Presets | The presets are hierarchical. |
| Stepwise, backward elimination, automatic search | These methods keep the model hierarchical: a term is added together with its missing lower-order terms, and a term is not removed while a higher-order term in the model contains it. |
| **Remove term** | Refuses to remove a term that is part of a higher-order term in the model. |

Example. With the factors `A`, `B` and `C` of the previous section:

```
formula        Hierarchical   terms in the model
A*C            unchecked      A*C
A*C            checked        A + C + A*C
C^2*D          checked        C + D + C*D + C^2 + C^2*D
```

A model without the main effects of an interaction has coefficients that depend on the position of the origin of the coded scale. Use it only when a physical reason excludes the main effects.

## Coded and actual units

The program fits every model in coded units. The coefficient table on the Analysis page is in coded units. Equations can be written in either units (see [Equations](#equations)).

### How each factor type is coded

| Factor type | Coded value | Range of the coded value |
|---|---|---|
| Continuous | `(x - centre) / half-range`. Centre = (low + high) / 2. Half-range = (high - low) / 2. | Low is -1, high is +1, centre is 0. |
| Discrete | The same formula, with the lowest and highest level in place of low and high. | Lowest level -1, highest +1. Levels in between take fractional values. |
| Mixture component | Pseudo-component: `(x - low) / (total - sum of all lows)`. The total is the mixture total of the project. | 0 to 1. The coded values of all components add to 1. |
| Categorical | Effect (sum-to-zero) coding, one column per level except the last. | See below. |

Environmental, noise and block factors are coded by their type in the same way.

Example for a discrete factor `Layer height` with levels 0.1, 0.15, 0.2 and 0.3. Centre = 0.2, half-range = 0.1.

```
actual   0.1   0.15   0.2   0.3
coded    -1    -0.5    0     1
```

Example for a mixture: components `A`, `B`, `C` with lows 72, 2 and 1, and a mixture total of 85. The sum of the lows is 75, so the span is 10. `A` = 77 gives the coded value 0.5.

### Categorical contrasts

A categorical factor with `k` levels has `k - 1` columns. The column of level `j` is 1 for level `j`, -1 for the last level and 0 for every other level. The columns are labelled `symbol[level]`.

For `Material` with levels PLA, PETG and ABS:

```
level   B[PLA]   B[PETG]
PLA       1        0
PETG      0        1
ABS      -1       -1
```

The coefficient of `B[PLA]` is the difference between PLA and the average of the three levels. The last level has no coefficient of its own. Its effect is minus the sum of the others. In a product term such as `B*C` the columns of `B` multiply the coded values of `C`, so the term has `k - 1` columns too.

Example. Response `Tensile strength` with the model `A + B + C + B*C` (the fused deposition example), coded units:

```
Tensile strength = 44.3523 - 3.52632 * A + 7.41143 * B[PLA] + 0.758736 * B[PETG] + 4.17846 * C + 0.824123 * B[PLA] * C - 0.0768396 * B[PETG] * C
```

For ABS the contrast of the main effect is -(7.41143 + 0.758736) = -8.17017.

### Coded to actual units

The program substitutes the coded value `(x - centre) / half-range` of each factor and expands the polynomial. The equation in actual units is equivalent to the coded one. The same model gives one equation per combination of categorical levels. The example above in actual units:

```
Material = PLA:
Tensile strength = 58.8164 - 35.2632 * [Layer height] + 0.333505 * [Nozzle temperature offset]

Material = PETG:
Tensile strength = 52.1637 - 35.2632 * [Layer height] + 0.273441 * [Nozzle temperature offset]

Material = ABS:
Tensile strength = 43.2348 - 35.2632 * [Layer height] + 0.228745 * [Nozzle temperature offset]
```

Check for ABS: the intercept in coded units is 44.3523 - 8.17017 = 36.18213. The term -3.52632 * A with A = (x - 0.2) / 0.1 becomes -35.2632 * x + 7.05264. The sum 36.18213 + 7.05264 is 43.2348.

## Response transforms

A transform is applied to the response before the model is fitted. The transform list of the Analysis page offers the following.

| Transform | Fitted quantity | Back-transform of a prediction | Valid response values |
|---|---|---|---|
| `None` | y | z | all |
| `Ln` | ln(y) | exp(z) | y > 0 |
| `Log10` | log10(y) | 10^z | y > 0 |
| `Square root` | sqrt(y) | z^2 (0 if z < 0) | y >= 0 |
| `Inverse` | 1/y | 1/z | y not 0 |
| `Power <λ>` | y^λ (ln(y) for λ = 0) | z^(1/λ) | y > 0; for an integer λ, y not 0; for λ > 0, y = 0 is allowed |

`Power <λ>` is a plain power, not the scaled Box-Cox form. The list offers `Power -0.5`, `Power 2`, `Power -2`, `Power 3` and `Power -3`: together with `None`, `Square root`, `Ln` and `Inverse`, these are all the powers that the Box-Cox plot recommends. Other powers can be set through the engine API (see [../API.md](../API.md)) or read from a project file. The list shows such a power as the current transform.

A run whose response value is outside the valid range of the transform is left out of the fit. The warning `N runs have values the transform cannot handle (e.g. ln of 0)` names the number of runs.

### Which transform to use

A transform is useful when the residuals grow with the fitted values (a funnel in the residual plot), when the response spans more than a factor of 10 between its smallest and largest value, or when the response is a rate or a time. The Box-Cox plot on the Diagnostics tab (see [analysis.md](analysis.md)) shows the effect of the transform power λ on the residual sum of squares.

How to read the plot for the choice of a transform:

- λ is the power of the transform. λ = 1 is `None`, λ = 0.5 is `Square root`, λ = 0 is `Ln` and λ = -1 is `Inverse`.
- The plot shows the best λ and a 95% confidence interval for λ.
- When the interval contains 1, the data give no reason for a transform.
- When 1 is outside the interval, the plot recommends the standard λ in the interval: the one closest to the best λ among 1, 0.5, 0, -0.5, -1, 2, -2, 3 and -3. `1` is preferred when it is in the interval.
- The Box-Cox profile is calculated for the current model terms and is available only when all response values are positive.

Example: response `Spindle power` of the face milling example. The note on the plot reads `Best λ = 0, 95% CI 0 to 0.05. Recommended: Ln.` The stored model uses `Ln`. For `Ester yield` in the esterification example the note reads `Best λ = 3, 95% CI -3 to 3. Recommended: None.` The interval is as wide as the search range. No transform is favoured.

### Predictions and statistics under a transform

- The coefficients, standard errors, ANOVA, mean, C.V., R², adjusted R², predicted R², PRESS, AICc and BIC are on the transformed scale.
- A prediction is back-transformed: the predicted value is the inverse transform of the fitted value. No bias correction is applied. For `Ln` the back-transformed prediction is the median, not the mean, of the response.
- The 95% confidence and prediction intervals are calculated on the transformed scale. The program back-transforms the end points. For `Inverse` and negative powers, an interval that contains 0 on the transformed scale gives an unbounded interval on the original scale.
- The Predicted vs actual plot and the model search table use the original scale.
- With a transform, the statistics block shows both **Predicted R²** (transformed scale) and **Predicted R² (original scale)**. The second is the value that ranks the candidates of the model search. Example: `Ester yield` with `Inverse` and the model `A + B + C + A*B + A^2 + B^2` gives 0.7768 on the transformed scale and 0.7920 on the original scale.

Example. The model `A + B` for `Ester yield` under each transform, in coded units, as plain text:

```
Ln            ln(Ester yield) = 4.49947 + 0.0250163 * A + 0.0127009 * B
              Ester yield = exp(4.49947 + 0.0250163 * A + 0.0127009 * B)
Log10         log10(Ester yield) = 1.95409 + 0.0108644 * A + 0.00551594 * B
              Ester yield = 10^(1.95409 + 0.0108644 * A + 0.00551594 * B)
Square root   sqrt(Ester yield) = 9.48659 + 0.117315 * A + 0.0592629 * B
              Ester yield = (9.48659 + 0.117315 * A + 0.0592629 * B)^2
Power 0.5     Ester yield^0.5 = 9.48659 + 0.117315 * A + 0.0592629 * B
              Ester yield = (9.48659 + 0.117315 * A + 0.0592629 * B)^(1/0.5)
```

The prediction at the centre of the design is 89.9692 for `Ln` with a 95% prediction interval of 84.899 to 95.342.

## Model selection

Selection removes terms that do not improve the model, or adds terms that do. Two methods are available from the **Model** menu. Both work within the terms of the current formula. To search a larger set of terms, apply a larger preset first (for example **Quadratic**) and then run the selection. Both keep the model hierarchical, keep every block factor main effect and keep the linear terms of mixture components. A selection that removes every term is an error.

The program writes a log of the steps under the tables on the Analysis page. The log disappears when a formula is applied.

### Stepwise selection (AICc)

Stepwise selection starts from the main effects (linear terms) that are in the current formula. Each step evaluates every term that can be added or removed:

- An absent term is added together with those of its lower-order terms that are absent. The log then reads, for example, `Add A^2*B with A, B, A*B, A^2`. A term can therefore enter after its main effect was removed.
- A present term can be removed when no higher-order term in the model contains it.

The step that lowers AICc the most is taken. The selection stops when no step lowers AICc. A step must lower AICc by more than 1e-9, and the number of steps is limited to 500. Terms that were added can be removed later and removed terms can be added again.

The log shows the AICc of the start model and the AICc after each step.

Example: response `Surface roughness Ra` of the face milling example, after **Model > Quadratic** (15 terms, factor `E` Flank wear is environmental):

```
Start (criterion -99.871)
Add A^2 (-112.64)
Add A*B (-127.1)
Remove D (-131.52)
Add B*C (-134.22)
Add B^2 (-136.7)
```

The model after selection is `A + B + C + E + A*B + B*C + A^2 + B^2`. The statistics block reports R² 0.9763, adjusted R² 0.9664, predicted R² 0.9388 and AICc -136.70.

#### AICc

AICc is Akaike's information criterion with a correction for small samples:

```
AICc = n * ln(SSE / n) + 2k + 2k(k + 1) / (n - k - 1)
```

`n` is the number of runs, `SSE` the residual sum of squares and `k` the number of estimated coefficients plus 1 (for the error variance). The criterion is infinite when `n - k - 1` is not positive.

- Lower is better.
- The first term measures the fit, the others penalise the number of coefficients. The correction is large when `k` is close to `n`.
- The absolute value has no meaning. Compare AICc values only for the same response, the same runs and the same transform.
- An SSE below 1e-24 times the sum of squares of the response is treated as rounding noise of a perfect fit. Such fits tie, and the penalty term determines the order.

BIC, shown in the statistics block, uses the penalty `k * ln(n)` in place of the AICc penalty and favours smaller models.

### Backward elimination (p > 0.10)

Backward elimination starts from every term of the current formula. Each step performs a partial F-test for each term that can be removed (terms that are not part of a higher-order term in the model). The term with the largest p-value is removed when its p-value is above the threshold. The elimination stops when every remaining term has a p-value at or below the threshold.

| Option | Meaning | Default |
|---|---|---|
| Threshold (alpha) | A term is removed when its p-value is above this value. | 0.10 |

The **Backward elimination (p > 0.10)** menu item uses the threshold 0.10. The engine accepts other thresholds (see [../API.md](../API.md)).

The log shows each removed term with its p-value. If the model has no residual degrees of freedom, p-values do not exist. The program then removes the term whose removal increases the residual sum of squares the least, until a residual remains. The log shows an empty value in the parentheses of those steps.

Example, same response and starting point as above:

```
Remove A*D (0.86397)
Remove C^2 (0.83547)
Remove A*C (0.723)
Remove D^2 (0.17209)
```

The model after elimination is `A + B + C + D + E + A*B + B*C + B*D + C*D + A^2 + B^2`. The statistics block reports R² 0.9876, adjusted R² 0.9790, predicted R² 0.9474 and AICc -135.65. The same data give a smaller model with stepwise selection (8 terms, AICc -136.70) than with backward elimination (11 terms, AICc -135.65).

### Choosing between the methods

| Method | Criterion | Tends to give |
|---|---|---|
| Stepwise (AICc) | Information criterion. Adds and removes terms. | Smaller models that predict well. |
| Backward elimination | p-value of a partial F-test. Removes terms only. | Models with terms that are significant at the chosen level. |

Neither method replaces a check of the diagnostics. A model chosen from many candidate terms fits the data better than it will fit new runs. The predicted R² (see below) is the measure to watch.

### Predicted R² and PRESS

PRESS (prediction error sum of squares) measures how well the model predicts a run that it was not fitted to:

```
PRESS = sum over runs of ( e_i / (1 - h_i) )^2
Predicted R² = 1 - PRESS / SS_total
```

`e_i` is the residual of run `i` and `h_i` its leverage. The ratio `e_i / (1 - h_i)` is the error of the prediction for run `i` when the model is refitted without run `i`. `SS_total` is the total sum of squares about the mean of the response.

- Predicted R² is at most 1 and can be negative. A negative value means that the model predicts new runs worse than the mean of the response does.
- Adjusted R² counts the terms. Predicted R² tests the model against single runs.
- A gap of more than 0.2 between adjusted and predicted R² produces a warning on the Analysis page. The usual causes are too many terms, a single influential run, or a response that requires a transform.
- Predicted R² is not defined (shown empty) when a run has leverage 1.

## Automatic model and transform search

**Model > Find best model and transform** searches transforms and terms and sets the best candidate as the model of the response, with its transform. The previous model and transform are replaced. **Edit > Undo** restores them.

The search:

1. Chooses the candidate transforms from the response values: `None` always, and `Ln`, `Square root` and `Inverse` when every value is above 0. When the smallest value is exactly 0, only `None` and `Square root`. `Log10` and `Power` are not searched.
2. Chooses the pool of terms: the Quadratic preset, or the Cubic preset when the number of runs is at least twice the number of cubic terms plus 4. With **Include environment × factor interactions** checked, the pool also contains the environment × factor interactions of the preset.
3. For each transform, runs stepwise AICc selection from the linear terms over the pool. The current model is not used.
4. Fits each result and calculates its predicted R² on the original scale.
5. Sorts the candidates by that value, highest first. The first candidate is set as the model.

The predicted R² on the original scale is calculated from the leave-one-out predictions of the PRESS calculation. Each leave-one-out prediction is back-transformed to the original scale. The ranking value is `1 - PRESS_original / SS_total_original`, where `PRESS_original` sums the squared differences between the measured responses and the back-transformed leave-one-out predictions. This allows models under different transforms to be compared on the scale in which the response is measured. A candidate with an undefined value is placed last.

### The model search table

The table appears under the equation on the Analysis page with the title **Model search**. The first row, the chosen candidate, is highlighted.

| Column | Meaning |
|---|---|
| Transform | The response transform of the candidate. |
| Terms | The number of terms in the model, without the intercept. |
| Predicted R² (original scale) | The ranking value. Higher is better. |
| Adjusted R² | Adjusted R² on the transformed scale. |
| AICc | AICc on the transformed scale. |

The rows are sorted by `Predicted R² (original scale)`. Adjusted R² and AICc are calculated on the transformed response, so values in different rows are not comparable. The AICc of a model under `Inverse` can be hundreds of units below that of the model under `None` without the first model being better.

Example, response `Spindle power` of the face milling example, with **Include environment × factor interactions** checked (factor `E` Flank wear is environmental):

```
Transform     Terms   Predicted R² (original scale)   Adjusted R²   AICc
Ln             11     0.9982                          0.9995        -204.08
Square root    13     0.9964                          0.9980        -193.45
None           13     0.9576                          0.9933        -110.83
Inverse         9     -3.5890                         0.9234         -30.97
```

The program sets `A + B + C + D + E + B*D + B*E + A^2 + B^2 + C^2 + D^2` with `Ln`. The predicted R² under `Inverse` is negative: the back-transformed leave-one-out predictions are worse than the mean, while adjusted R² on the transformed scale is 0.92.

Example, response `Surface roughness Ra`, same project and setting:

```
Transform     Terms   Predicted R² (original scale)   Adjusted R²   AICc
Square root     8     0.9820                          0.9862        -194.96
Ln              9     0.9816                          0.9849        -141.78
None            8     0.9800                          0.9866        -162.53
Inverse         6     0.9558                          0.9698        -110.76
```

The first three candidates are close. The difference of 0.002 between `Square root` and `None` is small. Check the plots of the leading candidates (see [analysis.md](analysis.md)) before accepting a transform.

The search ranks a fixed set of candidates. The result depends on the pool of terms and on the greedy path of the selection. Run **Stepwise selection (AICc)** or edit the formula afterwards to adjust the chosen model.

## Equations

The **Equation** section of the Analysis page shows the fitted model as an equation. It has three controls.

| Control | Options | Default |
|---|---|---|
| Units list | `Actual units`, `Coded units` | `Actual units` |
| Syntax list | `Plain text`, `LaTeX`, `Excel`, `Python`, `C / C++ / Rust` | `Plain text` |
| **Copy** | Copies the equation text to the clipboard. | |

Rules for all syntaxes:

- Coefficients have 6 significant digits.
- Aliased terms (not estimable with the runs available) are left out.
- In actual units, a coefficient smaller than 1e-13 times the largest coefficient in its equation is dropped.
- With a transform, `Plain text` and `LaTeX` give two lines: the equation on the transformed scale and the equation solved for the response. `Excel`, `Python` and `C / C++ / Rust` give the solved equation only.
- Where the model has categorical factors, the actual-unit output has one equation per combination of levels, each labelled with the levels. At most 64 equations are written. A larger number gives the message `Too many categorical level combinations for actual-unit equations; use coded units.`
- Coded units give one equation. The categorical columns appear as `B[PLA]`.

### Variable names

| Syntax | Coded units | Actual units | Response |
|---|---|---|---|
| Plain text | Factor symbol (`A`). Mixture components use pseudo-component values. Categorical columns as `B[PLA]`. | Factor name. A name with characters other than letters, digits and `_` is written in brackets: `[Reaction time]`. | Response name. |
| LaTeX | Symbol. Categorical columns as `B_{[\text{PLA}]}`. | `\text{Factor name}`. | `\text{Response name}`. |
| Excel | Symbol. | Factor name with other characters replaced by `_`: `Reaction_time`. | Not used. |
| Python, C | Symbol. | Factor name as in Excel. | Name as in Excel: `Ester_yield`. |

### Operators by syntax

| Item | Plain text | LaTeX | Excel | Python | C / C++ / Rust |
|---|---|---|---|---|---|
| Multiplication | ` * ` | ` \cdot ` | `*` | `*` | `*` |
| Power | `A^2` | `A^{2}` | `A^2` | `A**2` | `A*A` |
| Start of line | `Name =` | `Name =` | `=` | `Name =` | `Name =` |
| End of line | none | none | none | none | `;` |
| Square root back-transform | `(z)^2` | `\left(z\right)^{2}` | `(z)^2` | `(z)**2` | `(z)*(z)` |
| Ln back-transform | `exp(z)` | `e^{z}` | `EXP(z)` | `math.exp(z)` | `exp(z)` |
| Log10 back-transform | `10^(z)` | `10^{z}` | `10^(z)` | `10**(z)` | `pow(10.0, z)` |
| Inverse back-transform | `1/(z)` | `\frac{1}{z}` | `1/(z)` | `1/(z)` | `1/(z)` |

The `Python` output uses `math.exp` for `Ln` and then starts with the line `import math`. In `Python`, `C / C++ / Rust` and `LaTeX`, the line that names the categorical levels of an equation is a comment (`#`, `//`, `%`). The `C / C++ / Rust` output uses `exp` and `pow` of the C maths library. The `Excel` output is a formula that starts with `=`. The variable names in the formula must be defined as names in the workbook or replaced by cell references.

### Examples

All examples use `Ester yield` from the esterification example with the formula `A + B + A*B + A^2`. In this example `A` is Temperature (80 to 100 °C, centre 90, half-range 10) and `B` is Reaction time (2 to 6 h, centre 4, half-range 2).

#### Coded units

Plain text:

```
Ester yield = 91.1444 + 2.20102 * A + 1.10625 * B - 1.65 * A * B - 1.54895 * A^2
```

LaTeX:

```
\text{Ester yield} = 91.1444 + 2.20102 \cdot A + 1.10625 \cdot B - 1.65 \cdot A \cdot B - 1.54895 \cdot A^{2}
```

Excel:

```
=91.1444 + 2.20102*A + 1.10625*B - 1.65*A*B - 1.54895*A^2
```

Python:

```
Ester_yield = 91.1444 + 2.20102*A + 1.10625*B - 1.65*A*B - 1.54895*A**2
```

C / C++ / Rust:

```
Ester_yield = 91.1444 + 2.20102*A + 1.10625*B - 1.65*A*B - 1.54895*A*A;
```

#### Actual units

Plain text:

```
Ester yield = -86.0418 + 3.3382 * Temperature + 7.97813 * [Reaction time] - 0.0825 * Temperature * [Reaction time] - 0.0154895 * Temperature^2
```

LaTeX:

```
\text{Ester yield} = -86.0418 + 3.3382 \cdot \text{Temperature} + 7.97813 \cdot \text{Reaction time} - 0.0825 \cdot \text{Temperature} \cdot \text{Reaction time} - 0.0154895 \cdot \text{Temperature}^{2}
```

Excel:

```
=-86.0418 + 3.3382*Temperature + 7.97813*Reaction_time - 0.0825*Temperature*Reaction_time - 0.0154895*Temperature^2
```

Python:

```
Ester_yield = -86.0418 + 3.3382*Temperature + 7.97813*Reaction_time - 0.0825*Temperature*Reaction_time - 0.0154895*Temperature**2
```

C / C++ / Rust:

```
Ester_yield = -86.0418 + 3.3382*Temperature + 7.97813*Reaction_time - 0.0825*Temperature*Reaction_time - 0.0154895*Temperature*Temperature;
```

Check of the actual-unit coefficient of `Temperature^2`: -1.54895 / 10^2 = -0.0154895.

#### With a transform

The model `A + B + C + A*B + A^2 + B^2` of the same response under `Inverse` (the `Inverse` candidate of the automatic search), plain text, coded units:

```
1/Ester yield = 0.0109127 - 0.000284544 * A - 0.000145906 * B - 0.000134182 * C + 0.000217957 * A * B + 0.000207873 * A^2 + 8.09736e-5 * B^2
Ester yield = 1/(0.0109127 - 0.000284544 * A - 0.000145906 * B - 0.000134182 * C + 0.000217957 * A * B + 0.000207873 * A^2 + 8.09736e-5 * B^2)
```

In `LaTeX`, a coefficient written in exponent form appears as `1.08978 \times 10^{-5}`.

#### Mixture model

For the tablet formulation example (mixture components `A`, `B`, `C` with a total of 85, factor `D` Compression force), response `Hardness`, formula `A + B + C + A*B + D`. The coded equation has no intercept:

```
Hardness = 82.198 * A + 151.931 * B + 58.3751 * C + 27.2799 * D + 38.4322 * A * B
```

`A`, `B` and `C` are pseudo-component values between 0 and 1. In actual units the equation has an intercept, because the components add to a constant total:

```
Hardness = -636.36 + 7.45116 * [Microcrystalline cellulose] - 12.4782 * [Povidone K30] + 5.83751 * [Croscarmellose sodium] + 4.54665 * [Compression force] + 0.384322 * [Microcrystalline cellulose] * [Povidone K30]
```

## Environmental factors in models

An environmental factor is measured at every run and cannot be set. In a model it acts as a covariate: it removes the variation that it causes from the residual and shows whether the best settings of the controlled factors depend on it. Setting the role of a factor is described in [setup-and-data.md](setup-and-data.md). The use of environmental values in optimisation is described in [optimisation.md](optimisation.md).

What the program does with an environmental factor in the model:

- The factor is coded by its type like any other factor (see [Coded and actual units](#coded-and-actual-units)).
- Presets and the default model contain its main effect.
- A run without a value for an environmental factor in the model is left out of the fit. The warning `N runs lack values for model factors and are left out (enter the measured environmental values)` names the number of runs.
- Products of an environmental factor with designed factors, and squares of the environmental factor, are in the model only when the formula contains them or when a preset is applied with **Include environment × factor interactions** checked.
- The check box applies to the presets and to the automatic search. Stepwise and backward selection search the terms of the current formula, so the interactions must be in the formula first.
- Selection can remove the main effect of an environmental factor when its removal lowers the criterion.

When the interactions of the environmental factors with the designed main effects are jointly significant (partial F-test, p < 0.05), the Analysis page shows a warning of the form `Environment × factor interactions are significant (p = ...): the best settings depend on ...`.

Example: response `Electricity` of the heat pump example. Factors `A` Supply temperature, `B` Fan speed and `C` Preheat start are controlled. Factors `D` Outdoor temperature and `E` Solar irradiation are environmental.

```
Quadratic, unchecked:  A + B + C + D + E + A*B + A*C + B*C + A^2 + B^2 + C^2
                       R² 0.9775   adjusted R² 0.9680   predicted R² 0.9499   AICc 199.36
                       Warning: Environment × factor interactions are significant (p = 0.0006): ...

Quadratic, checked:    A + B + C + D + E + A*B + A*C + A*D + A*E + B*C + B*D + B*E + C*D + C*E
                       + A^2 + B^2 + C^2 + D^2 + E^2
                       R² 0.9927   adjusted R² 0.9849   predicted R² 0.9624   AICc 215.28
                       Warning: E has VIF 11.8 ...
```

The checked model has 19 terms for 38 runs, and its AICc is higher. Stepwise selection (AICc) on it gives:

```
Start (criterion 185.56)
Add A*D (164.69)
Add B^2 (158.28)
Add B*E (157.03)
```

The result is `A + B + C + D + E + A*D + B*E + B^2`: the controlled factor `A` interacts with the outdoor temperature `D`, and `B` interacts with the solar irradiation `E`. The AICc falls from 199.36 for the quadratic model without interactions to 157.03.

A categorical environmental factor, such as a lot, has one coefficient per level except the last. A factor with many levels uses many degrees of freedom and, in products, many columns. Check the predicted R² after adding it.

## Conditional factors in models

A conditional factor applies only when a parent factor meets a condition (see [setup-and-data.md](setup-and-data.md)). In a formula it is written like any other factor. The model matrix handles it as follows:

- Every column that contains the factor is 0 where the factor does not apply: the coded centre for a numeric factor, no contrast for a categorical factor. The effects of the factor are estimated from the runs where it applies.
- A run where the factor does not apply has no value for it.
- Presets leave out the product of a conditional factor with a parent (or an ancestor) that takes a single value wherever the factor applies. The column would repeat the main effect of the conditional factor.
- Equations in actual units have one equation per region, labelled with the condition.
- The coded equation begins with a note that terms with the factor are 0 where the factor does not apply. The note is a comment in `Python` (`#`), `C / C++ / Rust` (`//`) and `LaTeX` (`%`) output, plain text in `Plain text` output, and left out of `Excel` output.

Example: a project with the continuous factor `T` Temperature (-20 to 20) and the categorical factor `D` Defrosting (Air, Water) with the condition `T < 0`. The response is `Texture`. The preset **Two-factor interactions** gives `T + D + T*D`. Coded units:

```
Terms with D are 0 where the factor does not apply.
Texture = 10.2037 + 3.44524 * T - 0.637185 * D[Air] + 0.155484 * T * D[Air]
```

Actual units:

```
Temperature >= 0:
Texture = 10.2037 + 0.172262 * Temperature

Defrosting = Air, Temperature < 0:
Texture = 9.56654 + 0.180036 * Temperature

Defrosting = Water, Temperature < 0:
Texture = 10.8409 + 0.164488 * Temperature
```

For `T >= 0` the equation has no defrosting term. For `T < 0` there is one equation per level of `D`.
