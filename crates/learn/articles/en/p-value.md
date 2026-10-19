# The p-value

A p-value is the probability of a result at least as extreme as the one observed, if the null hypothesis were true: if, say, the two groups had the same mean in the population and only chance made them differ in the sample.

## How to read it

- A **small** p-value (0.05 or less, by the usual convention) means the data would be unusual if there were no effect: the result is called significant, and the null hypothesis is rejected at that level.
- A **large** p-value means the data are compatible with no effect. It doesn't show there is none: the sample may just be too small to see it.

The level, called $\alpha$, is chosen before looking: 0.05 is a convention, not a law. MK Sigma's tables show the p-value as **Sig.**, and the Σ shows where it comes from: the test statistic and the distribution it is compared with.

## What it is not

- It is **not** the probability that the null hypothesis is true, nor that the result is due to chance.
- It says **nothing about the size** of an effect: with thousands of cases a negligible difference is significant; report the effect and its confidence interval.
- $p = 0.049$ and $p = 0.051$ are practically the same evidence; don't read the threshold as a cliff.

## One-tailed and two-tailed

A two-tailed p-value counts extreme results in both directions; a one-tailed one only in the direction stated before seeing the data, and is half the two-tailed one. Use one-tailed only when the other direction could not matter.

## Many tests

Each test at 0.05 has a 5% chance of a false alarm; twenty tests on unrelated variables will usually give one. When many comparisons are made, correct for it, as the Bonferroni method of One-Way ANOVA does, and say how many tests were run.

## Confidence intervals

A 95% confidence interval and a test at 0.05 agree: the test is significant exactly when the interval leaves out the value of no effect (0 for a difference). The interval says more: how large the effect plausibly is.

## Try it

Does the mean score of the sample differ from 70?

```syntax
T-TEST /TESTVAL=70 /VARIABLES=score.
```

Read Sig. (2-tailed), then the interval of the difference. See also the [t test](t-test).
