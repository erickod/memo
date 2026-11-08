# Funcionalidades

O que o `memo` faz hoje. Cada item diz como foi verificado: **teste** (coberto pela suíte, 304 testes), **medido** (rodado num repositório real ou sintético, sem teste que o trave) ou **não verificado**. O que ainda falta ou funciona mal está em [Limitações](limitacoes.md), e o que pode vir depois em [Possíveis caminhos](caminhos.md).

## Indexação

- **Python em grafo.** Extrai módulos, classes, funções, métodos e atributos com `tree-sitter-python` e guarda arestas de chamada (`CHAMA`, `CRIA`), tipo (`RECEBE`, `RETORNA`), herança (`ESPECIALIZA`), import (`IMPORTA`), escrita e leitura de atributo (`ESCREVE`, `LE`) e `LEVANTA`. (teste)
- **Incremental e determinística.** Reindexa só o que mudou; mesma árvore e mesma config geram a mesma saída, byte a byte. Duas indexações seguidas, sem mudança, não escrevem nada. (teste)
- **Resolução de chamadas** por sete regras mais evidência de import, marcando com `~` o que é aproximado. A cobertura (`resolvidas`, `aprox`, `não resolvidas`) vem em toda consulta de relação. (teste)
- **Índice que acompanha a config.** Mudar `skeleton`, `context_root`, `layers`, `verbs`, `roles`, `write_methods` ou os pesos de `[bridges]` reindexa tudo no próximo comando. (teste)
- **Base e feature.** A base vem da branch principal por ref git, sem checkout; a árvore de trabalho alimenta só o grafo da feature (`derive`, `promote`, `drop`). (teste)
- **Banco derivado e descartável.** Fica em `$COMMON/memo/`, fora da árvore versionada; mudar o esquema recria o banco sem tocar nas notas. (teste)

## A palavra como conceito

- **Palavras** vêm de nomes de símbolos, diretórios, imports e docstrings de testes (só as que já existem em algum identificador), com slot de ação, objeto, papel ou contexto. (teste)
- **Pontes** ligam duas palavras: mesmo símbolo, teste e símbolo testado, import, chamada, mesmo arquivo e mesmo contexto, com pesos editáveis. (teste)
- **Esqueleto** é decisão explícita: só as palavras da lista `skeleton` nunca são âncora. O `memo` não decide sozinho se uma palavra é de domínio. (teste)
- **Representatividade** na ficha `query word`: em arquivos, frente ao repositório, a cada contexto e a cada vizinha forte (`P(palavra|vizinha)` e `P(vizinha|palavra)`). É informação, não classificação. (teste; medido em um repositório real)
- **Aliases sugeridos** (`query aliases`): pares prováveis de sinônimos, somente leitura, com o `note add` pronto. (teste)

## Consultas

- `brief "<pedido>"`: âncoras, símbolos com a etiqueta das pontes que os trouxeram, contorno, padrão, escritores e notas. Os símbolos que carregam a palavra vêm primeiro (até 10), os que chegam só por vizinhança depois (até 5), com amortecimento por grau para um hub não dominar; os testes ficam numa seção própria. (teste; medido)
- `query symbol`, `callers`, `callees`, `impact`, `writers`, `readers`, `type`, `tests` e `word`. (teste)
- `--no-tests`, `--limit`, `--max-chars`, `--json`, `--no-index`, e os deltas por task (`--since-last`, `--full`). (teste)
- Saída compacta, sempre com endereços `caminho:Linicio-Lfim`, e a linha final `cobertura`. (teste)

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
