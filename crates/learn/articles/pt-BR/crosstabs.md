# Tabulação cruzada e o teste qui-quadrado

Uma tabulação cruzada conta os casos em cada combinação de duas variáveis categóricas. O teste qui-quadrado dela pergunta se as variáveis estão relacionadas: se a distribuição de uma muda com a outra mais do que o acaso faria.

## Quando usar

Duas variáveis **Nominais** ou **Ordinais**, cada caso contado uma vez. Com uma variável de Escala, agrupe-a antes (RECODE) ou use outro procedimento: veja [qual teste usar](which-test).

## As tabelas

- **Resumo do processamento de casos**: os casos válidos nas duas variáveis e os que ficaram de fora.
- **A tabulação cruzada**: as contagens e, se pedidas, as contagens esperadas e as porcentagens na linha, na coluna ou no total. Compare as porcentagens dentro dos grupos que você está comparando: são elas que mostram uma relação.
- **Testes qui-quadrado**: o qui-quadrado de Pearson, os graus de liberdade $\left( r - 1 \right) \left( c - 1 \right)$ para $r$ linhas e $c$ colunas, e a significância. Numa tabela 2 × 2, também a correção de continuidade de Yates e o teste exato de Fisher.

## As fórmulas

A contagem esperada de cada célula, se as variáveis fossem independentes, vem dos totais da sua linha e da sua coluna:

$$E = \frac{R \cdot C}{N}$$

$$\chi^{2} = \sum \frac{\left( O - E \right)^{2}}{E}$$

Quanto mais as contagens observadas $O$ se afastam das esperadas, maior o qui-quadrado e menor o seu [p-valor](p-value).

## A condição dele

O teste qui-quadrado é aproximado. Ele vale quando as contagens esperadas não são pequenas demais: nenhuma célula abaixo de 1, e no máximo 20% delas abaixo de 5 (a nota sob a tabela diz quantas são). Caso contrário, numa tabela 2 × 2 leia o **Teste exato de Fisher**, que não precisa de aproximação; numa maior, junte as categorias com poucos casos.

## O que ele não diz

Um qui-quadrado significativo diz que as variáveis estão relacionadas, não com que força nem por quê. Olhe as porcentagens para ver a direção; uma amostra grande torna significativas relações minúsculas. O $V$ de Cramér, $V = \sqrt{\chi^{2} / \left( N \left( k - 1 \right) \right)}$, com $k$ o menor entre o número de linhas e de colunas, mede a força de 0 a 1.

## Experimente

As variáveis da amostra são de Escala; o exemplo cria duas categóricas a partir delas.

```syntax
COMPUTE older = age > 35.
COMPUTE high = score > 70.
CROSSTABS older BY high /CELLS=COUNT EXPECTED ROW /STATISTICS=CHISQ.
```

[Abrir Tabulação cruzada](dialog:crosstabs).
