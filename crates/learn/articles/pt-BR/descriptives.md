# Descritivas

As Descritivas resumem cada variável de Escala numa linha: quantos casos têm valor válido, o menor e o maior valor, a média e o desvio padrão. É o primeiro olhar sobre uma variável, antes de qualquer teste.

## Quando usar

Use em variáveis medidas numa escala, em que a distância entre os valores tem sentido: idade, renda, uma nota de teste. Numa variável Nominal ou Ordinal a média não tem sentido; conte os valores dela com as Frequências.

## A tabela

- **N**: os casos com valor válido. Um valor ausente (ausente do sistema, ou um dos valores ausentes definidos pelo usuário) fica de fora só daquela variável.
- **Mínimo** e **Máximo**: o menor e o maior valor válido. Olhe para eles primeiro: um valor impossível (uma idade de 999) costuma ser um código de ausente que não foi declarado.
- **Média**: a soma dos valores dividida por N.
- **Desvio padrão**: o quanto os valores costumam se afastar da média, na unidade da própria variável.
- **N válido (de lista)**: os casos válidos em todas as variáveis da tabela ao mesmo tempo.

## As fórmulas

$$\bar{x} = \frac{\sum_{i=1}^{n} x_{i}}{n}$$

$$s = \sqrt{\frac{\sum \left( x_{i} - \bar{x} \right)^{2}}{n - 1}}$$

Os desvios ao quadrado são divididos por $n - 1$, e não por $n$: eles são medidos a partir da média da própria amostra, que fica mais perto dos dados do que a média da população ficaria, e dividir por $n - 1$ corrige isso, de modo que a variância estima a da população sem viés. Com pesos, cada soma conta cada caso tantas vezes quanto o seu peso.

## Como ler

A média e o desvio padrão descrevem bem uma variável quando os valores são mais ou menos simétricos. Quando alguns valores ficam longe dos outros (como costuma acontecer com renda), a média vai atrás deles: compare-a com a mediana, no Explorar, e olhe um histograma. Ao lado da tabela, o Σ mostra cada soma deste cálculo nos seus dados.

## Experimente

```syntax
DESCRIPTIVES VARIABLES=age income score.
```

[Abrir Descritivas](dialog:descriptives). Para ir além: [qual teste usar](which-test).
