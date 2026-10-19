# Crosstabs and the chi-square test

A crosstabulation counts the cases in each combination of two categorical variables. Its chi-square test asks whether the variables are related: whether the distribution of one changes with the other more than chance would make it.

## When to use it

Two **Nominal** or **Ordinal** variables, each case counted once. With a Scale variable, group it first (RECODE) or use another procedure: see [which test to use](which-test).

## The tables

- **Case Processing Summary**: the cases valid on both variables, and those left out.
- **The crosstabulation**: the counts, and on request the expected counts and the percentages within rows, columns or the total. Compare the percentages within the groups you are comparing: they are what shows a relation.
- **Chi-Square Tests**: Pearson's chi-square, its degrees of freedom $\left( r - 1 \right) \left( c - 1 \right)$ for $r$ rows and $c$ columns, and its significance. For a 2 × 2 table also Yates's continuity correction and Fisher's exact test.

## The formulas

Each cell's expected count, if the variables were independent, comes from its row's and its column's totals:

$$E = \frac{R \cdot C}{N}$$

$$\chi^{2} = \sum \frac{\left( O - E \right)^{2}}{E}$$

The farther the observed counts $O$ from the expected ones, the larger chi-square, and the smaller its [p-value](p-value).

## Its condition

The chi-square test is approximate. It holds when the expected counts are not too small: no cell below 1, and at most 20% of them below 5 (the note under the table says how many there are). Otherwise, in a 2 × 2 table read **Fisher's Exact Test**, which needs no approximation; in a larger one, merge sparse categories.

## What it doesn't say

A significant chi-square says the variables are related, not how strongly nor why. Look at the percentages for the direction; a large sample makes tiny relations significant. Cramér's $V = \sqrt{\chi^{2} / \left( N \left( k - 1 \right) \right)}$, with $k$ the smaller of rows and columns, measures the strength from 0 to 1.

## Try it

The sample's variables are Scale; the example makes two categorical ones from them.

```syntax
COMPUTE older = age > 35.
COMPUTE high = score > 70.
CROSSTABS older BY high /CELLS=COUNT EXPECTED ROW /STATISTICS=CHISQ.
```

[Open Crosstabs](dialog:crosstabs).
