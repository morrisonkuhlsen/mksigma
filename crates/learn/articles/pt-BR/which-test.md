# Qual teste usar?

Comece pela sua pergunta e pelo tipo das suas variáveis: o nível de mensuração delas (Escala, Ordinal ou Nominal, na Visualização de variáveis) decide quase toda a escolha. Cada resposta abaixo diz o procedimento e abre o diálogo dele.

## Descrever uma variável

- Uma variável de **Escala** (idade, renda, uma nota): a média, a dispersão, o mínimo e o máximo, com as [Descritivas](descriptives). [Abrir Descritivas](dialog:descriptives).
- Uma variável **Nominal** ou **Ordinal** (um grupo, uma resposta numa escala de concordância): quantos casos têm cada valor, com as Frequências. [Abrir Frequências](dialog:frequencies).

## Comparar grupos numa variável de Escala

- **Dois grupos** de pessoas diferentes (homens e mulheres, tratados e não tratados): o [teste t para amostras independentes](t-test). [Abrir o teste t](dialog:ttest). Se os valores estão longe da normal e os grupos são pequenos, ou se a variável é só Ordinal, o teste de Mann-Whitney compara os postos no lugar das médias. [Abrir Testes não paramétricos](dialog:nonparametric).
- **Três grupos ou mais**: a ANOVA de um fator, ou o teste de Kruskal-Wallis sobre os postos. [Abrir ANOVA de um fator](dialog:oneway).
- **As mesmas pessoas medidas duas vezes** (antes e depois): o teste t pareado, ou os postos sinalizados de Wilcoxon.

## Relacionar duas variáveis

- Duas variáveis **categóricas** (Nominais ou Ordinais): uma [tabulação cruzada com o teste qui-quadrado](crosstabs). [Abrir Tabulação cruzada](dialog:crosstabs).
- Duas variáveis de **Escala**: a correlação de Pearson, de −1 a 1. [Abrir Correlacionar](dialog:correlations).

## Prever uma variável a partir de outras

- Um desfecho de **Escala** a partir de um ou mais preditores: a regressão linear. [Abrir Regressão](dialog:regression).
- Um desfecho com **dois valores** (sim ou não): a regressão logística binária.

> Todo teste responde se uma diferença ou relação poderia ser acaso; leia o [p-valor](p-value) junto com o tamanho do efeito, nunca sozinho.
