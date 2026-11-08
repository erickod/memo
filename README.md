# memo

CLI em Rust que indexa um repositório Python como grafo de palavras, símbolos e chamadas, responde consultas sobre esse grafo e guarda notas humanas ancoradas nos símbolos. O objetivo é dar a um agente de IA contexto compacto e verificável, sem que ele precise explorar o repositório.

O `memo` não chama APIs de IA, não executa o código do repositório e não versiona o grafo. Ele entrega fatos; quem julga é o agente e o humano. A especificação completa está em [`Spec CLI de memória estrutural para IA.md`](Spec%20CLI%20de%20mem%C3%B3ria%20estrutural%20para%20IA.md).

## Instalação

Precisa de Rust estável e `git`. O SQLite vem embutido.

```bash
cargo install --path crates/memo-cli   # instala o binário `memo`
# ou, sem instalar:
cargo build --release                  # target/release/memo
```

## Início rápido

Dentro de um repositório git com código Python:

```bash
memo init claude --mcp        # ou: memo init codex --mcp
memo brief "calcular limite de crédito"
memo query impact CalculateLimitUseCase.execute
```

`memo init <harness>`:

1. cria `$COMMON/memo/` (`COMMON` é o `git rev-parse --git-common-dir`, então worktrees compartilham o estado);
2. grava `memo.toml` com as sementes, sem nunca sobrescrever um existente;
3. indexa a base a partir da branch principal, sem checkout;
4. instala, entre os marcadores `<!-- memo:begin -->` e `<!-- memo:end -->`, um bloco de instruções em `CLAUDE.md` (claude) ou `AGENTS.md` (codex). Com `--mcp`, registra também `memo serve` em `.mcp.json` (claude) ou `.codex/config.toml` (codex).

Use `--print` para ver o que seria escrito sem gravar nada. Rodar `init` de novo não duplica nada. O harness `agy` ainda não é suportado e dá erro (exit 1).

Nada é escrito na árvore versionada, a não ser pelo `init`, nos arquivos acima.

## Consultas

O `memo` reindexa de forma incremental antes de responder (desligue com `--no-index`). A saída é texto compacto, com endereços `caminho:Linicio-Lfim`. Um `~` marca relação aproximada, e o resultado vazio nunca prova ausência: confirme com busca no código. Toda consulta de relação termina com a linha `cobertura`.

```bash
memo brief "<pedido>"                        # âncoras, símbolos, contorno, padrão, escritores e notas
memo query symbol <símbolo>                  # endereço, assinatura, arestas diretas e notas
memo query callers <símbolo> --depth 2       # quem chama (profundidade até 3)
memo query callees <símbolo>                 # o que ele chama
memo query impact <símbolo>                  # chamadores, consumidores do tipo, Protocol, testes, notas
memo query writers <Classe.atributo>         # quem escreve o atributo
memo query readers <Classe.atributo>         # quem lê o atributo
memo query type <Tipo>                       # quem produz e quem consome o tipo
memo query tests <símbolo>                   # testes que o chamam (exato) ou se sobrepõem por palavras (~)
memo query word <palavra>                    # a ficha do conceito (ver abaixo)
memo query aliases                           # sugestões de sinônimos, somente leitura
```

`<símbolo>` aceita o `qname` completo ou um sufixo único (`CalculateLimitUseCase.execute`). Sufixo ambíguo lista as candidatas e sai com exit 2.

### Flags globais

| Flag | Efeito |
| --- | --- |
| `--repo <caminho>` | raiz do repositório (padrão: a raiz git atual) |
| `--task <id>` | task da chamada (também por `MEMO_TASK`); ativa os deltas e as notas da task |
| `--json` | saída JSON: `{"schema":1,"cmd":...,"data":...,"cobertura":...}` |
| `--no-index` | não reindexa antes de responder |
| `--limit <n>` | limita os itens listados |
| `--no-tests` | esconde os testes da saída (as estatísticas continuam contando tudo); a `cobertura` diz `N testes ocultos` |
| `--max-chars <n>` | teto de caracteres (padrão 6000); o corte termina com `+N omitidos` |
| `--since-last` / `--full` | só o que é novo desde a última chamada (`brief` e `query impact`) / tudo |

Com `--task`, a segunda chamada de `brief` ou `query impact` devolve só o que mudou, mais a linha `inalterados: N`.

### A palavra como conceito

O `memo` trata a palavra como o conceito central. Ela nasce de nomes de símbolos, de diretórios, de imports (nome e caminho do módulo) e das docstrings de testes (só palavras que já existem em algum identificador; `GIVEN`, `WHEN` e `THEN` marcam o trecho). Duas palavras se ligam por *pontes*: o mesmo símbolo, um teste e o símbolo que ele chama de forma exata, um import, uma chamada, o mesmo arquivo ou o mesmo contexto.

