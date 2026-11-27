# Analysis page

On the Analysis page, the program fits a model to one response and displays the fit, the statistics for assessing it, diagnostic plots and prediction plots. This document explains every number and plot on the page.

Related documents:

- Model formula syntax, presets, transforms, stepwise selection and the model search: [models.md](models.md).
- Desirability and numerical optimisation of the fitted models: [optimisation.md](optimisation.md).
- Factors, responses, roles, coding and run entry: [setup-and-data.md](setup-and-data.md).
- Design types and their properties: [designs.md](designs.md).
- Developers: [../API.md](../API.md).

## Contents

1. [Page layout](#page-layout)
2. [Worked example](#worked-example)
3. [Fit statistics](#fit-statistics)
4. [ANOVA table](#anova-table)
5. [Coefficient table](#coefficient-table)
6. [Warnings and highlighting](#warnings-and-highlighting)
7. [Canonical analysis](#canonical-analysis)
8. [Equation](#equation)
9. [Diagnostics tab](#diagnostics-tab)
10. [Box-Cox plot](#box-cox-plot)
11. [Effects tab: screening](#effects-tab-screening)
12. [Profiler tab](#profiler-tab)
13. [Surface tab](#surface-tab)
14. [Interaction tab](#interaction-tab)
15. [Data tab](#data-tab)
16. [Robustness tab: signal-to-noise](#robustness-tab-signal-to-noise)
17. [Environment x factor interaction note](#environment-x-factor-interaction-note)
18. [Quick reference of thresholds](#quick-reference-of-thresholds)

## Page layout

The top row selects the response, shows the model formula and holds the model controls (**Apply**, **Model**, **Hierarchical**, and the response transform box). These are described in [models.md](models.md). Everything below the top row refers to the response and model selected there.

The left pane, from top to bottom:

1. Warnings, each prefixed with a warning sign.
2. Fit statistics (a grid of name and value pairs).
3. The canonical analysis lines, when the model has quadratic terms.
4. **Coefficients (coded units)**: the coefficient table.
5. **Analysis of variance**: the ANOVA table.
6. **Equation**: the fitted equation in actual or coded units.
7. Model search output, when a search or stepwise selection was run.

The right pane has seven tabs: **Diagnostics**, **Effects**, **Profiler**, **Surface**, **Interaction**, **Data** and **Robustness**.

Every table has a context menu (right click). **Copy table** copies the table as tab-separated text. In the coefficient and ANOVA tables, **Remove term** removes the term of the clicked row from the model.

Every plot shows a tooltip with the run number and the coordinates of the data point nearest to the pointer.

### Data used in the fit

The fit uses the runs that satisfy all of the following:

- The run is not excluded.
- The run has a finite value for the response.
- The run has a value for every factor in the model (a measured environmental factor must be entered).
- The response can be transformed (for example, `Ln` is defined only for positive values).

Each skipped group of runs produces a warning that gives the count.

### Coding

The coefficient table and the canonical analysis are in coded units.

| Factor kind | Coding |
|---|---|
| Continuous, discrete | Low end of the range is -1, high end is +1, the centre is 0. |
| Categorical | Sum-to-zero (effect) coding: a factor with k levels has k-1 columns, labelled `C[X]` for level X. The last level is implied. |
| Mixture component | Pseudo-component coding from 0 to 1. A model with a linear mixture term has no intercept. See [models.md](models.md). |

Coded units make coefficients of different factors comparable and make the variance inflation factors and the effect sizes independent of the factor units. The **Equation** section converts the model to actual units.

## Worked example

The example project `reactor` (from the example menu in the application) has five factors and two responses.

| Symbol | Factor | Role | Range |
|---|---|---|---|
| A | Temperature | Controlled | 150 to 200 °C |
| B | Time | Controlled | 10 to 30 min |
| C | Catalyst | Controlled | X, Y |
| D | Stirring | Controlled | 200, 400, 600 rpm |
| E | Humidity | Environmental | 30 to 70 %RH |

The response `Yield` (%) has 26 runs. The stored model is a quadratic with environment interactions, reduced by stepwise AICc selection:

```
A + B + C + D + E + A*B + A*E + D*E + A^2 + B^2 + D^2
```

Fit statistics:

```
Runs used         26
Std. deviation    0.57218
Mean              79.685
C.V. %            0.72
R²                0.9939
Adjusted R²       0.9891
Predicted R²      0.9711
Adeq. precision   50.619
PRESS             21.681
AICc              11.21
BIC               -2.77
```

ANOVA table:

```
Source             Sum of squares  df  Mean square  F        p
Model              746.397         11  67.8543      207.255  < 0.0001
A                  215.605         1   215.605      658.546  < 0.0001
B                  162.16          1   162.16       495.306  < 0.0001
C                  73.5977         1   73.5977      224.798  < 0.0001
D                  15.2261         1   15.2261      46.507   < 0.0001
E                  53.7428         1   53.7428      164.153  < 0.0001
A*B                52.6965         1   52.6965      160.957  < 0.0001
A*E                8.09833         1   8.09833      24.736   0.0002
D*E                1.84435         1   1.84435      5.633    0.0325
A^2                44.8967         1   44.8967      137.133  < 0.0001
B^2                22.6601         1   22.6601      69.213   < 0.0001
D^2                3.09557         1   3.09557      9.455    0.0082
Residual           4.58352         14  0.327395
Total (corrected)  750.981         25
```

Coefficient table (coded units):

```
Term       Coefficient  Std. error  t         p         95% CI low  95% CI high  VIF
Intercept  86.6651      0.4919      176.177   < 0.0001  85.61       87.72
A          3.67615      0.1433      25.662    < 0.0001  3.3689      3.9834       1.32
B          2.82465      0.1269      22.255    < 0.0001  2.5524      3.0969       1.03
C[X]       1.88688      0.1258      14.993    < 0.0001  1.617       2.1568       1.25
D          1.02274      0.15        6.820     < 0.0001  0.70108     1.3444       1.44
E          -3.35409     0.2618      -12.812   < 0.0001  -3.9156     -2.7926      1.37
A*B        1.82689      0.144       12.687    < 0.0001  1.518       2.1357       1.08
A*E        -1.6256      0.3269      -4.973    0.0002    -2.3266     -0.92457     1.92
D*E        0.716564     0.3019      2.373     0.0325    0.069044    1.3641       1.71
A^2        -3.73274     0.3188      -11.710   < 0.0001  -4.4164     -3.0491      1.23
B^2        -2.89284     0.3477      -8.319    < 0.0001  -3.6386     -2.1471      1.49
D^2        -0.957348    0.3113      -3.075    0.0082    -1.6251     -0.28959     1.20
```

Canonical analysis lines:

```
Stationary point (maximum, inside the factor ranges): Temperature = 191.58, Time = 26.976, Stirring = 600
Predicted response there: 91.029
Eigenvalues: -0.9573, -2.307, -4.318
```

Reading the output:

- R² and adjusted R² are close to 1 and predicted R² is within 0.03 of adjusted R². The model explains the data and predicts left-out runs well.
- Adequate precision is far above 4. The fitted values span many standard deviations of noise.
- All p-values are below 0.05, so every term has bold emphasis. D*E (p = 0.0325) is the weakest.
- All VIF values are below 2. The estimates are nearly independent.
- The humidity terms E, A*E and D*E are significant. The optimum temperature depends on the humidity measured on the day.
- All three eigenvalues are negative, so the stationary point is a maximum.

### Removing the environment interactions

If the same response is fitted with `A + B + C + D + E + A*B + A^2 + B^2 + D^2`, the output changes:

```
Std. deviation    1.0133
R²                0.9781
Adjusted R²       0.9658
Predicted R²      0.9384
Adeq. precision   31.863
AICc              28.92

Warning: Environment × factor interactions are significant (p = 0.0004): the best settings
depend on Humidity. Add them with Model ▸ Include environment × factor interactions, then
choose a preset or run stepwise selection.

Stationary point (maximum, outside the factor ranges): Temperature = 197.61, Time = 32.306, Stirring = 600
```

The residual standard deviation almost doubles (0.57 to 1.01), AICc rises from 11.2 to 28.9, and the stationary point moves outside the Time range. The warning is explained in [Environment x factor interaction note](#environment-x-factor-interaction-note).

## Fit statistics

The statistics grid at the top of the left pane. With a response transform, every statistic in this grid (including R², predicted R², PRESS, Mean and C.V. %) refers to the transformed response, except the last row, **Predicted R² (original scale)**, which appears only with a transform. Predictions in the Profiler tab and in the Equation section are returned to the original scale.

Notation: n is the number of runs used, p is the number of estimated coefficients (the rank of the model matrix, aliased columns excluded), y is the response, ŷ the fitted value, e = y - ŷ the residual, h the leverage of a run (the diagonal of the hat matrix).

SSE = sum(e²), SST = sum((y - mean(y))²), residual df = n - p, MSE = SSE / (n - p).

| Statistic | Definition | How to read it | Pitfalls |
|---|---|---|---|
| Runs used | n | Compare with the number of runs in the table. Differences are explained by warnings. | A run with a missing value is absent from the fit. |
| Std. deviation | s = sqrt(MSE), the residual standard deviation (RMSE). | Typical size of the unexplained variation in the response units. Compare with the response tolerance or the size of the effects. | Requires residual df > 0. Inflated by an inadequate model as well as by noise. |
| Mean | mean(y) | Centre of the data. | Not a size reference after a Ln or Inverse transform. |
| C.V. % | 100 · s / abs(mean(y)) | Noise as a percentage of the mean. Values below 10 % are typical of a well-controlled process. | Misleading when the mean is near 0, when the response has an arbitrary origin (temperature in °C), or with a transform (the mean of ln y can be near 0). |
| R² | 1 - SSE / SST | Fraction of the variation in the response explained by the model. | Never decreases when a term is added. A value near 1 with few residual df is not evidence of a good model. Blank when SST = 0. |
| Adjusted R² | 1 - (SSE / (n - p)) / (SST / (n - 1)) | R² corrected for the number of terms. Decreases when a term does not reduce MSE. | Can be below 0. Blank without residual df. |
| Predicted R² | 1 - PRESS / SST | Estimates the explained fraction for new runs. A value close to adjusted R² (within 0.2) indicates the model is not over-fitted. | A much lower value than adjusted R² indicates too many terms, an influential run, or a missing transform. Can be negative. |
| Adeq. precision | (max(ŷ) - min(ŷ)) / sqrt(p · MSE / n) | Signal-to-noise ratio of the fitted surface over the runs. A value above 4 indicates the model can be used to navigate the design space. | Computed on the fitted values at the runs, so it depends on how the runs spread. |
| PRESS | sum((e / (1 - h))²) | Sum of squared leave-one-out prediction errors. Lower is better. | Blank when a run has leverage 1 (the run alone determines a coefficient). |
| AICc | n · ln(SSE / n) + 2k + 2k(k + 1) / (n - k - 1), with k = p + 1 | Lower is better. Balances fit against the number of terms, with a small-sample correction. Differences below 2 are not meaningful, differences above 10 are decisive. | Valid only between models fitted to the same runs and the same transform. Blank when n - k - 1 <= 0. |
| BIC | n · ln(SSE / n) + k · ln(n) | Lower is better. Penalises terms more strongly than AIC for larger n. | Same restrictions as AICc. |
| Predicted R² (original scale) | 1 - PRESS_orig / SST_orig, where PRESS_orig sums the squared differences between each measured response and its back-transformed leave-one-out prediction. Shown only with a transform. | Predictive quality on the scale in which the response is measured. This is the value that ranks the candidates of the automatic model search (see [models.md](models.md)). Comparable between transforms. | Can be much lower than predicted R² on the transformed scale when back-transformed predictions are poor for a few runs. |

An SSE below 1e-24 times the sum of squares of the response is treated as rounding noise for AICc and BIC. Several perfect fits then tie, and the penalty term determines the order.

Typical thresholds:

| Statistic | Acceptable | Concern |
|---|---|---|
| Adjusted R² - predicted R² | below 0.2 | above 0.2 (the program warns) |
| Adeq. precision | above 4 | below 4 (the program warns) |
| C.V. % | depends on the process | no fixed limit |
| R² | depends on the noise level of the process | a high value does not replace the lack-of-fit and residual checks |

## ANOVA table

The table decomposes the variation of the (transformed) response. Columns:

| Column | Meaning |
|---|---|
| Source | Row name. |
| Sum of squares | Variation attributed to the row (SS). |
| df | Degrees of freedom of the row. |
| Mean square | SS / df (MS). |
| F | MS of the row divided by the residual MS, except for lack of fit. |
| p | Probability of an F at least this large if the row has no effect. |

### Rows

| Row | SS | df | F and p |
|---|---|---|---|
| Model | SST - SSE | p - 1 (rank minus one) | F = MS_model / MSE, with df (p - 1, n - p). Tests whether any term has an effect. |
| Each term (A, A*B, A^2, ...) | Partial SS: the increase in SSE if the term is removed and the model is refitted. | Number of columns of the term (1 for a numeric term, k-1 for a categorical factor with k levels). | F = MS_term / MSE, df (df_term, n - p). |
| Residual | SSE | n - p | No F or p. MS is MSE = s². |
| Lack of fit | SSE - SS_pure error | df_resid - df_pure error | F = MS_lack of fit / MS_pure error. |
| Pure error | Variation within groups of replicate runs. | n - (number of distinct design points) | No F or p. |
| Total (corrected) | SST | n - 1 | |

Notes:

- The term rows are marginal: each term is tested as if entered last, so the order of terms does not matter. Their sums of squares add up to the model sum of squares only for orthogonal designs.
- For a single-column term, the term F equals the square of the t statistic of the same coefficient in the coefficient table, and the two p-values are equal. In the example, A has t = 25.662 and F = 658.546 = 25.662².
- A term whose columns are all aliased shows df 0 and has no F or p.
- A model with a linear mixture group shows one **Linear mixture** row for the group of linear blending terms, tested against an intercept-only replacement. See [models.md](models.md).
- Without residual df (n = p) there is no Residual MS, so F and p are blank. The warning "No residual degrees of freedom" is shown. Use the half-normal plot instead (see [Effects tab](#effects-tab-screening)).

### Lack of fit and pure error

Lack of fit tests whether the residual variation is larger than the replicate variation. A significant result indicates that the model form is wrong (missing curvature or interactions) or that there are outliers.

How the program forms the test:

1. Runs with identical settings of every factor are grouped into design points. All factors count, including factors that are not in the model and measured environmental factors. Numeric settings are compared in coded units rounded to 1e-7. An inactive conditional factor counts as a fixed marker.
2. SS_pure error is the sum over groups of the squared deviations from the group mean. df_pure error = n - (number of groups).
3. df_lack of fit = df_resid - df_pure error and SS_lack of fit = SSE - SS_pure error.
4. F = (SS_lack of fit / df_lack of fit) / (SS_pure error / df_pure error), with p from the F distribution with (df_lack of fit, df_pure error).

The two rows appear only when df_pure error > 0 and df_lack of fit > 0. They are absent when:

- No design point is replicated (all runs are distinct). This is common when a measured environmental factor varies between runs, as in `reactor`.
- The model has as many terms as there are distinct design points, so df_lack of fit = 0.

How to read the result:

- p below 0.05 (the **Lack of fit** row is shown in the error colour): the model does not describe the mean response adequately. The program adds the warning "Significant lack of fit".
- p above 0.05: no evidence of lack of fit.

Example 1: `bio-bread-ccd`, response `Loaf volume`, a central composite design with replicated centre points.

```
Source             Sum of squares  df  Mean square  F       p
Model              263297          11  23936.1      39.717  < 0.0001
Residual           4218.68         7   602.669
Lack of fit        2102.02         4   525.505      0.745   0.6210
Pure error         2116.67         3   705.556
Total (corrected)  267516          18
```

Lack of fit is not significant (p = 0.62). The residual variation is of the same size as the replicate variation.

Example 2: `chem-hydrogenation-screening`, response `Conversion`, a screening design with a linear model in seven factors.

```
Source             Sum of squares  df  Mean square  F       p
Model              363.986         7   51.9979      25.844  < 0.0001
Residual           24.1439         12  2.01199
Lack of fit        24.0639         10  2.40639      60.160  0.0165   (error colour)
Pure error         0.08            2   0.04
Total (corrected)  388.13          19
```

The lack of fit holds 99.7 % of the residual SS. The linear model leaves curvature or interactions unexplained. The centre-point replicates have a very small variance (0.04).

Pitfalls:

- With few pure-error df (1 to 3) the test has little power. A non-significant result then does not demonstrate fit.
- A very small pure error makes the test significant for a deviation of no practical importance. Compare the lack-of-fit MS with the size of the effects.
- Replicates that are not independent (repeated measurements of one run) understate the pure error.
- A model that omits a factor treats runs that differ only in that factor as non-replicates, so they do not contribute to pure error.

## Coefficient table

The table is titled **Coefficients (coded units)**. One row per model column; the intercept is first.

| Column | Definition |
|---|---|
| Term | Column label. Categorical columns appear as `C[X]`, products as `A*B`, squares as `A^2`. |
| Coefficient | Least-squares estimate b in coded units. |
| Std. error | SE = sqrt(MSE · v), v the diagonal element of (X'X)^-1 for the column. |
| t | b / SE. |
| p | Two-sided p-value of t with n - p df. Tests whether the coefficient is 0 with all other terms in the model. |
| 95% CI low, 95% CI high | b -/+ t(0.975, n - p) · SE. The level is fixed at 95 %. |
| VIF | Variance inflation factor (below). Blank for the intercept. |

### Reading coefficients

- Main effect of a numeric factor: with coded units from -1 to +1, the coefficient is half of the change in the response from the low to the high setting, with the other factors at 0. The change over the full range is twice the coefficient.
- Square term `A^2`: the curvature. The mean of the responses at the low and high ends of A differs from the response at the centre by the coefficient. A negative coefficient indicates a maximum in A, a positive coefficient a minimum.
- Interaction `A*B`: the effect of A (change from low to high) at B = +1 minus the effect of A at B = -1 equals four times the coefficient. The interaction plot shows it.
- Categorical `C[X]`: the deviation of level X from the average over the levels. For two levels X and Y, `C[X]` = 1.887 means X is 1.887 above the average and Y is 1.887 below, so X is higher than Y by 3.774.
- Intercept: the prediction at the centre of the design (all coded numeric factors 0, categorical factors at the average over the levels). It equals the mean of the data only for a balanced design with a linear model.

A significant coefficient does not imply a large effect. Compare the coefficient with the response spread, not only the p-value.

### Confidence interval

The interval contains the true coefficient with 95 % confidence under the model assumptions. An interval that includes 0 corresponds to p above 0.05. A wide interval indicates an imprecise estimate: few runs, large noise, or a high VIF.

### VIF

For a column j, VIF_j = 1 / (1 - R²_j), where R²_j is the R² of regressing column j on the other predictor columns. In the program it is computed from the centred columns (not centred without an intercept). The variance of the estimate is VIF_j times larger than with orthogonal columns.

| VIF | Reading |
|---|---|
| 1 | Orthogonal: the estimate is independent of the other terms. |
| 1 to 5 | Normal for designs with squares and interactions (the example has 1.03 to 1.92). |
| 5 to 10 | Noticeable correlation. Check the design or the model. |
| above 10 | The estimate is strongly correlated with others. The program adds a warning for each such column. |

Typical causes: a constrained region (mixtures, formulation constraints), a run set that does not support the model, a measured environmental factor correlated with a designed factor, and runs lost from a design. In `bio-tablet-formulation`, the VIF of C is 37.3. Removing a term or adding runs lowers VIF. Individual coefficients with a high VIF should not be interpreted separately.

### Aliased columns

When columns of the model matrix are linearly dependent (tolerance 1e-8), the later column is aliased. It shows **aliased** in the Coefficient column, blank statistics and muted emphasis, and a warning lists it. The cause is a design that cannot separate the effects (a fractional factorial, or too few runs). Remove the term or add runs.

## Warnings and highlighting

### Emphasis

Tables carry an emphasis for each row.

| Emphasis | Appearance | Where and trigger |
|---|---|---|
| strong | Bold text | ANOVA rows **Model** and **Total (corrected)**. |
| good | Bold text | Coefficient rows (not the intercept) with p < 0.05; ANOVA term rows with p < 0.05. |
| muted | Grey text | Coefficient rows (not the intercept) with p > 0.10; ANOVA term rows with p > 0.10; aliased coefficient rows. |
| bad | Error colour (red) | ANOVA row **Lack of fit** when p < 0.05. |
| none | Normal text | Everything else, including terms with 0.05 <= p <= 0.10, the intercept, Residual and Pure error. |

The emphasis is a visual aid at fixed thresholds. It does not replace a judgement of the effect size.

### Warnings

Warnings appear at the top of the left pane. The exact triggers:

| Warning | Trigger |
|---|---|
| N excluded runs are left out. | N runs have the status excluded. |
| N runs without a value for this response are left out. | Missing or non-finite response. |
| N runs lack values for model factors and are left out. | Missing factor value; the text adds "(enter the measured environmental values)" when the factor is environmental. |
| N runs have values the transform cannot handle. | For example Ln of 0 or a negative value. |
| Aliased (not estimable with these runs): ... | Aliased columns exist. |
| No residual degrees of freedom: ... | n - p is 0 or less. |
| Predicted R² is much lower than adjusted R²: ... | Adjusted R² - predicted R² > 0.2. |
| Significant lack of fit: ... | Lack-of-fit p < 0.05. |
| Adequate precision below 4: ... | Adequate precision < 4 and residual df > 0. |
| X has VIF v: its estimate is correlated with other terms. | VIF > 10 for a term column. |
| Possible outliers (\|externally studentized residual\| > 3.5) at run N. | Absolute externally studentised residual > 3.5. The run numbers are run orders. |
| Environment × factor interactions are significant ... | See [below](#environment-x-factor-interaction-note). |

## Canonical analysis

For a model with squared terms, the canonical analysis lines appear under the statistics. They describe the stationary point of the fitted surface: the factor setting where the slope is 0.

Method. The program takes the numeric factors that have a square term in the model. It writes the quadratic part as

```
y = b0 + x'b + x'Bx
```

where x holds the coded values of these factors, b the linear coefficients, and B the symmetric matrix with the square coefficients on the diagonal and half of each interaction coefficient off the diagonal. Terms involving other factors are ignored, and those factors are held at their centre (categorical factors at the first level).

- Stationary point: x_s = -(1/2) B^-1 b, shown in actual units.
- Eigenvalues of B: the curvature along the principal axes of the surface, in coded units.

| Eigenvalues | Kind |
|---|---|
| All negative | maximum |
| All positive | minimum |
| Mixed signs | saddle point |

The first line states the kind and whether the point is inside (every coded coordinate between -1 and +1) or outside the factor ranges. The second line gives the predicted response at the stationary point (original scale, all other factors at their centre). The third line lists the eigenvalues.

How to read it:

- A maximum or minimum inside the ranges is a candidate optimum for a response with the matching goal.
- A point outside the ranges is an extrapolation. The model is not validated there. Use the Surface tab and the optimisation page ([optimisation.md](optimisation.md)) to find the best point inside the ranges.
- A saddle point is not an optimum. The response rises along some axes and falls along others, and the optimum lies on the boundary of the region.
- An eigenvalue near 0 compared with the others indicates a ridge: the response changes little along that axis, and the stationary point is poorly determined. If B is singular, the program prints "The quadratic part is singular: the surface has a ridge."
- Larger eigenvalue magnitude means sharper curvature.

Pitfalls: the analysis uses only the factors with a square term. A factor without a square term, or a measured environmental factor with an interaction, is held at the centre. For `reactor` the stationary point is computed with Humidity at 50 %RH and the catalyst at X, and it moves when the humidity changes.

## Equation

The **Equation** section prints the fitted model. Two selectors set **Actual units** or **Coded units**, and the text syntax (plain text, LaTeX, Excel, Python, C). **Copy** copies the text.

- Coded units: one equation with the coefficient table values. A note appears for conditional factors.
- Actual units: one equation per combination of categorical levels (at most 64) and per region of conditional factors.
- With a response transform, the plain and LaTeX output show the equation on the transformed scale and the solved form for the response itself.

## Diagnostics tab

The tab shows six plots. Residual plots use the **externally studentised residual** (also called deleted or RStudent residual) of each run. For run i, with internally studentised residual r_i = e_i / (s · sqrt(1 - h_i)):

```
t_i = r_i · sqrt((n - p - 1) / (n - p - r_i²))
```

This is the residual divided by an estimate of its standard deviation computed without run i. If the model is adequate, t_i follows a t distribution with n - p - 1 df, so most values lie between -2 and +2 and nearly all between -3 and +3. A value is blank when the residual df are too few (n - p <= 1).

The residuals, leverages and the other per-run statistics are not listed in a table. Leverage and DFFITS are not output. Leverage enters the studentised residuals, PRESS and Cook's distance.

If the response has a transform, all residual plots are on the transformed scale.

### Predicted vs actual

| Item | Content |
|---|---|
| x | Actual response of each run (original scale). |
| y | Fitted value (back-transformed to the original scale). |
| Line | The diagonal y = x. |

Points near the line indicate a good fit. Systematic curvature around the line indicates a missing term or a wrong transform. A point far from the line is a poorly fitted run. The plot restates R²: it does not show over-fitting.

### Normal plot of residuals

| Item | Content |
|---|---|
| x | Externally studentised residual. |
| y | Normal quantile of the rank of the residual, at (i - 0.375) / (m + 0.25) (Blom plotting position). |
| Line | The diagonal. |

Points on the line indicate normally distributed errors. Typical departures:

- S-shape: heavier or lighter tails than a normal distribution.
- A bend (one side stretched): skewed errors. Consider a transform (see the Box-Cox plot).
- One or two isolated end points: outliers.

With fewer than about 10 runs the plot has little diagnostic power.

### Residuals vs predicted

| Item | Content |
|---|---|
| x | Fitted value (transformed scale when a transform is active; the axis label then reads "Predicted (transformed)"). |
| y | Externally studentised residual. |
| Lines | Solid at 0, dashed at -3.5 and +3.5. |

A random band around 0 with a constant width indicates a good model. Patterns:

- Funnel (width grows with the fit): the variance depends on the level. Use a transform (Ln or square root, or the Box-Cox recommendation).
- Curve (U or arch): the model misses curvature or an interaction.
- Points beyond the dashed lines: possible outliers.

### Residuals vs run order

| Item | Content |
|---|---|
| x | Run order number. |
| y | Externally studentised residual. |
| Lines | Solid at 0, dashed at -3.5 and +3.5; a thin line joins the runs in order. |

A random scatter indicates that nothing changed over the experiment. A trend, a step, or runs of same-sign residuals indicate drift (tool wear, ambient changes, operator or batch change) that is not in the model. Consider including a measured environmental factor or blocking. This plot is the only check of independence.

### Cook's distance

| Item | Content |
|---|---|
| Bars | Cook's distance of each run (x is the run order). |
| Line | Dashed at 4 / n. |

For run i, with r_i internally studentised and p the number of coefficients:

```
D_i = r_i² · h_i / (p · (1 - h_i))
```

D_i measures how much all fitted values change if run i is left out. A bar above the dashed line (4 / n) marks an influential run. Check that run: confirm the measurement, and refit with the run excluded (see [setup-and-data.md](setup-and-data.md)) to see how the model changes. A run should not be removed only because it is influential.

In a design with few runs, every run has a high leverage, and D_i above 4 / n can occur for several runs.

### Outlier limits

The outlier warning and the dashed lines of the residual plots use the same limit: 3.5 on the absolute externally studentised residual. A point beyond a dashed line is named in the warning.

## Box-Cox plot

The sixth plot of the Diagnostics tab helps choose a power transform of the response. The transformed response is

```
y(λ) = (y^λ - 1) / (λ · g^(λ - 1))   for λ not 0
y(λ) = g · ln(y)                     for λ = 0
```

with g the geometric mean of y. The scaling by g makes the residual sums of squares comparable between values of λ.

The program refits the current model structure to y(λ) for λ from -3 to 3 in steps of 0.05 and plots ln(SSE) against λ.

| Element | Meaning |
|---|---|
| Curve **ln(SSE)** | Residual sum of squares of the model for each λ. The lowest point is the best λ. |
| Dashed horizontal line | The confidence threshold: min ln(SSE) + ln(1 + t² / df), with t the 97.5 % quantile of t with the residual df. |
| Dashed vertical lines **95% CI** | The smallest and largest λ whose curve value lies below the threshold. |
| Vertical line **Current** | The λ of the current transform: 1 for None, 0 for Ln and Log10, 0.5 for Square root, -1 for Inverse, the exponent for a Power transform. |
| Vertical line **Best** | λ at the minimum of the curve. |
| Note | "Best λ = x, 95% CI a to b. Recommended: T." |

Recommended transform. If λ = 1 is inside the interval, the recommendation is None (no transform is needed). Otherwise the program takes the standard λ values inside the interval (0.5, 0, -0.5, -1, 2, -2, 3, -3) and recommends the one closest to the best λ. Without a standard value inside the interval, it recommends the best λ as a power. Every standard recommendation is in the transform list of the Analysis page; a recommendation of another power can only be set through the engine API (see [../API.md](../API.md)).

| λ | Transform |
|---|---|
| 1 | None |
| 0.5 | Square root |
| 0 | Ln |
| -1 | Inverse |
| other | Power λ |

The transform is applied in the transform box in the top row (see [models.md](models.md)).

Example (`reactor`, Yield): "Best λ = 1.55, 95% CI -0.25 to 3. Recommended: None." The interval includes 1, so no transform is needed.

Example (`chem-hydrogenation-screening`, Conversion): "Best λ = 3, 95% CI 1.3 to 3." The best value is at the edge of the grid and the interval excludes 1. This model has significant lack of fit, so the Box-Cox result is not reliable until the model form is corrected.

Pitfalls:

- All response values must be positive. Otherwise the plot is replaced by the message "Box-Cox requires all response values to be positive".
- Residual df are required.
- The curve depends on the model structure. With a wrong structure (lack of fit), the Box-Cox result is not reliable. Fix the model first.
- A wide interval (common with few runs) means the data do not determine λ. Prefer λ = 1, or the simplest transform inside the interval.
- A best λ at -3 or +3 indicates that the data do not locate a minimum within the grid.
- Transforms change the meaning of R², PRESS, AICc and C.V. %. They cannot be compared between transforms.
- The interval is the range between the smallest and largest grid value below the threshold. A curve with two separate minima would still give one range.

## Effects tab: screening

The tab shows two plots for ranking the effects in the model. They are used mainly for screening designs, where the model is saturated or nearly saturated and the ANOVA has few or no residual df.

### Effects and PSE

The program takes every non-aliased, non-intercept column of the model and computes its effect:

effect = coefficient · (highest value of the column − lowest value of the column), over the coded region. The effect is the change in the fitted response when the column goes from its lowest to its highest value:

- Main effects of numeric factors, interactions, and categorical columns (coded -1 to +1): effect = 2 · coefficient. For a two-level categorical factor this is the difference between the two levels.
- Squares (coded 0 to 1): effect = coefficient.
- Mixture components (pseudo-components 0 to 1): effect = coefficient.

Standardised effect:

- With residual df: the t statistic of the coefficient.
- Without residual df: effect / PSE.

Lenth's method estimates the standard error of the effects without residual df, assuming that most effects are 0 (effect sparsity). For m effects c_1 to c_m:

```
s0  = 1.5 · median(|c|)
PSE = 1.5 · median(|c| for |c| < 2.5 · s0)       (pseudo standard error)
d   = m / 3
ME  = t(1 - α/2, d) · PSE                         (margin of error)
SME = t(γ, d) · PSE, γ = (1 + (1 - α)^(1/m)) / 2  (simultaneous margin of error)
```

with α = 0.05. An effect larger than ME in absolute value is likely active. SME corrects for the number of effects inspected and is conservative: an effect above SME is active with strong confidence. The plots use ME. PSE and SME are computed but not displayed.

### Pareto chart of standardised effects

| Item | Content |
|---|---|
| Bars | Absolute standardised effect of each term, sorted from largest at the top. A bar of the second series colour marks a negative effect. |
| x axis | **\|t\|** when there are residual df; **\|effect\| / PSE** when there are none. |
| Dashed vertical line | **t (α = 0.05)**: t(0.975, residual df). Without residual df, the line is **Lenth ME** (ME / PSE). |

A bar beyond the dashed line indicates a significant effect. With residual df, the bars repeat the t values of the coefficient table in sorted form, which makes them easy to compare.

### Half-normal plot of effects

| Item | Content |
|---|---|
| Points | Absolute effect (x) against the half-normal quantile of its rank (y): Φ^-1(0.5 + 0.5 · (i - 0.5) / m). |
| Labels | The terms with an effect above ME and the three largest effects. |
| Diagonal line | The line through the origin with slope 1 / PSE. Effects from pure noise lie on it. |
| Dashed vertical line **ME** | Lenth's margin of error. |

How to read it: inactive effects form a straight line through the origin along the diagonal. Active effects fall to the right of the line, away from it. Terms clearly right of the line and beyond ME are the effects worth keeping.

Example (`chem-hydrogenation-screening`, Conversion, linear model in 7 factors, 12 residual df). Lenth values: PSE 0.300, ME 1.129, SME 2.702.

```
Term     |Effect|  Beyond ME
C        6.663      yes
E        5.812      yes
A[Pd/C]  3.25       yes
B[MeOH]  0.5        no
D        0.337      no
G        0.062      no
F        0.037      no
```

The Pareto chart for the same model uses t, because residual df exist: the |t| values are 9.394 (C), 8.196 (E), 5.020 (A[Pd/C]) and below 0.8 for the rest, with the reference at 2.179. Three effects are active. The Lack of fit row in the ANOVA is significant for this response, so the linear model leaves curvature unexplained (see [Lack of fit](#lack-of-fit-and-pure-error)).

Pitfalls:

- Lenth's method assumes effect sparsity. With many active effects the PSE is inflated and active effects are missed.
- It assumes effects of equal precision. Effects of different columns have the same standard error only in orthogonal designs.
- With few effects (m below 8) the margin is poorly estimated.
- For a categorical factor with more than two levels, each column compares one level with the average of all levels, so its effect is not the difference between two levels.
- Squares and interactions are screened together with the main effects. Under the hierarchy principle, keep the main effect of any active interaction.

## Profiler tab

The profiler shows how the prediction changes along each factor. It has one plot per factor in the model.

Controls:

- Each plot has a value box under it. The value box holds the **current** setting of the factor in actual units (a drop-down for categorical factors). The other factors in each plot are held at their current values.
- A click in a plot sets the current value of that factor to the clicked position.
- **Center** resets all factors to their centre (first level for categorical factors, the centre point for mixtures).

The heading line above the plots reads

```
Prediction 88.5519    95% CI 87.442 to 89.662   95% PI 86.897 to 90.207
```

(`reactor`, Yield, all factors at their centre.)

| Item | Meaning |
|---|---|
| Prediction | The fitted response at the current settings (original scale). |
| 95% CI | Confidence interval of the **mean** response at this setting: ŷ ± t(0.975, n - p) · SE_fit, with SE_fit² = MSE · x'(X'X)^-1 x. It reflects the uncertainty of the fitted surface. |
| 95% PI | Prediction interval for one **new run** at this setting: ŷ ± t(0.975, n - p) · sqrt(SE_fit² + MSE). It is wider than the CI because it adds the run-to-run noise. |

With a transform, the intervals are computed on the transformed scale and mapped back, so they are asymmetric. For Inverse and negative powers, an interval that contains 0 on the transformed scale maps to an unbounded interval.

In each plot:

| Item | Content |
|---|---|
| Line **Prediction** | The prediction along the factor (41 points; levels for discrete and categorical factors). |
| Band or error bars **95% CI** | The confidence interval of the mean response (a band for continuous factors, error bars for discrete and categorical ones). |
| Dashed vertical line **Current** | The current value of the factor. |

All plots share one vertical axis, so the slopes are comparable: a steep line indicates a large effect over the range of the factor, a flat line a small effect. Curvature indicates squared terms, and a plot whose shape changes when another factor is moved indicates an interaction.

Use the profiler to explore settings, to see the interval of a candidate setting, and to compare it with a specification. The prediction interval is the one that applies to individual runs.

Pitfalls:

- The intervals assume the model is correct. They do not include model-selection uncertainty.
- Predictions outside the design region (extrapolation) are not validated. The intervals widen there, but the widening underestimates the real error.
- The interval of the mean (CI) applies to the average of many runs. For a single batch use the PI.
- Environmental factors appear as plots. Set them to the expected conditions of use.

## Surface tab

The tab shows the response over two numeric factors, with all other factors at their current values from the Profiler tab.

Controls:

- Two selectors choose the factors for the x and y axes (labelled with symbol and name). Both lists contain only numeric factors that are in the model. The factors must differ.
- **3D** switches between the contour plot and the 3D surface.

### Contour plot

- A colour map of the predicted response over a 61 by 61 grid, with labelled contour lines at round values.
- The runs are drawn as points. Hover to see the run number. All runs are shown in the plane of the two factors, whatever the other factors are. They can lie away from the plotted slice.
- Parallel straight contours indicate no interaction between the two factors. Curved or elliptical contours indicate curvature. A saddle shows crossing contours. A rising ridge shows contours stretched in one direction.

### 3D surface

- A projected surface on a 31 by 31 grid. Drag with the pointer to rotate (azimuth and elevation).
- It shows the same prediction as the contour plot, with the shape easier to see and the numbers harder to read.

For mixture factors, the other components are rescaled in proportion so that the total stays 1. For two mixture components, the grid is limited to the region where the two proportions do not exceed 1.

Pitfalls:

- The surface is a slice. Changing the other factors in the Profiler tab moves it.
- The runs plotted on the contour plot are not at the height of the slice.
- A surface that is well defined between the runs is not necessarily accurate in corners without runs.

## Interaction tab

The tab shows the model prediction for one factor at two levels of another. The two selectors choose the **x** factor (swept over its range) and the **y** factor (one line per level). Other factors are held at the current profiler values.

- Numeric y factor: two lines, at the low and the high settings (coded -1 and +1). For a mixture component the lines are at 0 and 1 of its pseudo-component.
- Categorical y factor: one line per level.
- The legend entries read "Name = value".

How to read it:

- Parallel lines: no interaction between the two factors in the model. The effect of the x factor is the same at both levels.
- Lines with different slopes: interaction. The more they differ, the stronger the interaction.
- Crossing lines: a strong interaction where the best level of one factor depends on the other. This is the main reason to study the factors together.

The lines come from the fitted model, not from the data. If the model has no `A*B` term, the lines for A and B are parallel by construction (unless a square term or a third factor interaction acts). A strong interaction in the data that is not in the model appears in the residuals instead.

The y factor shows only the two extremes. Curvature in the y factor is not visible. Use the Profiler or Surface tab for that.

## Data tab

The tab shows the raw response against one factor, chosen with the selector. It does not depend on a model, and the selector lists the factors that are not in the model.

- Points are the runs (hover for the run number). Excluded runs and runs where a conditional factor does not apply are omitted.
- For a numeric factor, a dashed least-squares line is drawn and the legend gives the correlation, `r = ...`.
- For a categorical factor, the points are placed at the levels.

Use it to inspect the data before and after fitting: outliers, clusters, gaps, and non-linear relationships. The correlation is a marginal one. In a non-orthogonal design it can differ from the model coefficient of the same factor, because the other factors are not held constant.

## Robustness tab: signal-to-noise

The tab analyses a response for robustness. The classic use is a Taguchi design with controlled factors in an inner array and noise factors in an outer array ([designs.md](designs.md)). The aim is to find controlled settings that give a response that is both on target and insensitive to noise.

Control: the selector **Nominal is best**, **Larger is better**, **Smaller is better** selects the signal-to-noise (S/N) ratio. The text beside it reads "Highest S/N: ..." and lists the best level of each factor.

### Grouping

1. Each run is assigned to a group by its settings of the **controlled** factors only. Noise factors, environmental factors and replicates therefore vary inside a group.
2. Excluded runs and runs without a value are omitted.
3. For each group the table lists the number of runs (n), the mean, the standard deviation (n - 1 divisor) and the S/N ratio.
4. Groups with too few runs are omitted: nominal is best requires at least 2 runs per group, the other two types at least 1. At least two usable groups are required, otherwise the tab shows the message "signal-to-noise analysis needs repeated observations at the same controlled settings (crossed noise conditions or replicates)".

If the project has no controlled factors, the tab shows "no controlled factors".

### S/N ratios

For a group of runs y_1 to y_n, in decibels:

| Type | Formula | Use when | Larger S/N means |
|---|---|---|---|
| Larger is better | S/N = -10 · log10( (1/n) · sum(1 / y_i²) ) | The response has no upper limit and a higher value is wanted (strength, yield). | A high response with little variation. |
| Smaller is better | S/N = -10 · log10( (1/n) · sum(y_i²) ) | The response should be as small as possible, with 0 as ideal (defects, shrinkage, noise level). | A low response with little variation. |
| Nominal is best | S/N = 10 · log10( mean² / s² ), s² the sample variance | The response has a target and variation around the mean must be small (a dimension). | A small coefficient of variation: mean large compared with the spread. |

Notes:

- The larger-is-better ratio is defined only for responses that are not 0. Zero values give a ratio that is not finite. Such levels are never chosen as best.
- The nominal-is-best ratio measures the **spread relative to the mean** only. It does not measure how close the mean is to the target. After choosing the factors that raise this ratio, adjust the mean with a factor that moves the mean but not the ratio (the mean plots show this). The program does not choose such a factor.
- A 1 dB gain in S/N corresponds to about a 12 % change in the root mean square (smaller or larger is better) or in the coefficient of variation (nominal is best): 10^(1/20) = 1.12.
- The S/N is a statistic of the group. It has no confidence interval. A difference of less than 1 dB between levels is hardly meaningful with 3 to 4 runs per group.

### Plots and the best levels

For each controlled factor with 2 to 9 levels, the tab draws two plots:

- **Mean S/N (dB)**: the average of the S/N ratios of the groups at each level. All S/N plots share the vertical scale.
- **Mean response**: the average of the group means at each level.

The "Highest S/N" text lists, for each factor, the level with the highest mean S/N. Levels are chosen factor by factor, which assumes that the effects of the factors on the S/N are additive. The choice does not use the model, and it does not test for interactions.

A factor with a large range of mean S/N over its levels influences the robustness. A factor that changes the mean response plot but not the S/N plot is an adjustment factor: use it to move the mean to the target.

Example (`mfg-injection-molding-taguchi`, response `Shrinkage`, factors A to D controlled, noise factors resin lot and conditioning humidity, 3 to 4 runs per group). The nominal-is-best table:

```
A    B    C   D   n  Mean    Std. dev.  S/N
230  700  16  90  4  1.1267  0.04127    28.725
270  550  8   90  4  1.4085  0.04455    29.999
250  550  16  50  4  1.1235  0.1008     20.939
270  700  12  50  4  1.1158  0.07772    23.141
...
```

Check of the first row: 10 · log10(1.1267² / 0.04127²) = 20 · log10(27.30) = 28.72. The best levels are:

```
Highest S/N: Melt temperature = 230, Hold pressure = 700, Cooling time = 8, Mould temperature = 90
```

With **Smaller is better** the same data give S/N values between about -1 and -3 dB and different best levels (`Cooling time = 16`, `Mould temperature = 50`). The two ratios answer different questions. Choose the one that matches the response goal.

## Environment x factor interaction note

A measured environmental factor (such as the humidity in `reactor`) cannot be set. If the response depends on it through interactions with the controlled factors, the best settings depend on its value. The note warns when such interactions are missing from the model.

Test:

1. The program takes the environmental factors and the designed (non-mixture) factors that appear as main effects in the model.
2. It forms all products of an environmental and a designed factor that are not yet in the model, and fits the model with these terms added.
3. It compares the two models with the extra-sum-of-squares F test:

```
F = ((SSE_model - SSE_larger) / (p_larger - p_model)) / MSE_larger
```

4. If p < 0.05, the warning is shown.

Message:

```
Environment × factor interactions are significant (p = 0.0004): the best settings depend on Humidity.
Add them with Model ▸ Include environment × factor interactions, then choose a preset or run stepwise selection.
```

The message names the environmental factors that are in the model. The **Model** button has the checkable item **Include environment × factor interactions**. When it is checked, the presets (Linear, Two-factor interactions, Quadratic, Cubic) also contain these products. Stepwise selection can then remove the products that are not needed.

No note appears when:

- No environmental factor is in the model as a main effect (add it first).
- All the products are already in the model.
- The enlarged model has no residual df.
- p >= 0.05.

The test considers all products together, so it has several df. It can be significant even if no single product is. The note does not identify which product is needed. Check the individual terms after adding them.

In the `reactor` example, the stored model contains A*E and D*E. Without them (see the [worked example](#removing-the-environment-interactions)) the note appears with p = 0.0004.

## Quick reference of thresholds

| Quantity | Threshold | Effect in the program |
|---|---|---|
| Coefficient or term p | < 0.05 | bold row |
| Coefficient or term p | > 0.10 | muted row |
| Lack-of-fit p | < 0.05 | error colour and a warning |
| Adjusted R² - predicted R² | > 0.2 | warning |
| Adequate precision | < 4 | warning |
| VIF | > 10 | warning |
| Absolute externally studentised residual | > 3.5 | outlier warning and outside the dashed lines in the residual plots |
| Cook's distance | > 4 / n | above the dashed line |
| Environment × factor interactions | p < 0.05 | warning with a hint |
| Confidence and prediction intervals | 95 % | fixed |
| Lenth's α | 0.05 | fixed |
| Box-Cox grid | λ from -3 to 3, step 0.05 | fixed |
