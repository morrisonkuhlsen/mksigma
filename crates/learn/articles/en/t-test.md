# Independent-samples t test

The t test compares the means of a Scale variable in two groups of different cases, and asks whether the difference between them is larger than chance alone would usually give.

## When to use it

- One **Scale** variable (the test variable) and one variable that splits the cases into **two groups** (the grouping variable).
- The cases are independent: each one belongs to one group only, and one case's value doesn't depend on another's.
- In each group the values are roughly normal, or the groups are large (from about 30 cases each the test holds well anyway). For small, skewed groups, Mann-Whitney's test on ranks is safer.

## The tables

**Group Statistics** gives each group's N, mean, standard deviation and the standard error of its mean.

**Independent Samples Test** has two rows. **Levene's test** decides which one to read: it tests whether the two groups' variances differ.

- Levene's Sig. above 0.05: no reason to doubt equal variances; read **Equal variances assumed**, the pooled test.
- Levene's Sig. of 0.05 or below: read **Equal variances not assumed**, Welch's test, whose degrees of freedom are usually not a whole number.

Then **t** is the difference of the means in standard errors, **Sig. (2-tailed)** its [p-value](p-value), and the interval is where the difference of the population means plausibly lies: when it doesn't include 0, the test is significant at that level.

## The formulas

$$t = \frac{\bar{x}_{1} - \bar{x}_{2}}{SE}$$

With equal variances, one variance pooled from both groups and $n_{1} + n_{2} - 2$ degrees of freedom:

$$s_{p}^{2} = \frac{\left( n_{1} - 1 \right) s_{1}^{2} + \left( n_{2} - 1 \right) s_{2}^{2}}{n_{1} + n_{2} - 2}$$

$$SE = \sqrt{s_{p}^{2} \left( \frac{1}{n_{1}} + \frac{1}{n_{2}} \right)}$$

Without that assumption, $SE = \sqrt{s_{1}^{2} / n_{1} + s_{2}^{2} / n_{2}}$ and Welch–Satterthwaite's degrees of freedom. The Σ beside the table shows both on your data, with Levene's test.

## The size of the difference

A significant difference can be small, and a large one can miss significance in a small sample. Report the difference itself, with its interval, and a standardized size such as Cohen's $d = \left( \bar{x}_{1} - \bar{x}_{2} \right) / s_{p}$: about 0.2 is small, 0.5 medium, 0.8 large.

## Try it

The sample has no group variable, so the example makes one first: whether a case is older than 35.

```syntax
COMPUTE older = age > 35.
T-TEST GROUPS=older(0 1) /VARIABLES=score.
```

[Open the t test](dialog:ttest). The same command compares a mean with a fixed value (`/TESTVAL=`) or two measurements of the same cases (`PAIRS=`).