`memo query word <palavra>` devolve a ficha: estatísticas (inclui `coer=`, a coerência da vizinhança), verbos, papéis e camadas, as vizinhas por tipo de ponte com os pesos, e os símbolos com as pontes que os trazem (`[nome, teste, import]`). O `memo brief` usa a mesma vizinhança só para ranquear: cada símbolo mostra por que entrou, a produção vem primeiro e os testes ficam numa seção `testes:` ao final.

Uma palavra frequente só vira `esqueleto` (nunca âncora) se for pouco específica **e** pouco coerente. `order`, que aparece sempre ao lado das mesmas palavras, continua sendo conceito. `get` e `data`, ligadas a tudo, viram esqueleto.

`memo query aliases` sugere pares que provavelmente são o mesmo conceito (`cancel`/`canceled`, ou palavras com vizinhança parecida), cada um com o `memo note add --tipo glossario ... --alias ...` pronto. Só lê; quem decide é você.

### Códigos de saída

| Código | Significado |
| --- | --- |
| 0 | sucesso (inclui resultado vazio e arquivo que não parseou) |
| 1 | erro de uso (inclui id de task inválido e harness desconhecido) |
| 2 | símbolo, âncora ou task não encontrados |
| 3 | índice indisponível ou corrompido, ou banco travado por outro processo por mais de 30 s |
| 4 | validação de nota falhou |
| 10 | erro interno |

## Notas

Uma nota é uma afirmação durável que o código não mostra, ancorada no menor escopo que a sustenta. Cada nota é um arquivo Markdown em `$COMMON/memo/notes/<id>.md`.

```bash
memo note add --tipo regra --ancora sym:CalculateLimitUseCase.execute \
  --origem humano "Limite é inteiro, em centavos."
memo note list
memo note show <id>
memo note confirm <id>      # proposta -> confirmada
memo note recheck <id>      # nota stale: atualiza os hashes e volta ao estado anterior
```

- `--tipo`: `decisao`, `regra`, `restricao`, `descartado` ou `glossario` (este aceita `--alias`).
- `--ancora`: `sym:<qname>`, `word:<texto>`, `edge:<qname>-><qname>` ou `file:<caminho>`. Palavra `esqueleto` (genérica, como `get` ou `data`) não pode ser âncora (exit 4).
- `--origem`: `humano`, `recomendacao_aceita` ou `agente`. O CLI só registra, não verifica quem confirma.
- `--confirm` cria a nota já confirmada; `--substitui <id>` marca a anterior como `substituida`. A resposta lista até cinco `conflitos` na mesma âncora.
- Texto até 1.200 caracteres.

Na reindexação, uma nota vira `stale` se a assinatura ou o corpo do símbolo mudou, é reancorada sem `stale` se o símbolo só foi renomeado, e vira `orfa` se a âncora sumiu. Notas nunca são apagadas pela reindexação. Notas `confirmada` pesam 1,0, `proposta` 0,5 e `stale` 0,3 na recuperação.

## Fluxo por task

```bash
memo derive lend-9452      # copia a base e reindexa a árvore de trabalho atual
memo --task lend-9452 brief "..."          # consulta a feature
memo --task lend-9452 note add ...         # nota com escopo da task
memo promote lend-9452     # depois do merge: reindexa a base pela principal e promove as notas
memo drop lend-9452        # apaga a feature e as notas da task
memo status                # base, features, notas por estado, tasks promovíveis
```

A base sempre vem da branch principal (por ref git). A árvore de trabalho, com as suas mudanças ainda não mergeadas, só alimenta o grafo da feature. Sem `derive`, as consultas leem a base. Depois do merge, `promote` confere cada âncora das notas da task na base; as que não existem viram `orfa`.

Outros comandos:

```bash
memo index [--ref <ref>] [--full]   # reindexa e mostra o resumo dos eventos
memo serve                          # servidor MCP por stdio
```

## Servidor MCP

`memo serve` fala MCP por stdio (JSON-RPC, uma mensagem por linha) e expõe quatro ferramentas, com a mesma saída do CLI: `brief(text, task?)`, `query(kind, target, depth?, task?)`, `note_add(tipo, ancora, texto, task?, confirm?)` e `status(task?)`. Erros de domínio voltam com `isError: true`. Registro manual, se preferir ao `--mcp` do `init`:

```json
{ "mcpServers": { "memo": { "type": "stdio", "command": "memo", "args": ["serve"] } } }
```

O protocolo foi escrito sem teste contra clientes reais (Claude Code, Codex). Confira antes de depender dele.

## Configuração

`$COMMON/memo/memo.toml` nasce com as sementes e é editável. Chaves ausentes mantêm a semente, e uma lista presente a substitui:

