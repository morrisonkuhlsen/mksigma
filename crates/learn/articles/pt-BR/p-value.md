# O p-valor

Um p-valor é a probabilidade de um resultado pelo menos tão extremo quanto o observado, se a hipótese nula fosse verdadeira: se, digamos, os dois grupos tivessem a mesma média na população e só o acaso os fizesse diferir na amostra.

## Como ler

- Um p-valor **pequeno** (0,05 ou menos, pela convenção usual) quer dizer que os dados seriam incomuns se não houvesse efeito: o resultado é chamado de significativo, e a hipótese nula é rejeitada naquele nível.
- Um p-valor **grande** quer dizer que os dados são compatíveis com a ausência de efeito. Ele não mostra que o efeito não existe: a amostra pode ser pequena demais para vê-lo.

O nível, chamado $\alpha$, é escolhido antes de olhar os dados: 0,05 é uma convenção, não uma lei. As tabelas do MK Sigma mostram o p-valor como **Sig.**, e o Σ mostra de onde ele vem: a estatística do teste e a distribuição com que ela é comparada.

## O que ele não é

- **Não** é a probabilidade de a hipótese nula ser verdadeira, nem de o resultado ser obra do acaso.
- **Não diz nada sobre o tamanho** de um efeito: com milhares de casos, uma diferença desprezível é significativa; relate o efeito e o seu intervalo de confiança.
- $p = 0.049$ e $p = 0.051$ são praticamente a mesma evidência; não leia o limite como um precipício.

## Unilateral e bilateral

Um p-valor bilateral conta os resultados extremos nas duas direções; um unilateral, só na direção afirmada antes de ver os dados, e vale metade do bilateral. Use o unilateral só quando a outra direção não puder ter importância.

## Muitos testes

Cada teste a 0,05 tem 5% de chance de um alarme falso; vinte testes com variáveis sem relação costumam dar um. Quando se fazem muitas comparações, corrija para isso, como faz o método de Bonferroni da ANOVA de um fator, e diga quantos testes foram feitos.

## Intervalos de confiança

Um intervalo de confiança de 95% e um teste a 0,05 concordam: o teste é significativo exatamente quando o intervalo deixa de fora o valor de nenhum efeito (0, para uma diferença). O intervalo diz mais: de que tamanho o efeito provavelmente é.

## Experimente

A nota média da amostra difere de 70?

```syntax
T-TEST /TESTVAL=70 /VARIABLES=score.
```

Leia a Sig. (bilateral) e depois o intervalo da diferença. Veja também o [teste t](t-test).
