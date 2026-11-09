# Funcionalidades

O que o `memo` faz hoje. Cada item diz como foi verificado: **teste** (coberto pela suíte, 467 testes), **medido** (rodado num repositório real ou sintético, sem teste que o trave) ou **não verificado**. O que ainda falta ou funciona mal está em [Limitações](limitacoes.md), e o que pode vir depois em [Possíveis caminhos](caminhos.md).

## Indexação

- **Python em grafo.** Extrai módulos, classes, funções, métodos, atributos e variáveis de módulo e de classe com `tree-sitter-python` e guarda arestas de chamada (`CHAMA`, `CRIA`), tipo (`RECEBE`, `RETORNA`), herança (`ESPECIALIZA`), import (`IMPORTA`), escrita e leitura de atributo (`ESCREVE`, `LE`) e `LEVANTA`. (teste)
- **Incremental e determinística.** Reindexa só o que mudou; mesma árvore e mesma config geram a mesma saída, byte a byte. Duas indexações seguidas, sem mudança, não escrevem nada. (teste)
- **Resolução de chamadas** por sete regras mais evidência de import, marcando com `~` o que é aproximado. A cobertura (`resolvidas`, `aprox`, `não resolvidas`) vem em toda consulta de relação. (teste)
- **Índice que acompanha a config.** Mudar `skeleton`, `context_root`, `layers`, `verbs`, `roles`, `write_methods` ou os pesos de `[bridges]` reindexa tudo no próximo comando. (teste)
- **Base e feature.** A base vem da branch principal por ref git, sem checkout; a árvore de trabalho alimenta só o grafo da feature (`derive`, `promote`, `drop`). (teste)
- **Banco derivado e descartável.** Fica em `$COMMON/memo/`, fora da árvore versionada; mudar o esquema recria o banco sem tocar nas notas. (teste)

## A palavra como conceito

- **Palavras** vêm de nomes de símbolos, diretórios, imports, docstrings de testes (só as que já existem em algum identificador) e, em cada escopo, de identificadores, comentários e docstring (fonte `body`, só em `query word`, com a etiqueta `corpo`), com slot de ação, objeto, papel ou contexto. (teste)
- **Pontes** ligam duas palavras: mesmo símbolo, teste e símbolo testado, import, chamada, mesmo arquivo e mesmo contexto, com pesos editáveis. (teste)
- **Esqueleto** é decisão explícita: só as palavras da lista `skeleton` nunca são âncora. O `memo` não decide sozinho se uma palavra é de domínio. (teste)
- **Representatividade** na ficha `query word`: em arquivos, frente ao repositório, a cada contexto e a cada vizinha forte (`P(palavra|vizinha)` e `P(vizinha|palavra)`). É informação, não classificação. (teste; medido em um repositório real)
- **Aliases sugeridos** (`query aliases`): pares prováveis de sinônimos, somente leitura, com o `note add` pronto. (teste)

## Consultas

- `brief "<pedido>"`: âncoras, símbolos com a etiqueta das pontes que os trouxeram, contorno, padrão, escritores e notas. Os símbolos que carregam a palavra vêm primeiro (até 10), os que chegam só por vizinhança depois (até 5), com amortecimento por grau para um hub não dominar; os testes ficam numa seção própria. (teste; medido)
- `query symbol`, `callers`, `callees`, `impact`, `writers`, `readers`, `type`, `tests` e `word`. (teste)
- `query impact` tem a seção `por palavra`: todo símbolo de produção cujas palavras de uso (chamadas, receptores, tipos anotados) contêm as do nome do alvo e que nenhuma aresta cobre, com um item por uso (e um por tipo anotado, marcado `tipo`). Nada é escondido: cada item traz a linha e o nível de evidência como rótulo e ordem: `forte` (o nome exato da classe dona, de uma base, de uma subclasse ou de um Protocol dela num uso da mesma linha; ou, para função, o mesmo módulo ou o import do alvo), `média` (o nome da classe em outro ponto do símbolo) e `só nome`. Vem da camada palavra ↔ item, em que cada uso é um item e não depende de a chamada ter sido resolvida; é sempre aproximada (`~`). No `bope` (1.017 arquivos), de 50 a 80 ms por alvo. (teste; medido)
- `--no-tests`, `--limit`, `--max-chars`, `--json`, `--no-index`, e os deltas por task (`--since-last`, `--full`). (teste)
- Saída compacta, sempre com endereços `caminho:Linicio-Lfim`, e a linha final `cobertura`. (teste)

