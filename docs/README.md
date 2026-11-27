# sdoe user documentation

These pages describe the features of the sdoe application: every option, what
each statistical quantity means, and worked examples. For building the program
see the [README](../README.md); for the engine's JSON interface see
[API.md](../API.md).

In the application, **Help > Documentation** (F1) opens these pages. The
search field finds the sections that contain every word of the query; words
match at the start of a word, regardless of case. Enter opens the first
result, the arrow keys move through the results, and the Back and Forward keys
of the platform (Alt+Left and Alt+Right on Linux and Windows) go back and
forward.

| Page | Contents |
|---|---|
| [Setup and data](setup-and-data.md) | Projects, factors and their roles, conditional factors, responses, data entry, import and export, reports, R scripts |
| [Designs](designs.md) | Every design type and its options, design evaluation, choosing a design |
| [Models](models.md) | Formula language, coding, transforms, model selection, equations |
| [Analysis](analysis.md) | ANOVA, fit statistics, coefficients, diagnostics, screening, canonical analysis, plots, signal-to-noise |
| [Optimisation](optimisation.md) | Desirability, environmental and robust optimisation, adjustment law, next run, confirmation runs |

## Workflow

A typical study passes through the pages of the application in order:

1. **Setup**: define the factors and responses.
2. **Design**: generate the runs.
3. **Runs**: carry out the experiment and enter the measured responses.
4. **Analysis**: fit and refine a model for each response.
5. **Optimize**: find the factor settings that best meet the goals for all
   responses, then confirm them with new runs.

Data from earlier experiments or from production records can enter at step 3
through CSV import, without a generated design.