| Chave | Efeito |
| --- | --- |
| `main_branch` | branch principal (padrão: `origin/HEAD`, depois `main`, depois `master`) |
| `roles`, `verbs` | vocabulário que classifica as palavras de um nome em papel e ação |
| `skeleton` | palavras genéricas, que nunca são âncora de `brief` nem de nota |
| `min_specificity` | abaixo disto a palavra também vira `esqueleto` (padrão 0,35; calibre no seu repositório) |
| `write_methods` | métodos de escrita externa (`save`, `publish`, ...) |
| `stale_on` | o que torna uma nota `stale`: `"sig"`, `"body"` ou os dois |
| `min_coherence` | abaixo disto (e com especificidade baixa) a palavra vira `esqueleto` (padrão 0,30) |
| `context_root`, `layers` | `app/<contexto>/<camada>`: o segmento após a raiz é o contexto, e a camada é um dos `layers`. `context_root` aceita um prefixo de caminho, como `backend/easy_lease` |
| `[bridges]` | pesos das pontes entre palavras: `symbol`, `test`, `import`, `import_external`, `call`, `file`, `context` e `approximate` (padrões 1,0 · 0,8 · 0,6 · 0,3 · 0,5 · 0,3 · 0,1 · 0,5) |

Mudar o `memo.toml` não reextrai arquivos inalterados; rode `memo index --full`.

**Atualização de versão:** o banco do `memo` é derivado. Quando o esquema muda (`schema_version`), o primeiro comando depois da atualização descarta o banco e reindexa tudo. As notas (`.md`) não são tocadas.

Variáveis de ambiente: `MEMO_TASK` (equivale a `--task`) e `MEMO_LOCK_TIMEOUT_MS` (espera pelo lock do banco, padrão 30000).

## Como funciona

- **Grafo em SQLite**, reconstruível do código: símbolos, palavras, ocorrências, arestas (`CHAMA`, `CRIA`, `RECEBE`, `RETORNA`, `ESPECIALIZA`, `IMPORTA`, `ESCREVE`, `LE`, `LEVANTA`) e eventos. O parse usa `tree-sitter-python`, é determinístico e roda em paralelo por arquivo.
- **Resolução de chamadas** em sete regras, da mais exata à mais aproximada. A injeção por `Protocol` aponta para o método do Protocol e liga `~` a cada implementação conhecida.
- **Determinismo:** a mesma árvore e a mesma configuração geram a mesma saída, byte a byte, sem LLM, sem rede e sem relógio nas consultas.
- **Falha parcial não é falha:** arquivo com erro de sintaxe entra na `cobertura` e o comando segue.
- **Estado fora da árvore versionada:**

```
$COMMON/memo/
  memo.toml            # configuração do repositório
  base.db              # grafo da branch principal
  features/<task>.db   # grafo derivado, um por task
  notes/<id>.md        # notas (fonte de verdade)
```

## Desenvolvimento

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

O workspace tem quatro crates: `memo-core` (extração, grafo, consultas, notas e eventos; não depende de `clap` nem de MCP), `memo-cli` (o binário `memo`), `memo-mcp` (servidor MCP) e `memo-harness` (adaptadores do `init`). A saída de texto é verificada por golden files (`insta`), e a fixture Python fica em `tests/fixtures/`.

## Limitações conhecidas

- Só Python.
- Nome definido mais de uma vez (`@property` com `@x.setter`, `@overload`, atributo e método com o mesmo nome, ou um módulo e uma função com o mesmo `qname`): vale a primeira definição. Mudar só o corpo de um setter não marca notas como `stale`.
- Âncoras secundárias de uma nota não geram `stale` nem `orfa`.
- Escrita em variável de módulo ou `global` ainda não gera a aresta `ESCREVE`.
- O limiar `min_specificity` precisa de calibração num repositório real; em repositórios pequenos, até palavras do domínio caem em `esqueleto`.
- Desempenho em repositório grande: a indexação completa de 5.000 arquivos levou cerca de 46 s numa medição sintética anterior à ficha do conceito (meta: 60 s). Depois dela, num repositório sintético de 5.000 arquivos, `brief` e `query word` ficaram lentos (minutos, contra a meta de 500 ms) e a indexação completa não foi remedida. A correção está planejada, mas ainda não foi feita.
- Os formatos de `.mcp.json` e `.codex/config.toml` vieram da documentação por busca resumida; confira com o seu cliente.
- Docstrings e comentários de produção ainda não geram palavras (só as de testes).
- O relatório de aliases por vizinhança sugere pares sem sentido em repositórios pequenos; use-o como lista para revisar.
- Em palavras com 10 vizinhas ou menos a coerência é 1,0, então só a lista `skeleton` as marca como esqueleto.