As correções de estrutura e relevância (TASK-8) acrescentam resolução de chamadas `Classe(...).metodo()`, origens distintas para escrita/inicialização/SQLAlchemy, âncoras de símbolo e arquivo no `brief`, contexto de mudanças base × feature e ranking por evidência antes do agrupamento. Fixtures sintéticas cobrem aliases, herança, factories, homônimos, imports ocultados por parâmetros, proximidade e propagação de confiança entre caminhos. Os trechos de leitura evitam sobreposição e completam blocos curtos até 40 linhas. Consulte o [contrato e os limites no README](../README.md#consultas).

A TASK-9 acrescenta a seção `relevantes:` com quatro estratégias e o peso de cada uma por item (vizinhança exata até 2 saltos, caminho de entrypoint, conexão com a região do alvo e nome exato), o detector de nomes genéricos (definições e precisão dos usos), entrypoints HTTP pela rota literal do decorador e por `entrypoint_decorators`, `testes via HTTP` por URL literal, contagem dos testes ocultos por tipo e homônimos ligados por chamada exata respondidos como grupo. A vizinhança vai até 3 saltos com peso 1/d, e o `brief` em texto escolhe âncoras pela interseção das palavras do pedido (o método herda as palavras da classe; palavras do `skeleton` desempatam). (teste; medido no `bope`: em 8 alvos com referência lida no código, 94,6% das referências no top 5 e 100% no top 10 da seção `relevantes`, contra 77% e 77% do `impact` anterior, sem os falsos positivos conhecidos; no `brief` em texto, num conjunto de 6 pedidos nunca usado para ajuste, top 5 de 27% para 72% e top 10 de 41% para 86%)

A TASK-10 acrescenta variáveis de módulo e de classe como símbolos, `ESCREVE` (`X = ...`, `X += ...`, `global`) e `LE` de nomes simples (inclusive em corpo de classe, decoradores, bases e valores padrão), com ocultamento por parâmetro, atribuição local e variável de compreensão, as seções `escritores` e `leitores` no `impact` de um atributo ou variável, e as palavras do corpo ligadas ao símbolo que as contém. Limites em [Limitações](limitacoes.md). (teste; C8 medido no repositório real de 800 arquivos: índice completo de 12,9 s para 15,2 s e banco de 24 MB para 35 MB; o ranking no `brief` **não foi medido**)

## Notas

- Uma afirmação durável por nota, em arquivo Markdown, ancorada em símbolo, palavra, aresta ou arquivo. Tipos `decisao`, `regra`, `restricao`, `descartado` e `glossario` (com `--alias`). (teste)
- Estados `proposta`, `confirmada`, `stale`, `orfa` e `substituida`, calculados na reindexação; reancoragem quando o símbolo só é renomeado; nada é apagado. (teste)
- Escopo de repositório ou de task, com `promote` depois do merge. (teste)

## Integração com agentes

- `memo init claude` e `memo init codex` instalam o bloco de instruções entre marcadores e, com `--mcp`, registram `memo serve`. (teste; os formatos de `.mcp.json` e `.codex/config.toml` **não foram verificados** contra os clientes reais)
- `memo serve`: servidor MCP por stdio com `brief`, `query`, `note_add` e `status`. (teste do protocolo; **não verificado** contra Claude Code nem Codex)

## Qualidade e desempenho

- Testes de saída com `insta` (golden files), `cargo fmt` e `cargo clippy -D warnings` limpos. (teste)
- Num repositório sintético de 5.000 arquivos (de um gerador que se perdeu): índice completo cerca de 44 s, reindexação sem mudança 0,04 s, `brief` cerca de 0,4 s, `query word` de 1 a 2,3 s. Num repositório real de 801 arquivos: índice completo cerca de 9 s. (medido) Com o gerador versionado em `scripts/measure/`, o perfil é outro: ver [Como reproduzir as medições](medicoes.md).
