# Designs and design evaluation

This page describes the **Design** page, where the program generates the runs of an experiment, and the **Evaluate** page, where the program computes statistical properties of a design before any run is made.

Related pages:

- [setup-and-data.md](setup-and-data.md): factors, roles, ranges, mixture components, conditions and the Runs sheet.
- [models.md](models.md): model terms and formulas, which the Evaluate page and the optimal designs use.
- [analysis.md](analysis.md): fitting the data after the runs are done.
- [optimisation.md](optimisation.md): finding settings from a fitted model.

The examples use the factors Temperature (A, 150 to 190 °C), Pressure (B, 1 to 3 bar), Time (C, 10 to 30 min), Catalyst (D, 1 to 2 %), Stirring (E, 100 to 300 rpm) and Feed (F, 5 to 15 g/min). All are continuous and controlled unless stated. All numbers in the examples were produced by the program.

## Contents

- [Using the Design page](#using-the-design-page)
- [Concepts shared by all designs](#concepts-shared-by-all-designs)
- [Design types](#design-types)
- [Choosing a design](#choosing-a-design)
- [Evaluate page](#evaluate-page)
- [Adding single runs](#adding-single-runs)

## Using the Design page

The page has three columns.

1. **Method list.** Methods are grouped as Factorial, Screening, Response surface, Optimal, Space filling, Mixture and Robust. A method that cannot be used with the current factors is greyed out. Hovering over it shows the reason. If no method is available, the page shows "Add factors on the Setup page first."
2. **Options.** The name and a short description of the selected method, followed by its options. Hovering over an option label shows a tooltip. The values entered for a method are kept while the project is open, so switching between methods does not reset them.
3. **Preview.** The notes of the design (run counts, resolution, alias structure, efficiencies), the line `Model:` with the model the design supports, and the table of runs.

Steps:

1. Select a method and set its options.
2. Press **Preview**. The program generates the runs and shows them. Any edit to an option clears the preview.
3. Optionally open the Evaluate page with source **Design preview** to check the design (see [Evaluate page](#evaluate-page)).
4. Press the apply button. The program writes the runs to the Runs sheet and switches to the Runs page.

### Apply buttons

| Control | Meaning |
|---|---|
| **Randomize run order** | Default on. The run order of the new runs is a random permutation. Runs of a blocked design are randomised within each block, and blocks stay in order. New runs follow existing runs in run order. The standard order keeps the order of the preview. When off, the run order equals the standard order. |
| **Replace runs** | Deletes all existing runs, with their results, and all stored models, and writes the previewed runs. If runs exist, a dialog "Replace runs?" asks for confirmation. **Undo** restores the previous state. |
| **Append to runs** | Keeps the existing runs and adds the previewed runs after them. Not shown for augmenting designs. |
| **Add runs** | Replaces **Replace runs** when the preview is an augmentation (optimal design with **Augment existing runs**). The existing runs stay and the new runs are added. |

The runs are created with status Planned. The point type column holds the kind of point (factorial, centre, axial, edge, vertex, optimal, augment and so on). Factors not set by the design (environmental factors) stay blank, except where an optimal design uses known values.

When a design has more than one block, the program creates a categorical factor named **Block** with the role Block and the levels 1, 2, and so on. An existing Block factor is reused and its levels are replaced.

The **Model** line of the preview is stored with the design. It is the default model on the Analysis and Evaluate pages. Measured environmental factors and block factors are added to it as main effects.

### Factors used

Designs set **Controlled** and **Noise** factors. **Environmental** factors are not set by a design. The preview notes list them, and their values are recorded during the experiment and used as covariates in the analysis. The only exception is the optimal design, which can plan around known or forecast values of environmental factors.

Every factor must have at least two levels. For a continuous or mixture factor, Low must be below High. Otherwise the program reports an error for every method.

If a factor has a condition (it applies only when another factor has a certain value), see [setup-and-data.md](setup-and-data.md). The factor is left blank in the runs where it does not apply. A full factorial contains each valid combination once per replicate and optimal designs are built with the conditions taken into account. All other designs place their points without regard to the conditions, and a note in the preview states this.

## Concepts shared by all designs

### Coded units

Designs are built in coded units. A continuous factor runs from -1 (Low) to +1 (High), with 0 at the centre. A discrete factor is coded from its lowest to its highest level. The preview table shows actual units.

### Run count

Each design type has its own run count formula, given in its section. The count shown in the preview includes centre points and replicates. A design is only useful for a model when the number of runs is at least the number of model coefficients. More runs than coefficients give residual degrees of freedom, which allow an error estimate and tests (see [Evaluate page](#evaluate-page)).

### Centre points

A centre point has all numeric factors at the middle of their range. Centre point runs:

- estimate pure error through repeated runs at one setting,
- test for curvature in two-level designs,
- add information on the quadratic terms in response surface designs.

Categorical factors cannot have a centre. Where a design has centre points and categorical factors, the centre points cycle through the levels of the categorical factors (full factorial, fractional factorial, Plackett-Burman, definitive screening). A discrete factor with no level at the centre of its range is a problem: the program then adds no centre points.

### Replicates

A replicate repeats the whole set of runs. Replicates are available in the full factorial, fractional factorial and the two mixture designs. Replicated runs give a pure-error estimate. For other designs, repeat settings with centre points or choose a larger run count.

### Blocks

A block is a group of runs made under similar conditions (a day, a batch of material, an operator). Block effects are estimated separately from the factor effects. Blocks are created by **Block by replicate** (full factorial) and **Blocks** (central composite).

### Seeds

The optimal and Latin hypercube designs use random numbers. Their **Seed** option fixes the sequence, so the same inputs give the same design. A different seed can give a different design of similar quality. The random run order of **Randomize run order** has no seed option.

### Factor types by design

| Design | Continuous | Discrete | Categorical | Mixture |
|---|---|---|---|---|
| Full factorial | yes | yes, own levels | yes, any number of levels | no |
| Fractional factorial | yes | yes, lowest and highest level | exactly 2 levels | no |
| Plackett-Burman | yes | yes, lowest and highest level | exactly 2 levels | no |
| Definitive screening | yes | yes, nearest level to each design value | exactly 2 levels | no |
| Central composite | yes | yes, nearest level | yes, crossed with the design | no |
| Box-Behnken | yes | yes, at least 3 levels | yes, crossed with the design | no |
| Optimal (custom) | yes | yes | yes | yes |
| Latin hypercube | yes | yes, balanced | yes, balanced | no |
| Simplex lattice, simplex centroid | process factors only, crossed | process factors, crossed | process factors, crossed | at least 2 |
| Taguchi | yes | yes | yes | no |

## Design types

### Full factorial

Every combination of the levels of all factors.

**When to use.** Few factors (up to about 4), when all interactions matter, or with categorical factors that have more than two levels. A two-level full factorial estimates all main effects and all interactions.

**Availability.** At least one controlled or noise factor and no mixture components.

| Option | Meaning | Default |
|---|---|---|
| **Levels per continuous factor** | Number of equally spaced levels between Low and High, 2 to 7. Discrete and categorical factors use their own levels. | 2 |
| **Center points** | Centre runs added after the factorial runs, 0 to 50. Added only when at least one factor is numeric. Categorical factors cycle through their levels. | 0 |
| **Replicates** | Number of times the whole set is repeated, 1 to 20. | 1 |
| **Block by replicate** | Each replicate becomes a block. Has an effect with 2 or more replicates. | off |

**Run count.** (product of the numbers of levels) + centre points, times replicates. The limit is 100000 runs.

**Model supported.** Two-factor interactions; quadratic when any factor has 3 or more levels.

**Example.** Four factors, two levels, 2 centre points: 16 + 2 = 18 runs.

```
 #  Type       A: Temperature  B: Pressure  C: Time  D: Catalyst
 1  factorial  150             1            10       1
 2  factorial  190             1            10       1
 3  factorial  150             3            10       1
 ...
16  factorial  190             3            30       2
17  center     170             2            20       1.5
18  center     170             2            20       1.5
```

Evaluation for the 2-factor-interaction model: D-efficiency 89.85 %, residual df 7, every standard error 0.25 σ, every VIF 1.00. All 16 factorial runs are orthogonal. The centre points are the reason the D-efficiency is below 100 %.

### Fractional factorial (2-level)

A regular 2^(k-p) fraction of the full two-level factorial: 2^(k-p) runs for k factors, with p generators.

**When to use.** Screening of 4 to about 15 factors with two levels each, when higher-order interactions can be assumed small.

**Availability.** At least 3 factors, all with two levels (continuous, discrete, or categorical with exactly 2 levels), no mixture components.

| Option | Meaning | Default |
|---|---|---|
| **Runs** | `Smallest with minimum resolution`, or a power of two from the list (each entry names the fraction, for example `8 runs (1/2 fraction)`). The list depends on the number of factors. | Smallest with minimum resolution |
| **Minimum resolution** | III, IV or V. Used only when **Runs** is automatic and no generators are given. | IV |
| **Generators** | Optional text such as `D=AB, E=AC`. When given, **Runs** and **Minimum resolution** are ignored. | empty |
| **Center points** | Centre runs, 0 to 50. | 0 |
| **Replicates** | 1 to 20. | 1 |
| **Fold over** | Adds the mirror image of the fraction (all signs reversed). | off |

**Resolution.** The resolution is the length of the shortest word in the defining relation.

| Resolution | Meaning |
|---|---|
| III | Main effects are aliased with two-factor interactions. |
| IV | Main effects are clear of two-factor interactions. Two-factor interactions are aliased with each other. |
| V | Main effects and two-factor interactions are clear of each other. Two-factor interactions are aliased with three-factor interactions. |

**Minimum aberration.** Among all fractions of the same size, the program selects the one with the fewest short words in the defining relation. Aberration is judged from the word length pattern, the number of words of each length, starting with the shortest. A fraction with minimum aberration has the least aliasing of low-order effects. For large spaces of generator choices the program uses a search, and the result is the best fraction found.

**Automatic run count.** With **Runs** automatic, the program takes the smallest number of runs 2^n, with 2^n greater than the number of factors, for which a fraction of the required resolution exists. If none exists below 65536 runs and k is at most 16, it gives the full factorial.

**User generators.** A generator has the form `target = word`. The target is a factor symbol. The word is a product of factor symbols and an optional sign.

- Symbols are those on the Setup page (case does not matter). Write them next to each other (`ABC`) or separated by `*` (`A*B*C`). Use `*` when symbols have more than one character.
- Separate generators with commas, semicolons or new lines.
- A leading `-` reverses the sign of the generated column.
- The word must contain at least two factors. Each target can appear once. A word that contains another target is rewritten with the generator of that target.
- The factors that are not targets are the base factors. They form a full factorial of 2^(number of base factors) runs.
- A generator set that makes a main effect fully aliased with another main effect or with a two-factor interaction of two other factors (resolution below III) is rejected with an error.

**Defining relation and alias structure.** The preview notes show the generators, the defining relation (`I = ABD = ACE = BCDE`) and the alias structure. The alias structure lists groups of effects that cannot be told apart. In `A + BD + CE`, the estimate for A is the sum of the effects of A, BD and CE. The list covers main effects and two-factor interactions with aliases of up to three factors, and shows at most 60 groups. With more than 16 generators only the short words of the defining relation are listed. A negative word in the defining relation gives a minus sign in the alias chain.

**Fold-over.** A fold-over adds the mirror image of the fraction. The combined design has twice the runs and, for a resolution III fraction, resolution IV: the main effects are separated from the two-factor interactions. A fold-over of a resolution IV or V fraction repeats the fraction and adds no information, so the program refuses it. Replicates and centre points apply to the combined design.

**Model supported.** Main effects (linear), or main effects and two-factor interactions for resolution V.

**Run count.** 2^(k-p) (doubled with **Fold over**) + centre points, times replicates.

**Example 1.** Four factors, automatic runs. The program generates a 2^(4-1) fraction of 8 runs, resolution IV:

```
2^(4-1) fraction, 8 runs, resolution IV.
Generators: D = ABC
Defining relation: I = ABCD
Alias structure (up to three-factor interactions):
  A + BCD
  B + ACD
  C + ABD
  D + ABC
  AB + CD
  AC + BD
  AD + BC
```

Main effects are clear of two-factor interactions. The pairs AB + CD, AC + BD and AD + BC cannot be separated. Evaluated for the main-effects model: D-efficiency 100 %, 3 residual df, power 49 % at Effect / σ = 2.

**Example 2.** Five factors, **Minimum resolution** III, automatic runs: 2^(5-2), 8 runs.

```
2^(5-2) fraction, 8 runs, resolution III.
Generators: D = AB, E = AC
Defining relation: I = ABD = ACE = BCDE
Alias structure (up to three-factor interactions):
  A + BD + CE
  B + AD + CDE
  C + AE + BDE
  D + AB + BCE
  E + AC + BCD
  BC + ACD + ABE + DE
  BE + ADE + ABC + CD
```

Entering `D=AB, E=AC` in **Generators** gives the same design. With **Fold over** on, the design has 16 runs, resolution IV, and the alias list is replaced by the note that the fold-over runs separate main effects from two-factor interactions.

If the generator `E=-ABD` is entered together with `D=ABC`, E is rewritten as `-C`. This is a main effect aliased with a main effect and the program reports an error.

### Plackett-Burman

A two-level orthogonal design for main effects in a multiple of 4 runs.

**When to use.** Screening many factors in few runs when only main effects are of interest. Plackett-Burman designs exist for run counts that a regular fraction does not offer (12, 20, 24, 28, ...).

**Availability.** At least 2 factors with two levels each, no mixture components.

| Option | Meaning | Default |
|---|---|---|
| **Runs (0 = smallest)** | Number of runs, a multiple of 4 from 0 to 96. 0 selects the smallest design with at least k + 1 runs. A run count for which no design is available gives an error. | 0 |
| **Center points** | Centre runs, 0 to 50. | 0 |

**Run count.** The smallest multiple of 4 that is at least k + 1 (when **Runs** is 0), plus centre points. A design with n runs holds up to n - 1 factors.

**Aliasing.** When n is a power of two (8, 16, 32) the design is a regular fraction of resolution III: each main effect is fully aliased with two-factor interactions. For other sizes (12, 20, 24, ...) the design is non-regular: each main effect is partially aliased with many two-factor interactions, with correlation below 1. Unused columns (n - 1 - k) are not assigned to factors and are noted.

**Model supported.** Main effects.

**Example.** Three factors with **Runs** = 12 give a 12-run design in which 8 columns are unused. Six factors give 8 runs with 1 unused column:

```
Plackett-Burman design with 8 runs for 6 factors.
Main effects are fully aliased with two-factor interactions (regular design, resolution III).
1 unused columns estimate error for effect screening.
```

Evaluated for the main-effects model: 1 residual df, standard error 0.354 σ, power 17.6 % at Effect / σ = 2. Eight runs are enough to estimate the effects but give almost no power.

### Definitive screening

A three-level design (each factor at -1, 0 and +1) built from a conference matrix. Each run is paired with its mirror image, and one centre run is added.

**When to use.** Screening 4 to about 12 continuous factors when curvature or an interaction may be present. A definitive screening design (DSD) can estimate main effects, and, with few active factors, some quadratic and two-factor interaction terms.

**Properties.**

- Main effects are orthogonal to each other.
- Main effects are not aliased with any two-factor interaction or quadratic effect.
- Quadratic effects are estimable, but not independent of each other or of interactions.

**Availability.** At least 3 factors, no mixture components. Categorical factors must have exactly 2 levels. The program takes the conference matrix of order m, the smallest even number not less than k for which one exists (for odd k, m = k + 1, and one column is not used).

| Option | Meaning | Default |
|---|---|---|
| **Extra center points** | Centre runs added to the one that is always present, 0 to 10. | 0 |

**Run count.** 2m + 1 + extra centre points, where m is the order of the conference matrix: 9 runs for 4 factors, 11 for 3 factors (m = 4), 13 for 5 or 6 factors (m = 6). Orders for which no matrix exists are skipped, so some k have more columns than factors.

**Special cases.**

- Categorical factors take alternating levels in the runs where continuous factors are at their centre.
- Discrete factors use the level nearest to each design value.

**Model supported.** Main effects. Use stepwise selection on the Analysis page to find active quadratic and interaction terms.

**Example.** Four factors:

```
Definitive screening design: 9 runs for 4 factors (conference matrix of order 4).
 #  Type       A: Temperature  B: Pressure  C: Time  D: Catalyst
 1  screening  170             3            30       2
 2  screening  150             2            30       1
 3  screening  150             1            20       2
 4  screening  150             3            10       1.5
 5  screening  170             1            10       1
 6  screening  190             2            10       2
 7  screening  190             3            20       1
 8  screening  190             1            30       1.5
 9  center     170             2            20       1.5
```

Evaluated for the main-effects model: D-efficiency 72.3 %, 4 residual df, all VIF 1.00, standard error of a main effect 0.408 σ. With six factors the design has 13 runs and D-efficiency 79.86 %.

### Central composite

A factorial core, axial (star) points and centre points. It supports a full quadratic model.

**When to use.** Optimisation of 2 to 6 continuous factors after screening, when the experiment can go slightly outside the range of the factors, or with face-centred or inscribed variants when it cannot.

**Availability.** At least 2 numeric factors (continuous or discrete), no mixture components. Categorical factors are allowed: the whole design is repeated for every combination of the categorical levels.

**Structure.**

- Factorial points at (±1, ..., ±1), from a full factorial or a resolution V fraction.
- Axial points at ±α on each axis, with the other factors at 0.
- Centre points.

| Option | Meaning | Default |
|---|---|---|
| **Axial distance** | The distance α of the axial points in coded units. Choices below. | Rotatable |
| **Inscribed (keep all points within ranges)** | Scales all points by 1/α, so the axial points are at ±1 and the factorial points at ±1/α. | off |
| **Factorial core** | `Resolution V fraction when smaller` or `Full factorial`. The fraction is used for 5 or more factors. | Resolution V fraction when smaller |
| **Center points** | Centre runs, 0 to 50. Per combination of categorical levels. | 5 for 2 factors, 6 for 3, 7 for 4, 6 for 5, 9 for 6, 10 for more |
| **Blocks** | `None`, or `Factorial / axial` (two blocks). | None |

**Axial distance.** F is the number of factorial runs, k the number of factors, n0 the number of centre points.

| Choice | α | Property |
|---|---|---|
| Rotatable | F^(1/4) | Prediction variance depends only on the distance from the centre. |
| Orthogonal | ((sqrt(F + T) - sqrt(F))^2 × F / 4)^(1/4), T = 2k + n0 | The quadratic terms are uncorrelated with the other terms (VIF 1.00 for three factors in the table below). |
| Face-centered (α = 1) | 1 | All points within the factor ranges. Three levels per factor. Quadratic terms are correlated (VIF 1.73 for three factors). |
| Spherical (α = √k) | sqrt(k) | All factorial and axial points on one sphere. |
| Practical (α = k^¼) | k^(1/4) | A compromise between face-centred and rotatable values. |

For α above 1 the axial points lie outside the factor ranges. The preview notes give the resulting actual ranges ("Axial points extend beyond the factor ranges: Temperature 136.36…203.64, ..."). Check that the equipment and the process allow those settings. Choose **Face-centered** or **Inscribed** when the ranges are hard limits. Discrete factors use the nearest available level.

**Blocks.** With **Factorial / axial**, block 1 holds the factorial points and block 2 the axial points. Centre points are split between the blocks (half rounded up in block 1). With **Rotatable** or **Orthogonal**, α is replaced by the value that makes the blocks orthogonal to the quadratic terms: α = sqrt(F × (2k + c_axial) / (2 × (F + c_factorial))), where c_factorial and c_axial are the centre points of each block. The notes give the value that was replaced. With other axial choices the notes state whether the blocks are orthogonal and give the α that would make them so.

**Run count.** F + 2k + n0, times the number of categorical level combinations. F is 2^k, or 2^(k-1) for k of 5 and more with the fraction.

**Model supported.** Full quadratic (main effects, two-factor interactions, squares).

**Example.** Three factors, default options with 6 centre points: 8 factorial, 6 axial, 6 centre = 20 runs, α = 1.682.

```
 #  Type      A: Temperature  B: Pressure  C: Time
 1  factorial 150             1            10
 ...
 8  factorial 190             3            30
 9  center    170             2            20      (6 centre rows)
...
15  axial     136.36          2            20
16  axial     203.64          2            20
17  axial     170             0.31821      20
18  axial     170             3.6818       20
19  axial     170             2            3.1821
20  axial     170             2            36.818
```

Effect of the axial distance, three factors, 6 centre points, quadratic model, 20 runs each:

| Axial distance | α | D-efficiency | G-efficiency | Average prediction variance |
|---|---|---|---|---|
| Rotatable | 1.682 | 61.58 % | 74.65 % | 0.2135 |
| Orthogonal | 1.525 | 55.05 % | 71.46 % | 0.2198 |
| Face-centered | 1 | 36.67 % | 63.04 % | 0.2953 |
| Spherical | 1.732 | 63.82 % | 75.68 % | 0.2117 |
| Practical | 1.316 | 47.28 % | 67.58 % | 0.2334 |
| Rotatable, inscribed | 1.682 | 12.95 % | 10.37 % | 0.7519 |

The efficiencies are computed in the coded cube from -1 to +1. Designs with α above 1 spread the points over a larger region, which raises the values, but the extra region is not part of the factor ranges. Compare face-centred and inscribed designs with the others with this in mind. With **Blocks** = `Factorial / axial` and the rotatable choice, α becomes 1.809 and the design keeps 20 runs.

### Box-Behnken

A three-level response surface design in which each run varies two or three factors at a time. All factors sit at their centre in the other positions. There are no corner points.

**When to use.** Optimisation of 3 to 7 continuous factors when the corners of the factor space are expensive, dangerous or infeasible, and the experiment must stay within the ranges.

**Availability.** At least 3 numeric factors, no mixture components. Discrete factors must have at least 3 levels. Categorical factors are crossed with the design.

| Option | Meaning | Default |
|---|---|---|
| **Center points** | Centre runs, 0 to 50. Per combination of categorical levels. | 5 for up to 5 factors, 6 for more |

**Run count.** Edge points + centre points. Edge points: 12 for 3 factors, 24 for 4, 40 for 5, 48 for 6, 56 for 7. For 8 or more factors every pair of factors is varied at ±1 (4 × k(k-1)/2 runs). For 6 and 7 factors the factors are varied in groups of 3.

**Model supported.** Full quadratic.

**Example.** Three factors with 5 centre points: 12 edge points + 5 = 17 runs.

```
 #  Type    A: Temperature  B: Pressure  C: Time
 1  edge    150             1            20
 2  edge    190             1            20
 3  edge    150             3            20
 4  edge    190             3            20
 5  edge    150             2            10
 ...
12  edge    170             3            30
13  center  170             2            20   (5 centre rows)
```

Evaluated for the quadratic model: D-efficiency 34.03 %, G-efficiency 43.17 %, A = 2.04, 7 residual df. Prediction at the corners has high variance because no run lies near them. The central composite design with the same factors has better D- and G-efficiency (see [Choosing a design](#choosing-a-design)) but uses points outside the ranges or at the corners.

### Optimal (custom)

A design built by an algorithm to optimise a statistical criterion for a chosen model, number of runs and factor space. It is the most flexible design.

**When to use.**

- The factor space is constrained, irregular, or mixes continuous, discrete, categorical and mixture factors.
- The number of runs is fixed by budget and does not match a classical design.
- The model is not a standard one (custom formula).
- Existing runs must be extended (augmentation).
- Environmental conditions are known in advance, for example a weather forecast or a production schedule.
- A classical design does not fit the factors.

**Availability.** At least one controlled or noise factor. Mixture components are allowed.

**Method.** The program uses coordinate exchange. It starts from random runs and replaces one coordinate of one run at a time with the candidate value that improves the criterion most, then keeps the change. It passes over all runs and coordinates until a pass gives no improvement. This is done from several random starts, and the best result is kept. Continuous factors take the 11 values -1, -0.8, ..., +1 (coded), so the settings of optimal designs for a 150 to 190 °C factor are multiples of 4 °C from 150. Mixture components move together over candidate blends: a lattice, centroids and extreme vertices of the constrained region.

| Option | Meaning | Default |
|---|---|---|
| **Criterion** | `D-optimal (parameter estimation)`, `I-optimal (prediction)` or `A-optimal`. See below. | D |
| **Model** | Linear, Two-factor interactions, Quadratic, Cubic or Custom formula. | Quadratic for up to 6 factors, otherwise Two-factor interactions |
| **Custom formula** | Model formula, used when the model is Custom, for example `(A+B+C)^2 + A^2`. See [models.md](models.md). | empty |
| **Runs (0 = model terms + 6)** | Number of new runs, 0 to 1000. With 0, the count is the number of model coefficients + 6 (less the existing runs when augmenting), or the number of known environmental values. | 0 |
| **Augment existing runs** | Keeps the current runs (except excluded ones) and adds runs that complement them. | off |
| **Random starts** | Number of random starting designs, 1 to 500. More starts give a better design and take longer. | 20 |
| **Seed** | Seed of the random numbers, 0 to 1000000000. | 1 |
| **Environment × factor interactions** | Only with environmental factors. Adds interactions between environmental and designed factors to the model. | off |
| **Known values of** (factor name) | One option per environmental factor. Forecast or scheduled values, one per run, separated by commas, spaces or new lines. | empty |

**Criteria.** M = X'X is the information matrix for the model matrix X, p the number of coefficients and n the number of runs.

| Criterion | Minimised quantity | Use |
|---|---|---|
| D | -log det(M), equal to a maximum det(M) | Precise estimation of all coefficients together. Suited to screening and factor selection. |
| I | Average of x'M^-1 x over the factor region (trace of W M^-1 for the moment matrix W of the region) | Precise prediction across the region. Suited to response surface use and optimisation. The region average is approximated with 4000 random points. |
| A | trace(M^-1) | Small average variance of the coefficient estimates. |

D-optimal designs tend to place runs at the edges and corners of the region. I-optimal designs put more runs in the interior and so give lower average prediction variance, at the price of less precise coefficients.

**Run count.** The model has p coefficients. The design must have at least p runs in total (existing runs included). Fewer runs give an error with the required number. With the default, the design has p + 6 runs, which leaves 6 residual degrees of freedom. A model with Quadratic for k factors has (k + 1)(k + 2)/2 coefficients: 10 for 3 factors, 15 for 4, 21 for 5.

**Augmentation.** With **Augment existing runs**, the runs on the Runs sheet stay unchanged and the program chooses the new runs to complement them. Excluded runs are not counted. Existing runs without values for all model factors are skipped, and the notes give the number. The apply button then reads **Add runs**. The evaluation of the preview includes the existing runs. Use augmentation to add axial points to a factorial, or to extend a screening design with runs that estimate the interactions found to be active.

**Planning around environmental values.** Enter known or forecast values of an environmental factor in **Known values of** (name). Entries are matched to runs in order. With fewer values than runs, the values repeat in order. The design then places the other factors for the best criterion given those values: the environmental value is part of the information matrix and is fixed. The runs of the preview show the values in the environmental column, and they are written to the Runs sheet. Without known values, environmental factors have no terms in the design model. They enter the analysis when measured. With **Environment × factor interactions** on and known values given, terms for the environmental factor and its interactions with the designed factors are included. That model has 16 coefficients in the example below, so at least 16 runs are needed.

**Model supported.** The chosen model, less terms with environmental factors without known values.

**Run table.** The point type is `optimal`, or `augment` for added runs. The notes give the efficiencies:

```
D-optimal design: 12 runs for a model with 10 coefficients.
D-efficiency 44.98%, A = 3.728.
```

**Example 1.** Three factors, quadratic model, 12 runs, three criteria:

| Criterion | D-efficiency | G-efficiency | A (trace) | Average prediction variance |
|---|---|---|---|---|
| D-optimal | 44.98 % | 55.79 % | 3.728 | 0.698 |
| I-optimal | 38.57 % | 42.07 % | 2.958 | 0.496 |
| A-optimal | 41.11 % | 43.89 % | 2.896 | 0.512 |

D-optimal has the best D-efficiency. I-optimal has the lowest average prediction variance. A-optimal has the lowest trace. With the default run count (16) the D-optimal design has D-efficiency 45.85 % and 6 residual df.

**Example 2.** Augmenting a 2^3 full factorial (8 runs) for a quadratic model with 6 new runs:

```
D-optimal design: 6 runs for a model with 10 coefficients.
Augments 8 existing runs.
D-efficiency 46.27%, A = 2.719.

 #  Type     A: Temperature  B: Pressure  C: Time
 1  augment  170             3            20
 2  augment  170             1            20     (and so on, 6 rows)
```

All 14 runs: 4 residual df, G-efficiency 82.89 %.

**Example 3.** Planning around a humidity forecast. Four controlled factors and the environmental factor Inlet air absolute humidity, with the forecast values 6 8 9 11 12 7 10 13 for the humidity. Model: two-factor interactions, **Environment × factor interactions** on, 16 runs (the 8 values repeat):

```
D-optimal design: 16 runs for a model with 16 coefficients.
D-efficiency 54.99%, A = 3.289.
Planned around the known values of Inlet air absolute humidity; the runs use them in the listed order.
 #  Type     A    B   C    D   E (humidity)
 1  optimal  170  10  400  30  6
 2  optimal  130  10  400  15  8
 3  optimal  170  4   600  30  9
 ...
```

The standard errors of the environment terms (E, A*E, ...) are 0.66 to 0.75 σ against 0.26 to 0.28 σ for the others, and the VIF is 1.5 to 2.0, because the humidity values are fixed and cannot be balanced against the other factors. A fit with 16 runs for 16 coefficients has no residual df. Add runs for an error estimate.

**Mixture components.** Efficiency numbers for mixture designs are small (1 % to 5 %) and not comparable with those of other designs, because the mixture coding is not a ±1 cube. Compare mixture designs with each other only.

### Latin hypercube

A space-filling design that places runs evenly through the factor space without an assumed model.

**When to use.** Computer experiments, simulations, or experiments in which the response is expected to be smooth but of unknown form. Not suited to estimating a specific model with few runs.

**Availability.** At least one controlled or noise factor, no mixture components.

| Option | Meaning | Default |
|---|---|---|
| **Runs** | Number of runs, 2 to 5000. | 10 × number of factors |
| **Maximin iterations** | Number of column-swap trials, 0 to 200000. 0 gives a random Latin hypercube. | 2000 |
| **Seed** | Seed of the random numbers, 0 to 1000000000. | 1 |

**Method.** Each continuous factor takes n distinct levels at the middle of n equal strata of its range, so each stratum has exactly one run. The assignment of levels to runs is then changed by swapping two values within a column. A swap is kept when the Φp criterion (the sum of the distances between all pairs of runs raised to the power -15, a smooth stand-in for maximin) decreases. Some swaps that increase it are also kept, with a probability that falls over the iterations (simulated annealing). Large Φp means that some runs are close together. Discrete and categorical factors are balanced: each level occurs about equally often. The distance between two runs counts 1 for each categorical factor with different levels, and the squared coded difference for numeric factors.

**Run count.** The number entered in **Runs**.

**Model supported.** Main effects, or the quadratic model when n is at least 3(k + 1)(k + 2)/2.

**Example.** Two continuous factors, one categorical (Grade: X, Y, Z) and one discrete (Speed: 100, 200, 400), 12 runs, seed 3:

```
Latin hypercube with 12 runs; continuous factors take 12 distinct levels.
Minimum distance between runs: 1.258 (coded units).
 #  Type          A: Temperature  B: Pressure  C: Grade  D: Speed
 1  space-filling 181.67          1.4167       X         100
 2  space-filling 155             1.5833       Z         100
 3  space-filling 151.67          1.9167       X         200
 ...
```

The main-effects model for this design has D-efficiency 53 %. Latin hypercubes are weak for parameter estimation: with three continuous factors and 20 runs the quadratic model has D-efficiency 15.9 % against 61.6 % for a rotatable central composite design.

### Simplex lattice

A mixture design: the blends are placed on a regular lattice in the space of component proportions.

**When to use.** Experiments in which the components add up to a fixed total (formulations, blends) and the response depends on the proportions.

**Availability.** At least 2 mixture components. The lower bounds of the components must sum to less than the mixture total, and the upper bounds to at least the total. Process factors are crossed with the lattice. See [setup-and-data.md](setup-and-data.md) for mixture components and bounds.

| Option | Meaning | Default |
|---|---|---|
| **Lattice degree** | The proportions are multiples of 1/degree, 1 to 6. | 2 |
| **Add overall centroid** | Adds the blend with equal proportions when the lattice does not include it. | on |
| **Add axial check blends** | Adds one blend per component, shifted towards that component. Used to check lack of fit. | off |
| **Replicates** | Repeats the set of blends, 1 to 10. | 1 |

**Run count.** For q components and degree m, C(m + q - 1, q - 1) lattice points, plus the centroid (when added and not already present) and q axial blends (when added), times replicates. When there are process factors, the blends are repeated for each combination of the process-factor levels: continuous factors at -1 and +1 when all process factors are numeric, otherwise every factor at the levels of its two-level grid (categorical factors at all their levels).

**Bounds.** Blends are laid out in pseudo-components when lower bounds exist. A blend that exceeds a component's upper bound is left out, and a note gives the count. When too few blends remain (fewer than the number of components) the program reports an error. Use the optimal design for constrained regions.

**Model supported.** Degree 1: linear blending. Degree 2: quadratic blending (each pair of components has an interaction). Degree 3 or more: cubic blending.

**Example.** Three components with no bounds (0 to 1), degree 2: 3 vertices, 3 edge midpoints and the centroid = 7 runs.

```
 #  Type      A: Oil   B: Water  C: Surfactant
 1  vertex    1        0         0
 2  edge      0.5      0.5       0
 3  edge      0.5      0         0.5
 4  vertex    0        1         0
 5  edge      0        0.5       0.5
 6  vertex    0        0         1
 7  centroid  0.33333  0.33333   0.33333
```

The model has 6 coefficients and the design 1 residual df. The pair interactions have standard errors of 4.58 σ against 1.0 σ for the linear terms, because the interaction columns are small except near the middle of an edge.

### Simplex centroid

A mixture design with all vertices, edge midpoints, face centroids, and the overall centroid.

**When to use.** Mixture experiments with 3 or more components when interactions between 3 components may matter. The design supports the special cubic model.

**Availability.** 2 to 12 mixture components, bounds as for the simplex lattice.

| Option | Meaning | Default |
|---|---|---|
| **Add axial check blends** | Adds one axial blend per component. | off |
| **Replicates** | 1 to 10. | 1 |

**Run count.** 2^q - 1 blends for q components (7 for 3, 15 for 4, 31 for 5), plus q axial blends if added, times replicates (and times the process-factor combinations).

**Model supported.** Special cubic for 3 or more components (pair interactions and the three-component interaction). Quadratic blending for 2.

**Example.** Three components: 3 vertices, 3 edges, 1 centroid = 7 runs. The design supports 7 coefficients and has 0 residual df: the fit is exact and has no error estimate. Replicates or axial blends add residual df. With axial blends and the degree-3 lattice (13 runs), residual df is 6.

### Taguchi (robust parameter)

An inner orthogonal array for the controlled factors, crossed with an outer design for the noise factors. Each inner run is made under every outer condition.

**When to use.** Robust design. The goal is settings of the controlled factors for which the response is on target and varies little under noise factors (material lots, ambient conditions, operators).

**Availability.** At least one controlled factor, no mixture components. Noise factors are optional. Without noise factors, the program produces the inner array alone, and the notes advise replicating the runs to estimate variability.

| Option | Meaning | Default |
|---|---|---|
| **Levels per continuous factor** | 2 or 3 levels for continuous factors. | 2 |
| **Inner array** | `Smallest that fits` or a named array. | Smallest that fits |
| **Outer design** | `Full factorial when small, else array`, `Full factorial` or `Orthogonal array`. | Full factorial when small, else array |

**Arrays.** An orthogonal array has columns with a fixed number of levels. A factor with s levels requires a column with s levels.

| Array | Runs | Columns |
|---|---|---|
| L4 | 4 | 3 × 2 levels |
| L8 | 8 | 7 × 2 |
| L9 | 9 | 4 × 3 |
| L12 | 12 | 11 × 2 |
| L16 | 16 | 15 × 2 |
| L16 (4-level) | 16 | 5 × 4 |
| L18 | 18 | 1 × 2, 7 × 3 |
| L25 | 25 | 6 × 5 |
| L27 | 27 | 13 × 3 |
| L32 | 32 | 31 × 2 |

With `Smallest that fits`, the program takes the first array in the order of the table that holds all factors, and prefers arrays without dummy levels. A two-level factor may use a three-level column. Its first level is then repeated as a dummy level, and a note in the preview states this.

**Outer design.** With the automatic choice, the outer design is a full factorial when it has at most 8 conditions, otherwise the smallest orthogonal array that fits.

**Run count.** (inner array runs) × (outer conditions). The limit is 100000 runs. The point type of each run names the inner and outer rows, for example `I3·O2`.

**Model supported.** Main effects. When there are noise factors and enough runs (more than twice the number of coefficients of a model with main effects and control × noise interactions), the control × noise interactions are added.

**Analysis.** Use the signal-to-noise analysis (see [analysis.md](analysis.md)) or model the control × noise interactions.

**Example.** Four controlled factors (melt temperature, hold pressure, cooling time, mould temperature), two noise factors (resin lot with 2 levels, conditioning humidity with 2 levels). The program chooses the L8 array and a full outer factorial:

```
Inner array L8 (8 runs) for 4 controlled factors.
Outer design: full factorial in 2 noise factors (4 conditions).
 #   Type   A: Melt temperature  B: Hold pressure  C: Cooling time  D: Mould temperature  E: Resin lot  F: Conditioning humidity
 1   I1·O1  230                  400               8                50                    Lot 4471     35
 2   I1·O2  230                  400               8                50                    Lot 4471     65
 3   I1·O3  230                  400               8                50                    Lot 4518     35
 4   I1·O4  230                  400               8                50                    Lot 4518     65
 5   I2·O1  270                  400               8                90                    Lot 4471     35
 ...
```

8 × 4 = 32 runs. The model has main effects and the 8 control × noise interactions (14 terms plus the intercept, 17 residual df). D-efficiency 100 %.

## Choosing a design

| Situation | Design |
|---|---|
| 2 to 4 factors, interactions matter | Full factorial |
| 5 to 15 two-level factors, find the active ones | Fractional factorial, resolution IV (main effects clear of interactions) |
| Many factors, few runs, main effects only | Plackett-Burman |
| 6 or more continuous factors, curvature possible | Definitive screening |
| 2 to 6 continuous factors, fit a quadratic model, extension beyond the ranges allowed | Central composite, rotatable |
| Same, ranges are hard limits | Central composite, face-centred or inscribed, or Box-Behnken |
| 3 to 7 factors, corners infeasible | Box-Behnken |
| Constraints, mixed factor types, odd run budget, special model | Optimal |
| Existing data, add runs | Optimal with **Augment existing runs** |
| Environmental conditions known in advance | Optimal with **Known values of** |
| Prediction over the region matters more than coefficients | Optimal, I criterion |
| Simulation or unknown response form | Latin hypercube |
| Components of a blend, fixed total | Simplex lattice or centroid; optimal when bounds constrain the region |
| Response must be insensitive to noise | Taguchi |

Guidelines:

- **Fix the model first.** A design is good for a model. Select the model (main effects, interactions, quadratic) from the questions the experiment must answer, then compare the designs on the Evaluate page with that model.
- **Sequential strategy.** Screen with a fractional factorial, Plackett-Burman or definitive screening design. Follow with a response surface design on the factors found active. Fold over or augment a screening design when an ambiguity remains.
- **Budget.** The number of runs must exceed the number of model coefficients. Reserve at least 4 to 6 residual df and centre points for an error estimate. Power is low below that (see the power column on the Evaluate page).
- **Resolution.** Resolution III is enough for screening when interactions are small. Use IV or V when interactions are likely.
- **Compare numerically.** For three factors and a quadratic model, with the quadratic preset on the Evaluate page:

| Design | Runs | Residual df | D-eff. | G-eff. | Average prediction variance | Largest VIF |
|---|---|---|---|---|---|---|
| Central composite, rotatable (6 centre) | 20 | 10 | 61.58 % | 74.65 % | 0.2135 | 1.02 |
| Central composite, face-centred (6 centre) | 20 | 10 | 36.67 % | 63.04 % | 0.2953 | 1.82 |
| Box-Behnken (5 centre) | 17 | 7 | 34.03 % | 43.17 % | 0.3415 | 1.01 |
| D-optimal (default runs) | 16 | 6 | 45.85 % | 72.79 % | 0.4836 | 1.11 |
| I-optimal (16 runs) | 16 | 6 | 43.00 % | 78.55 % | 0.3397 | 1.42 |
| D-optimal, 20 runs | 20 | 10 | 46.40 % | 82.57 % | 0.4085 | 1.04 |
| Full factorial, 3 levels | 27 | 17 | 44.21 % | 72.73 % | 0.2201 | 1.0 |
| Latin hypercube, 20 runs | 20 | 10 | 15.91 % | 11.25 % | 0.6055 | 1.33 |

The central composite design has the best precision per run, and the optimal designs are close with 16 runs. The Latin hypercube is not suited to this question.

## Evaluate page

The Evaluate page measures how well a set of runs supports a model. It evaluates the runs before they are made (**Design preview**) or the current runs of the project (**Current runs**). The page updates when the project changes, and when a control changes.

### Controls

| Control | Meaning | Default |
|---|---|---|
| Source (**Current runs** / **Design preview**) | The runs to evaluate: those on the Runs sheet without excluded ones, or the previewed design. A preview that augments existing runs is evaluated together with them. | Current runs |
| Model (**Default model**, **Linear**, **Two-factor interactions**, **Quadratic**, **Cubic**, **Custom formula**) | The model terms. **Default model**: for a preview, the model the design supports. For current runs, the model of the last applied design with measured environmental and block factors added as main effects; without a design, quadratic when the runs allow it (at least 4 more runs than terms), otherwise two-factor interactions or linear. | Default model |
| **Model formula** | A formula, shown with **Custom formula**. Press Enter to evaluate. See [models.md](models.md). | empty |
| **Environment interactions** | Shown for the two-factor-interaction, quadratic and cubic presets. Adds interactions between environmental and designed factors. | off |
| **Effect / σ** | The size of the effect to detect, in units of the error standard deviation σ, for the power column. | 2 |
| **Evaluate** | Runs the evaluation. | |

Runs that lack a value for a factor in the model are not counted, and a warning gives the number. If no run has all values, an error is shown. A model with environmental factors cannot be evaluated on current runs before those factors are measured: the program reports an error.

### Layout

- The model formula in text form.
- **Statistics** (list below).
- Warnings, marked with ⚠.
- A table with one row per model coefficient: **Term**, **Std. error / σ**, **VIF**, **Power**. Rows with power below 80 % are shown in a muted colour.
- A heat map **Correlation of model terms (|r|)**.
- A plot **Fraction of design space**.

### Statistics

Notation: X is the model matrix (n runs, p columns, coded units), M = X'X, and σ² the error variance. x denotes the model row of a point in the factor space.

**Runs (n).** The number of runs used.

**Model coefficients (p).** The number of columns of X, including the intercept.

**Residual df.** n minus the rank of X. Residual degrees of freedom are what remains to estimate σ. With 0, the design fits the model exactly, error cannot be estimated, tests are impossible and the power column stays empty. A warning appears. Aim for at least 4 to 6.

**D-efficiency.** 100 × det(M)^(1/p) / n.

- It measures the information on all coefficients together, per run.
- 100 % is reached by an orthogonal design with all columns at ±1 (a two-level factorial for a main-effects or interaction model).
- Models with squared terms cannot reach 100 %. Values of 40 % to 70 % are normal for second-order designs.
- It is 0 when the model has terms that cannot be estimated (aliased terms).
- Use it to compare designs for the same model and factor region. Values from different models or different regions are not comparable. For mixture factors, values are small because of the coding.

**G-efficiency.** 100 × p / (n × max over the region of x'M^-1 x).

- It relates the worst-case prediction variance to that of the ideal case, a design with a maximum scaled prediction variance of p.
- 100 % is the upper limit: the maximum of n x'M^-1 x over the region cannot be below p.
- Values of 50 % or more mean no part of the region is poorly predicted. The maximum is found by sampling and a local search.
- Blank when the model is not fully estimable.

**A (trace of (X'X)⁻¹).** The sum of the coefficient variances in units of σ². Smaller is better. It falls when runs are added. It is also the criterion minimised by an A-optimal design.

**Average prediction variance.** The mean of x'M^-1 x over the factor region, estimated from 2000 random points. It is the variance of a predicted mean response in units of σ². For example, 0.2 means that the standard error of the prediction is sqrt(0.2) σ = 0.45 σ on average. Smaller is better. It is the quantity minimised by an I-optimal design. Blank when the model is not fully estimable.

### Table columns

**Std. error / σ.** The standard error of the coefficient in units of σ: sqrt of the corresponding diagonal element of M^-1. For a coded coefficient this is the standard error of half the effect (the change in response when the factor goes from 0 to +1). For a two-level orthogonal design with n runs it is 1/sqrt(n). Smaller is better. With the standard deviation of the experiment (σ) known, multiply the value by σ.

**VIF** (variance inflation factor). VIF = 1 / (1 - R²), where R² is the coefficient of determination of the regression of this column on all other non-intercept columns of the model (centred). It is 1 for an orthogonal column and larger when the column is correlated with the others. The variance of the coefficient is multiplied by the VIF compared with an orthogonal design.

| VIF | Reading |
|---|---|
| 1 | Orthogonal |
| up to 2 | Small correlation, acceptable |
| 2 to 5 | Moderate. Estimates are correlated. |
| above 5 to 10 | Strong. Coefficients are imprecise and confounded. |

The intercept has no VIF. A blank VIF means that the column could not be computed.

**Power.** The probability of detecting a coefficient of a given size with an F-test at significance level 0.05. The size is that of an effect of **Effect / σ** error standard deviations across the coded range, so the coefficient is (Effect / σ) / 2 in units of σ. The non-centrality of the F distribution is λ = β² / (M^-1)ii with β = (Effect / σ) / 2, with 1 and residual-df degrees of freedom. A power of 80 % or more is the usual target, and rows below 80 % are muted. Power increases with the number of runs, with the effect size, and with the residual df. It is blank when there are no residual df.

For the example of four factors in a 2^(4-1) fraction (8 runs, 3 residual df) with Effect / σ = 2, power is 49 %. The 18-run full factorial with 2 centre points (7 residual df) reaches 92.7 % for each coefficient of the two-factor-interaction model. Set **Effect / σ** to the smallest effect that matters, in units of the run-to-run standard deviation, which comes from earlier experiments or repeated runs.

### Correlation map

The heat map shows |r|, the absolute value of the correlation between pairs of model columns (without the intercept), including columns that cannot be estimated.

- 0 (light): the columns are orthogonal. The effects can be estimated independently.
- 1 (dark): the columns are identical or opposite. The effects are fully confounded (aliased).
- Values between 0.3 and 0.7 mean partial aliasing.

The map is the way to see the alias structure of any design, including non-regular designs like Plackett-Burman 12 runs. For regular fractions, pairs with |r| = 1 are the alias pairs listed on the Design page.

Warnings report pairs with |r| above 0.9, and pairs with |r| above 0.5 when one of the two terms involves an environmental factor ("the environmental effect is partly confounded with the factor effect"). A warning **Not estimable (aliased)** lists the terms that the runs cannot separate from other terms. These terms are removed from the table and D-efficiency is 0.

**Example.** The five-factor 2^(5-2) fraction (8 runs, resolution III) evaluated for the two-factor-interaction model has 16 coefficients and 8 runs:

```
Not estimable (aliased): A*B, A*C, A*D, A*E, B*D, C*D, C*E, D*E.
A and B*D are correlated (r = 1).
A and C*E are correlated (r = 1).
...
```

For the linear model the same runs have D-efficiency 100 %, 2 residual df, all VIF 1.00. Power at Effect / σ = 2 is 35.7 %, at Effect / σ = 1 it is 13.8 %.

### Fraction of design space plot

The plot shows the scaled prediction variance (SPV) n x'M^-1 x (vertical axis) of random points in the factor region sorted from low to high (horizontal axis, 0 to 1). A point (f, v) means that a fraction f of the region has SPV at most v.

- A flat curve means uniform prediction quality.
- A curve that rises steeply at the right means a small part of the region (usually the corners) is predicted poorly.
- A low curve is better. Compare designs with the same n, since the SPV is scaled by n.
- The maximum of the SPV is the quantity in G-efficiency: G = 100 × p / maximum. The mean of the curve divided by n is the average prediction variance.

### Reading the page

1. Check the residual df. Zero or near zero means the design cannot support the model with an error estimate.
2. Check the warnings and the correlation map for aliased terms that matter.
3. Check the VIF column: values above 5 for terms of interest are a problem.
4. Check the power column at the smallest effect of interest.
5. Compare D- and G-efficiency and the average prediction variance with alternative designs for the same model.

## Adding single runs

The **Next run…** button on the Runs page opens a dialog that suggests one run at a time. It takes the current values of the environmental factors, and **Suggest** gives settings for the controlled factors.

- **Most informative**: the settings with the largest prediction variance x'(X'X)^-1 x under the current runs and model, found by random starts and a local search. The run adds the most information on the model.
- **Best predicted**: the settings with the best desirability for the current environment (see [optimisation.md](optimisation.md)).

**Add as planned run** adds the run, with point type `sequential` or `best`.
