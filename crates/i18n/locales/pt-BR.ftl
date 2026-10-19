# MK Sigma em português do Brasil.

language-menu = Idioma

## Ribbon and title row
autosave = Salvamento automático
autosave-on-tip = O salvamento automático está ligado: cada alteração é salva no arquivo
autosave-off-tip = Salvamento automático: salvar os dados no arquivo a cada alteração
save = Salvar
search-tip = Pesquisar comandos e variáveis (Ctrl+F)
export-output-tip = Exportar a saída
export-html-menu = Exportar saída como HTML…
export-pdf-menu = Exportar saída como PDF…
export-latex-menu = Exportar saída como LaTeX…
theme-tip = Tema
theme-light = Claro
theme-dark = Escuro
theme-system = Sistema
file = Arquivo
file-tip = Abrir, salvar, exportar e arquivos recentes
ribbon-show = Mostrar a faixa de opções (Ctrl+F1)
ribbon-hide = Ocultar a faixa de opções (Ctrl+F1)
open = Abrir
open-tip = Abrir um arquivo de dados (Ctrl+O)
save-tip = Salvar os dados (Ctrl+S)
undo = Desfazer
redo = Refazer
nothing-to-undo = Nada para desfazer
nothing-to-redo = Nada para refazer
run-syntax = Executar sintaxe
run-syntax-tip = Executar todos os comandos do editor de sintaxe
export-html = Exportar HTML
export-html-tip = Salvar a saída como página web
export-pdf = Exportar PDF
export-pdf-tip = Salvar a saída como documento PDF
export-latex-tip = Salvar a saída como documento LaTeX: tabelas booktabs, gráficos pgfplots
export-latex = Exportar LaTeX
clear-output = Limpar saída
clear-output-tip = Remover todos os itens da saída
insert-case = Inserir caso
insert-case-tip = Acrescentar um caso em branco no final
insert-variable = Inserir variável
insert-variable-tip = Acrescentar uma variável numérica
value-labels = Rótulos de valor
value-labels-tip = Mostrar os rótulos de valor em vez dos valores
dictionary = Dicionário
dictionary-tip = Mostrar a definição de cada variável na saída
suggest-tip = Acrescenta um comando ao editor de sintaxe, para completar e executar
compute = Calcular
recode = Recodificar
select-cases = Selecionar casos
dialog-tip = Abre o diálogo: escolha as variáveis e as opções
chart-dialog-tip = Abre o diálogo: escolha as variáveis
descriptives = Descritivas
frequencies = Frequências
explore = Explorar
crosstabs = Tabulação cruzada
t-test = Teste t
oneway = ANOVA de um fator
correlate = Correlacionar
regression = Regressão
logistic = Logística
nonparametric = Não paramétricos
histogram = Histograma
bar = Barras
scatter = Dispersão
boxplot = Boxplot
data-view = Visualização de dados
data-view-tip = Os casos e seus valores
variable-view = Visualização de variáveis
variable-view-tip = As variáveis e suas definições
output-style = Estilo da saída
output-style-tip = Como tabelas e gráficos aparecem na tela e em cada exportação
font = Fonte
size = Tamanho
palette = Paleta
decimals = Decimais
open-ellipsis = Abrir…
save-as-ellipsis = Salvar como…
save-dashboard-ellipsis = Salvar painel…
display-dictionary = Exibir dicionário
compute-variable = Calcular variável
t-test-independent = Teste t para amostras independentes
correlations-bivariate = Correlações bivariadas
regression-linear = Regressão linear
logistic-binary = Regressão logística binária
nonparametric-tests = Testes não paramétricos (Mann-Whitney)
bar-chart = Gráfico de barras
scatterplot = Gráfico de dispersão
theme-light-full = Tema claro
theme-dark-full = Tema escuro
tab-home = Início
tab-data = Dados
tab-transform = Transformar
tab-analyze = Analisar
tab-graphs = Gráficos
tab-view = Exibir
undo-named = Desfazer { $what }
redo-named = Refazer { $what }
decimals-auto = Auto

## Window and status
status-sample-start = Conjunto de dados de exemplo. Use Arquivo > Abrir… ou solte um arquivo .sav na janela.
status-in-syntax-editor = { $name } está no editor de sintaxe: Executar o executa
status-opened-dashboard = Painel { $name } aberto
status-new-dataset = Novo conjunto de dados: acrescente variáveis na Visualização de variáveis, ou abra um arquivo.
status-sample = Conjunto de dados de exemplo.
status-dashboard-needs-file = Os dados não vieram de um arquivo que possa ser lido de novo: abra-o com Arquivo > Abrir, ou salve-o antes
status-saved-dashboard = Painel salvo em { $path }
export-title-output = Saída do MK Sigma
export-title-output-of = { $file } — Saída
export-title-filtered = (filtrada por { $filters })
status-exported-output = Saída exportada para { $path }
export-title-dashboard = Painel do MK Sigma
export-title-dashboard-of = { $file } — Painel
status-exported-interactive = um painel interativo (ele contém os dados)
status-exported-static = o painel
status-exported-dashboard = Exportado { $what } para { $path }
status-saved = { $name } salvo
status-opened = { $name }: { $cases ->
    [one] { $cases } caso
    *[other] { $cases } casos
    }, { $variables ->
    [one] { $variables } variável
    *[other] { $variables } variáveis
    }
status-undid = Desfeito: { $what }
status-redid = Refeito: { $what }
untitled = Sem título
search-variable = Variável: { $label }
search-hint = Pesquisar comandos e variáveis
search-nothing = Nada encontrado
status-template-added = Comando acrescentado ao editor de sintaxe: troque os nomes provisórios e clique em Executar.
dropped-file = arquivo solto
syntax = Sintaxe
run = Executar
run-tip = Executar todos os comandos do editor
steps-paste = Colar passos na sintaxe
step-apply = Aplicar este passo
step-skipped = Ignorado: { $error }
step-up = subir
step-up-tip = Mover este passo para antes
step-down = descer
step-down-tip = Mover este passo para depois
step-remove = remover
step-remove-tip = Remover este passo
status-exported-calculation = Cálculo exportado para { $path }
status-pasted = Colado no editor de sintaxe: Executar o executa.
status-ready = Pronto
fact-cases = { $n ->
    [one] { $n } caso
    *[other] { $n } casos
    }
fact-variables = { $n ->
    [one] { $n } variável
    *[other] { $n } variáveis
    }
fact-weighted = Ponderado por { $name }
zoom-reset = Zoom em 100%
zoom-in = Aumentar zoom
zoom-out = Diminuir zoom
zoom = Zoom
fact-filtered = Filtrado por { $filters }
