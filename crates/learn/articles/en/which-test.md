# Which test?

Start from your question and from the kind of variables you have: their level of measurement (Scale, Ordinal or Nominal, in Variable View) decides most of the choice. Each answer below names the procedure and opens its dialog.

## Describe one variable

- A **Scale** variable (age, income, a score): its mean, spread, minimum and maximum, with [Descriptives](descriptives). [Open Descriptives](dialog:descriptives).
- A **Nominal** or **Ordinal** variable (a group, an answer on a scale of agreement): how many cases have each value, with Frequencies. [Open Frequencies](dialog:frequencies).

## Compare groups on a Scale variable

- **Two groups** of different people (men and women, treated and not): the [independent-samples t test](t-test). [Open the t test](dialog:ttest). If the values are far from normal and the groups are small, or the variable is only Ordinal, Mann-Whitney's test compares their ranks instead. [Open Nonparametric Tests](dialog:nonparametric).
- **Three groups or more**: one-way ANOVA, or Kruskal-Wallis's test on ranks. [Open One-Way ANOVA](dialog:oneway).
- **The same people measured twice** (before and after): the paired t test, or Wilcoxon's signed ranks.

## Relate two variables

- Two **categorical** variables (Nominal or Ordinal): a [crosstabulation with its chi-square test](crosstabs). [Open Crosstabs](dialog:crosstabs).
- Two **Scale** variables: Pearson's correlation, from −1 to 1. [Open Correlate](dialog:correlations).

## Predict a variable from others

- A **Scale** outcome from one or more predictors: linear regression. [Open Regression](dialog:regression).
- An outcome with **two values** (yes or no): binary logistic regression.

> Every test answers whether a difference or a relation could be chance; read its [p-value](p-value) together with the size of the effect, never alone.
