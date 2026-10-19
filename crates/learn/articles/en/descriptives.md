# Descriptives

Descriptives summarizes each Scale variable in one row: how many cases have a valid value, the smallest and the largest, the mean and the standard deviation. It is the first look at a variable before any test.

## When to use it

Use it for variables measured on a scale, where the distance between values means something: age, income, a test score. For a Nominal or Ordinal variable a mean has no meaning; count its values with Frequencies instead.

## The table

- **N**: the cases with a valid value. A value that is missing (system-missing, or one of the variable's user-missing values) is left out of that variable only.
- **Minimum** and **Maximum**: the smallest and the largest valid value. Look at them first: an impossible value (an age of 999) is often a missing-value code that wasn't declared.
- **Mean**: the sum of the values divided by N.
- **Std. Deviation**: how far the values typically are from the mean, in the variable's own unit.
- **Valid N (listwise)**: the cases valid on every variable of the table at once.

## The formulas

$$\bar{x} = \frac{\sum_{i=1}^{n} x_{i}}{n}$$

$$s = \sqrt{\frac{\sum \left( x_{i} - \bar{x} \right)^{2}}{n - 1}}$$

The squared deviations are divided by $n - 1$, not $n$: the deviations are measured from the sample's own mean, which sits closer to the data than the population's mean would, and dividing by $n - 1$ corrects that, so the variance estimates the population's without bias. With weights, every sum counts each case as many times as its weight.

## Reading it

The mean and the standard deviation describe a variable well when its values are roughly symmetric. When a few values are far from the rest (incomes, often), the mean follows them: compare it with the median, in Explore, and look at a histogram. Beside the table, the Σ shows every sum of this calculation on your data.

## Try it

```syntax
DESCRIPTIVES VARIABLES=age income score.
```

[Open Descriptives](dialog:descriptives). Next: [which test to use](which-test) to go further.
