# Teste t para amostras independentes

O teste t compara as médias de uma variável de Escala em dois grupos de casos diferentes e pergunta se a diferença entre elas é maior do que o acaso costuma produzir.

## Quando usar

- Uma variável de **Escala** (a variável de teste) e uma variável que divide os casos em **dois grupos** (a variável de agrupamento).
- Os casos são independentes: cada um pertence a um só grupo, e o valor de um caso não depende do de outro.
- Em cada grupo os valores são mais ou menos normais, ou os grupos são grandes (a partir de uns 30 casos cada, o teste funciona bem mesmo assim). Para grupos pequenos e assimétricos, o teste de Mann-Whitney, sobre os postos, é mais seguro.

## As tabelas

As **Estatísticas de grupo** dão o N, a média, o desvio padrão e o erro padrão da média de cada grupo.

O **Teste de amostras independentes** tem duas linhas. O **teste de Levene** decide qual ler: ele testa se as variâncias dos dois grupos diferem.

- Sig. de Levene acima de 0,05: não há motivo para duvidar da igualdade das variâncias; leia **Variâncias iguais assumidas**, o teste combinado.
- Sig. de Levene de 0,05 ou menos: leia **Variâncias iguais não assumidas**, o teste de Welch, cujos graus de liberdade em geral não são inteiros.

Depois, **t** é a diferença das médias em erros padrão, **Sig. (bilateral)** é o seu [p-valor](p-value), e o intervalo é onde a diferença entre as médias da população provavelmente está: quando ele não inclui o 0, o teste é significativo naquele nível.

## As fórmulas

$$t = \frac{\bar{x}_{1} - \bar{x}_{2}}{SE}$$

Com variâncias iguais, uma variância combinada dos dois grupos e $n_{1} + n_{2} - 2$ graus de liberdade:

$$s_{p}^{2} = \frac{\left( n_{1} - 1 \right) s_{1}^{2} + \left( n_{2} - 1 \right) s_{2}^{2}}{n_{1} + n_{2} - 2}$$

$$SE = \sqrt{s_{p}^{2} \left( \frac{1}{n_{1}} + \frac{1}{n_{2}} \right)}$$

Sem essa suposição, $SE = \sqrt{s_{1}^{2} / n_{1} + s_{2}^{2} / n_{2}}$ e os graus de liberdade de Welch–Satterthwaite. O Σ ao lado da tabela mostra os dois nos seus dados, com o teste de Levene.

## O tamanho da diferença

Uma diferença significativa pode ser pequena, e uma grande pode não ser significativa numa amostra pequena. Relate a própria diferença, com o seu intervalo, e um tamanho padronizado como o $d$ de Cohen, $d = \left( \bar{x}_{1} - \bar{x}_{2} \right) / s_{p}$: cerca de 0,2 é pequeno, 0,5 médio, 0,8 grande.

## Experimente

A amostra não tem variável de grupo, então o exemplo cria uma antes: se o caso tem mais de 35 anos.

```syntax
COMPUTE older = age > 35.
T-TEST GROUPS=older(0 1) /VARIABLES=score.
```

[Abrir o teste t](dialog:ttest). O mesmo comando compara uma média com um valor fixo (`/TESTVAL=`) ou duas medidas dos mesmos casos (`PAIRS=`).
