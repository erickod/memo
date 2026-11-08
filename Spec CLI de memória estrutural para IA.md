# Spec: CLI de memória estrutural para IA

Nov 7, 2026 · @Erick Duarte

## 1. Objetivo e escopo

`memo` (nome provisório) é um CLI em Rust que indexa um repositório Python como grafo de palavras, símbolos e chamadas, expõe esse grafo a consultas e guarda notas humanas ancoradas nos símbolos. O objetivo é dar a um agente de IA contexto compacto e verificável, sem que ele precise explorar o repositório.

**O MVP entrega:**

- indexação incremental e determinística de Python, para a branch principal (base) e para uma feature (derivada da base);
- consultas: `brief`, `callers`, `callees`, `impact`, `writers`, `word`, `type`, `tests`, `symbol`;
- notas atômicas ancoradas, com os estados `stale` e `órfã` detectados na reindexação;
- `init`, que instala instruções no harness (claude e codex; agy sob verificação), e `serve` (MCP) sobre o mesmo núcleo.

**Não-objetivos:** chamar APIs de provedores de IA; executar código do repositório; versionar o grafo; julgar código (o CLI entrega fatos, quem julga é o agente e o humano); suportar outras linguagens (a interface prevê, o MVP não implementa).

**Consumidores:** o agente, via shell (canal principal) ou MCP, e o humano, pelo mesmo CLI.

## 2. Princípios e restrições

Sete regras valem para todo o código e orientam qualquer decisão não prevista nesta spec.

1. **Derivado e descartável.** Tudo estrutural se reconstrói do código. As notas são o único estado próprio.
2. **Determinístico.** Mesma árvore de arquivos e mesma config geram o mesmo grafo e a mesma saída, byte a byte. Sem LLM, sem rede e sem relógio no resultado das consultas.
3. **Saída compacta.** Texto denso, endereços `caminho:Linicio-Lfim`, nenhum trecho de código, nenhum eco da entrada nos itens (só o cabeçalho de `brief` repete o pedido, como no exemplo da seção 8.3). `--json` é opcional. O limite padrão é `--max-chars 6000`, com a linha final `+N omitidos`.
4. **Cobertura explícita.** Toda consulta de relação devolve `cobertura` (resolvidas, aproximadas, não resolvidas). Aresta inferida leva `~`. Resultado vazio nunca prova ausência.
5. **Índice sempre fresco.** Comandos de consulta reindexam de forma incremental antes de responder (desligável com `--no-index`).
6. **Não toca a árvore versionada.** Todo estado vive fora dela. A única escrita externa é a de `init`, no arquivo de instruções e na config MCP do harness, sob marcadores, com `--print` como dry-run.
7. **Falha parcial não é falha.** Arquivo que não parseia entra em `cobertura.arquivos_com_erro` e o comando segue.

**Restrições técnicas:** Rust estável; SQLite via `rusqlite` com `bundled`; `tree-sitter` e `tree-sitter-python`; `clap`; `serde` e `serde_json`; `toml`; `blake3`; binário único, sem runtime externo; testes de saída com `insta` (golden files). Dependência fora dessa lista exige justificativa no PR.

## 3. Arquitetura

O núcleo é uma biblioteca; o CLI, o servidor MCP e os adaptadores de harness são consumidores finos que não repetem lógica.

&#91;embedded content: arquitetura · 3 consumidores, 1 núcleo, 2 fontes de estado\]

`memo-cli` e `memo-mcp` chamam as mesmas funções de `memo-core`. `memo-harness` só roda no `init`. O estado fica em `$COMMON/memo/` e o código do repositório só é lido.

### Workspace Cargo

| Crate | Responsabilidade |
| --- | --- |
| `memo-core` | extração, grafo, consultas, notas e eventos; não depende de `clap` nem de MCP |
| `memo-cli` | binário `memo`: argumentos, formatação (texto e JSON) e códigos de saída |
| `memo-mcp` | servidor MCP por stdio com as quatro ferramentas da seção 9.4 |
| `memo-harness` | trait `HarnessAdapter` e os adaptadores |

### Traits de fronteira

- `LanguageExtractor`: lê um arquivo e devolve fatos (símbolos, palavras, referências brutas). MVP: `PythonExtractor`.
- `GraphStore`: persistência e consulta do grafo. MVP: `SqliteStore`. Caminhos de profundidade variável (callers de callers) usam CTE recursiva dentro do store, então trocar o motor não muda quem consulta.
- `SourceTree`: fonte dos arquivos (seção 6.2).
- `HarnessAdapter`: instalação por harness (seção 9.2).

### Regras de implementação

- `memo-core` não imprime nada: devolve estruturas. O formato de saída é responsabilidade de `memo-cli` e `memo-mcp`.
- Erros de domínio tipados com `thiserror`, mapeados para os códigos da seção 8.1. `anyhow` só nas bordas dos binários.
- O parse roda em paralelo por arquivo e a escrita no SQLite numa única transação por indexação. `rayon` é a única dependência extra já aprovada, para esse paralelismo.

## 4. Modelo de dados

O grafo vive em SQLite, com uma tabela por entidade. O esquema abaixo é o contrato: o agente pode ajustar tipos e índices, mas não os nomes nem a semântica das colunas.

```sql
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT);   -- schema_version, indexed_ref, indexed_commit
CREATE TABLE file(
  id INTEGER PRIMARY KEY, path TEXT UNIQUE NOT NULL, hash TEXT NOT NULL,
  mtime_ns INTEGER, size INTEGER, context TEXT, layer TEXT, parse_error TEXT);
CREATE TABLE symbol(
  id INTEGER PRIMARY KEY, qname TEXT UNIQUE NOT NULL, name TEXT NOT NULL,
  kind TEXT NOT NULL,        -- package, module, class, function, method, attribute, external
  nivel INTEGER NOT NULL,    -- 1 attr, 2 classe, 3 função/método, 4 módulo, 5 pacote
  parent_id INTEGER REFERENCES symbol(id), file_id INTEGER REFERENCES file(id),
  line_start INTEGER, line_end INTEGER,
  sig_hash TEXT, body_hash TEXT, decorators TEXT,
  is_test INTEGER DEFAULT 0, is_entrypoint INTEGER DEFAULT 0, is_protocol INTEGER DEFAULT 0);
CREATE TABLE word(id INTEGER PRIMARY KEY, text TEXT UNIQUE NOT NULL);
CREATE TABLE occurrence(
  symbol_id INTEGER,         -- só quando o dono é um símbolo e a palavra vem do nome
  word_id INTEGER NOT NULL, pos INTEGER NOT NULL,
  slot TEXT NOT NULL,        -- action, object, role, context
  segment TEXT,              -- given, when, then (só em testes)
  owner_kind TEXT NOT NULL DEFAULT 'symbol',  -- symbol, file, import
  owner_id INTEGER NOT NULL,
  source TEXT NOT NULL DEFAULT 'name',        -- name, path, import, test_doc (doc: reservado, fase 2)
  PRIMARY KEY(owner_kind, owner_id, source, pos));
CREATE TABLE doc(symbol_id INTEGER PRIMARY KEY, text TEXT NOT NULL);  -- docstring de teste, para refazer as palavras
CREATE TABLE edge(
  id INTEGER PRIMARY KEY, kind TEXT NOT NULL,
  src INTEGER NOT NULL REFERENCES symbol(id), dst INTEGER REFERENCES symbol(id),
  dst_text TEXT,             -- destino externo ou não resolvido
  line INTEGER, attr TEXT, pos INTEGER,
  approx INTEGER DEFAULT 0, count INTEGER DEFAULT 1);
CREATE TABLE raw_ref(file_id INTEGER, src INTEGER, kind TEXT, callee_text TEXT, receiver TEXT, line INTEGER);
CREATE TABLE import(file_id INTEGER, local_name TEXT, target TEXT, line INTEGER, type_checking INTEGER DEFAULT 0);
CREATE TABLE word_stats(
  word_id INTEGER PRIMARY KEY, uso INTEGER, alcance INTEGER, grau INTEGER,
  idf REAL, especificidade REAL, centralidade REAL, esqueleto INTEGER, coerencia REAL);
CREATE TABLE event(id INTEGER PRIMARY KEY, batch TEXT NOT NULL, seq INTEGER, kind TEXT NOT NULL, subject TEXT, payload TEXT);
CREATE TABLE session_seen(task TEXT, cmd TEXT, key TEXT, PRIMARY KEY(task, cmd, key));
```

Índices mínimos: `edge(src, kind)`, `edge(dst, kind)`, `occurrence(word_id)`, `symbol(file_id)`, `symbol(parent_id)`. `raw_ref` e `import` guardam o que foi lido da AST para permitir re-resolver chamadas sem reparsear o arquivo.

### Identidade do símbolo

- `qname` é o caminho pontuado do módulo mais classes e nome: `app.billing.application.usecases.calculate_limit_usecase.CalculateLimitUseCase.execute`. Atributos usam o mesmo padrão (`...CalculateLimitUseCase._repo`). Símbolos de fora do repo usam `ext:<texto>`.
- `sig_hash` é o `blake3` da assinatura normalizada: nomes dos parâmetros, anotações, presença de default, retorno, `async` e decoradores.
- `body_hash` é o `blake3` do corpo serializado por tipos de nó e folhas, sem comentários, docstring, espaços e números de linha. Reformatar não gera evento.
- Linha, pesos e estatísticas são atributos recalculados. Nunca fazem parte da identidade.

### Níveis

O `nivel` segue a taxonomia L1 a L5 do n-partite para o modo code: attr (1), classe (2), função ou método (3), módulo (4), pacote (5).

### Tipos de aresta

A contenção (pacote, módulo, classe, método) usa `parent_id` e `file_id`, não aresta. A composição por palavras usa `occurrence`. As arestas são estas:

| Tipo | Origem → destino | Como nasce na AST | Exata ou aproximada |
| --- | --- | --- | --- |
| `CHAMA` | função ou método → símbolo | chamada, resolvida pelas regras da seção 5; `line` é a linha da chamada | exata quando resolvida por escopo, import, `self` ou tipo anotado; `~` nas demais |
| `CRIA` | função ou método → classe | `Tipo(...)` | exata quando resolve |
| `RECEBE` | função ou método → tipo | anotação de parâmetro, desembrulhada (`list[X]`, `Optional[X]`, `X \| None` viram `X`); `attr` guarda o nome do parâmetro e `pos` a posição | exata |
| `RETORNA` | função ou método → tipo | anotação de retorno, desembrulhada | exata |
| `ESPECIALIZA` | classe → classe | herança explícita, ou implementação estrutural de um Protocol (todos os métodos públicos existem na classe, com a mesma aridade) | exata na herança, `~` na estrutural |
| `IMPORTA` | módulo → módulo ou símbolo | `import` e `from ... import`; `attr = type_checking` quando sob `TYPE_CHECKING` | exata |
| `ESCREVE` | função ou método → atributo ou externo | `self.x = ...`, `self.x[k] = ...`, `+=`, método mutador em `self.x`, escrita em global, chamada de método de escrita em receptor tipado | exata para atributo, `~` para externo por nome |
| `LE` | função ou método → atributo | leitura de `self.x` | exata |
| `LEVANTA` | função ou método → classe de exceção | `raise X(...)` ou `raise X` | exata quando resolve |

### Contexto e camada

`file.context` e `file.layer` saem de configuração em `memo.toml`: `context_root` (segmento raiz, padrão `app`; o segmento seguinte é o contexto) e `layers` (segmentos que nomeiam a camada). Sem motor de glob no MVP. A regra padrão segue o guidelines do projeto: o segmento após `app/` é o contexto, e o segmento `domain`, `application` ou `infra` é a camada.

## 5. Extração estática Python

A extração roda por arquivo, atrás da trait `LanguageExtractor` (o MVP implementa só Python). Cada arquivo passa por cinco etapas: símbolos, palavras, tipos, chamadas e efeitos. Depois, uma passada de resolução liga o que foi lido.

### 5.1 Símbolos

Extrair módulo, classes, funções, métodos e atributos (`self.x = ...` em qualquer método, e atributos anotados no corpo da classe). Registrar `line_start` e `line_end`, decoradores, `is_test` (arquivo em `tests/` ou nome `test_*`), `is_entrypoint` (decorador que casa com `entrypoint_decorators` da config) e `is_protocol` (classe que herda de `Protocol`). Funções locais e lambdas não viram símbolos.

### 5.2 Palavras e slots

1. Quebrar cada nome por `snake_case`, `CamelCase` e dígitos. Siglas ficam juntas (`HTTPServer` vira `http`, `server`). Tudo em minúsculas. Nomes dunder (`__init__`) não geram palavras.
2. Juntar tokens vizinhos quando a forma unida está na lista de papéis (`use`, `case` vira `usecase`).
3. Classificar cada palavra pelo vocabulário, nunca pela posição: está em `roles` → `role`; senão está em `verbs` e o símbolo não é atributo → `action`; senão → `object`. Slot vazio é válido.
4. Em testes, tirar o prefixo `test_` e registrar o `segment` (`given`, `when`, `then`) de cada palavra, pelos marcadores `given_`, `when_` e `then_` do nome.
5. Sem stemming e sem tradução no MVP. Sinônimos e plurais se resolvem por notas `glossario` (seção 7); `memo query aliases` sugere pares (somente leitura).
6. Outras fontes de palavras, cada uma com `owner_kind` e `source` próprios na `occurrence`:
   - **Diretórios do arquivo** (`source = path`, dono `file`): a raiz `context_root` sai, o segmento seguinte é o contexto (slot `context`), os segmentos de `layers` saem e os demais viram palavras.
   - **Imports** (`source = import`, dono o import): o nome importado e o caminho do módulo. A raiz do pacote (último segmento de `context_root`) e as camadas saem, o segmento seguinte à raiz é o contexto (slot `context`). Alvo que não começa pela raiz (externo) usa todos os segmentos e pesa menos nas pontes.
   - **Docstring de teste** (`source = test_doc`, dono o símbolo de teste): `GIVEN`, `WHEN` e `THEN` são marcadores de `segment`, não palavras; só conta a palavra que já existe como palavra de nome, caminho ou import (vocabulário fechado).
   Docstrings e comentários de produção ficam para uma segunda fase.

O vocabulário vem de `memo.toml` (seção 6) com sementes embutidas: `roles` (`service`, `usecase`, `repository`, `handler`, `mapper`, `schema`, `dto`, `input`, `output`, `protocol`, `exception`, `client`, `event`, `command`, `route`), `verbs` (`get`, `set`, `create`, `calculate`, `validate`, `update`, `delete`, `find`, `list`, `handle`, `execute`, `build`, `parse`, `publish`, `consume`, `save`, `load`, `send`) e `skeleton` (`get`, `data`, `utils`, `helper`, `base`, `manager`, `common`, `process`, `service`).

### 5.3 Tipos

Para cada função ou método, ler as anotações dos parâmetros e do retorno. Desembrulhar `list[X]`, `set[X]`, `Optional[X]`, `X | None`, `Awaitable[X]` e `Coroutine[..., X]`. Guardar o texto bruto em `dst_text` quando o tipo não resolve para símbolo do repo. Parâmetro sem anotação gera `RECEBE` com `dst_text = NULL` e o nome em `attr`: o nome é o sinal que sobra.

### 5.4 Resolução de chamadas

Aplicar as regras em ordem e parar na primeira que resolve. Toda aresta nasce com `line` (linha da chamada) e `approx`.

1. `nome()` com definição no mesmo módulo ou escopo envolvente: `CHAMA`, exata.
2. Nome importado (`from x import f`, `import x; x.f()`): resolver pela tabela `import` até o `qname`; `CHAMA`, exata se existe no índice.
3. `self.m()` e `cls.m()`: método da própria classe ou de bases do repo; exata se acha, `~` se não.
4. `self.attr.m()` em que `attr` vem de parâmetro anotado do `__init__` (`self.attr = param`) ou de anotação de classe: resolver o tipo, depois o método `m` nesse tipo; exata. Se o tipo é Protocol, a aresta vai para o método do Protocol (exata) e uma aresta extra `~` vai para cada implementação conhecida (`ESPECIALIZA` estrutural).
5. `Tipo(...)` com `Tipo` resolvendo para classe: `CRIA`, exata.
6. `x.m()` com `x` local anotado ou construído na mesma função (`x = Tipo(...)`): método de `Tipo`; `~`.
7. Sem resolução: se o nome do método existe em exatamente um símbolo do repo, `CHAMA` `~` para ele; se existe em dois ou mais, não criar aresta e somar em `cobertura.nao_resolvidas`; se não existe, `CHAMA` com `dst_text` externo.

As regras 3 e 4 cobrem o padrão de DI do guidelines (`__init__(self, *, dep: Protocol)`), que é uniforme e por isso resolve bem por anotação.

### 5.5 Efeitos, por símbolo e não por valor

- `ESCREVE` atributo: `self.x = ...`, `self.x[k] = ...`, atribuição aumentada, método mutador em `self.x` (`append`, `extend`, `update`, `add`, `pop`, `remove`, `clear`, `insert`, `setdefault`, lista em config) e escrita em variável de módulo ou `global`.
- `ESCREVE` externo: chamada a método da lista `write_methods` (`save`, `add`, `insert`, `update`, `delete`, `publish`, `send`, `commit`, `flush`) em receptor de tipo conhecido; `dst_text` = `Tipo.metodo`, `~`.
- `LE`: leitura de `self.x`.
- `LEVANTA`: `raise X(...)` e `raise X`; `raise` sem argumento não gera aresta.
- Variáveis locais não são armazenadas.

### 5.6 Pesos

Após cada indexação, recalcular `word_stats` com SQL e uma iteração de PageRank:

- `uso` = nº de ocorrências; `alcance` = nº de arquivos distintos; `grau` = nº de palavras distintas que co-ocorrem em algum símbolo;
- `idf` = `ln(N_símbolos / (1 + símbolos_com_a_palavra))`, e `especificidade` = `idf` normalizado em 0 a 1;
- `centralidade` = PageRank (amortecimento 0,85, 20 iterações) no grafo de co-ocorrência por símbolo, sem palavras `skeleton`;
- `coerencia` = peso das 10 vizinhas mais fortes (peso decrescente, nome crescente) dividido pelo peso total das pontes da palavra, de 0 a 1; vizinhas da lista `skeleton` não contam; sem vizinhas, 0;
- `esqueleto = 1` se a palavra está em `skeleton`, ou se tem `especificidade` abaixo de `min_specificity` (valor inicial 0,35) **e** `coerencia` abaixo de `min_coherence` (valor inicial 0,30). Valores a calibrar no repo real. Palavra frequente e coerente (um conceito, como `order` com `placed`, `canceled`) não é esqueleto; palavra frequente ligada a tudo (`get`, `data`) é.

**Pontes entre palavras** (derivadas, sem tabela): símbolo (mesmo nome) 1,0 · teste e símbolo testado, só por chamada exata 0,8 · import 0,6 (externo 0,3) · chamada 0,5 · mesmo arquivo 0,3 · mesmo contexto 0,1; ponte aproximada (`~`) vale metade. Pesos em `[bridges]` do `memo.toml`. As pontes de arquivo e contexto só são calculadas por palavra (na ficha); a coerência usa símbolo, teste, import e chamada.

Palavra `esqueleto` nunca é âncora de `brief` nem de nota.

## 6. Armazenamento, escopo e reindexação

O grafo da feature é uma cópia completa do grafo da base, reindexada só no que mudou. Não existe overlay nem merge de grafo: como o grafo é derivado, depois do merge basta reindexar a branch principal.

### 6.1 Onde fica o estado

`COMMON` é o resultado de `git rev-parse --git-common-dir`, o que faz o estado ser compartilhado entre worktrees. Nada fica na árvore versionada.

```
$COMMON/memo/
  memo.toml            # config do repo (vocabulário, limiares, globs)
  base.db              # grafo da branch principal
  features/<task>.db   # grafo derivado, um por task
  notes/<id>.md        # notas (fonte de verdade, uma por arquivo)
```

- `memo.toml` nasce no `init` com as sementes embutidas e é editável. Define `main_branch`, `roles`, `verbs`, `skeleton`, `write_methods`, `entrypoint_decorators`, `context_root` e `layers`, `min_specificity` e `stale_on`, `min_coherence` e a tabela `[bridges]` (pesos das pontes). `context_root` aceita um prefixo de caminho (`backend/easy_lease`): o contexto é o segmento seguinte.
- A chave da feature é `--task <id>` (ou `MEMO_TASK`). Sem ela, usa o nome da branch atual com barras trocadas por hífen.
- A branch principal vem de `main_branch`; se ausente, de `origin/HEAD`, depois `main`, depois `master`.

### 6.2 Fonte dos arquivos

A indexação lê de uma abstração `SourceTree` com duas implementações: a árvore de trabalho (`git ls-files -co --exclude-standard`, só `*.py`) e uma ref git (`git ls-tree` mais `git cat-file --batch`, sem checkout). Isso permite indexar a base mesmo estando em outra branch.

### 6.3 Reindexação incremental

1. Listar os arquivos. Comparar `mtime_ns` e `size` com o registrado; só os diferentes têm o `hash` recalculado.
2. Arquivos novos ou com hash diferente: reparsear e substituir seus símbolos, palavras, ocorrências, `raw_ref` e `import`. Arquivos removidos: apagar seus símbolos.
3. Re-resolver chamadas e tipos dos arquivos alterados e dos que importam módulos alterados, a partir de `raw_ref` e `import`.
4. Recalcular `word_stats`.
5. Comparar o conjunto de símbolos antes e depois e emitir os eventos da seção 10, todos no mesmo `batch`, dentro de uma transação.

Indexar duas vezes seguidas sem mudança não escreve nada e não emite eventos. Meta inicial: reindexação sem mudanças abaixo de 300 ms em 5.000 arquivos.

### 6.4 Derivar, promover e descartar

- `derive <task>`: copia `base.db` para `features/<task>.db` (backup API do SQLite) e roda a reindexação sobre a árvore de trabalho atual. Se a base estiver atrás da branch principal, o comando avisa em uma linha e segue.
- Consultas com `--task` (ou com a branch atual casando um `features/<task>.db`) leem o grafo da feature; sem ele, leem a base.
- `promote <task>`: depois do merge, reindexa a base a partir da ref principal e promove as notas da task (seção 7). Não é automático. `status` lista as tasks cujos símbolos já aparecem na base ou cuja branch foi mergeada (`git branch --merged`, ou `gh pr view` se disponível).
- `drop <task>`: apaga `features/<task>.db` e as notas com escopo da task. Só por comando explícito.

Squash e rebase não quebram nada, porque nenhuma identidade usa hash de commit: a promoção confere os âncoras pelo `qname` e pelos hashes de assinatura e de corpo.

## 7. Notas

A nota é o único conhecimento que o CLI guarda além do grafo. Cada nota é um arquivo Markdown em `$COMMON/memo/notes/<id>.md`, com uma única afirmação, ancorada no menor escopo que a sustenta. O banco só mantém um índice reconstruível dos arquivos.

```markdown
---
id: n-4k2m9q1x
tipo: regra
estado: confirmada
escopo: task:lend-9452
ancora: sym:app.billing.domain.term.calculate_term_days
secundarias: [word:prazo]
origem: humano
task: lend-9452
ancora_sig: 9f3c1a
ancora_body: 07be44
criada: 2026-11-07T14:05:00-03:00
---
Prazo é contado em dias corridos, por regra do contrato.
```

### 7.1 Campos

| Campo | Valores | Observação |
| --- | --- | --- |
| `tipo` | `decisao`, `regra`, `restricao`, `descartado`, `glossario` | `glossario` aceita `aliases: [..]` (sinônimos, plurais, termos em outro idioma) |
| `estado` | `proposta`, `confirmada`, `stale`, `orfa`, `substituida` | transições abaixo |
| `escopo` | `repo` ou `task:<id>` | notas de task só aparecem em consultas dessa task |
| `ancora` | `sym:<qname>`, `word:<texto>`, `edge:<qname>-><qname>`, `file:<caminho>` | uma principal e, opcionalmente, `secundarias` |
| `origem` | `humano`, `recomendacao_aceita`, `agente` | informada pelo chamador; o CLI só registra |
| `ancora_sig`, `ancora_body` | hashes no momento da nota | base da detecção de `stale` |
| `substitui` | id de outra nota | a antiga passa a `substituida` |

Texto: até 1.200 caracteres (rejeitar acima disso); a recomendação é uma afirmação por nota, até 600.

### 7.2 Validação em `note add`

1. Todo âncora existe no grafo consultado; senão, erro `ancora_inexistente` (exit 2).
2. Âncora `word:` não pode ser palavra `esqueleto`; senão, erro `ancora_generica` (exit 4).
3. O estado inicial é `proposta`. Com `--confirm` vira `confirmada`; o agente só o usa para decisões vindas de rodada humana. O CLI não verifica quem confirma, só registra `origem`.
4. A resposta lista até cinco notas existentes na mesma âncora principal sob `conflitos:`. A decisão é humana, e a nova nota pode usar `--substitui`.

### 7.3 Transições, calculadas na reindexação

| Situação | Resultado |
| --- | --- |
| `sig_hash` ou `body_hash` da âncora `sym:` mudou (`stale_on`, padrão os dois) | `stale` |
| Âncora `sym:` sumiu e há par de renomeação no lote (mesmos `sig_hash` e `body_hash`) | reancora para o novo `qname`, evento `SYMBOL_RENAMED`, sem `stale` |
| Âncora `sym:` sumiu sem par | `orfa` |
| Âncora `word:` perdeu a última ocorrência | `orfa` |
| Âncora `edge:` desapareceu | `orfa`; se só um extremo mudou de assinatura, `stale` |
| `note recheck <id>` | atualiza os hashes e volta ao estado anterior ao `stale` |
| `note add --substitui <id>` | a antiga vira `substituida` |

Nota nunca é apagada pela reindexação. `orfa` e `substituida` saem das consultas e ficam em `note list`.

### 7.4 Peso na recuperação

`confirmada` vale 1,0; `proposta`, 0,5; `stale`, 0,3 e aparece com marcador. Nenhuma nota é podada por peso.

### 7.5 Promoção

`promote <task>` troca `escopo: task:<id>` por `repo` nas notas da task e confere cada âncora na base. Âncora que não existe vira `orfa` e entra no relatório. Notas `proposta` sobem como `proposta`.

## 8. Comandos e formato de saída

### 8.1 Flags globais e códigos de saída

Flags: `--repo <caminho>` (padrão: raiz git atual), `--task <id>`, `--json`, `--no-index`, `--limit <n>`, `--max-chars <n>` (padrão 6000), `--since-last` e `--full` (deltas, seção 8.4). `--no-tests` (padrão desligada) esconde os testes da saída de `brief` e dos `query`, sem alterar as estatísticas; a `cobertura` informa `N testes ocultos`; em `query tests` é erro de uso.

| Código | Significado |
| --- | --- |
| 0 | sucesso (inclui resultado vazio e falha parcial de parse) |
| 1 | erro de uso |
| 2 | símbolo, âncora ou task não encontrados |
| 3 | índice indisponível ou corrompido |
| 4 | validação de nota falhou |
| 10 | erro interno |

### 8.2 Comandos

| Comando | Função |
| --- | --- |
| `init [<repo>] <harness> [--ref] [--mcp] [--print]` | cria `$COMMON/memo`, grava `memo.toml`, indexa a base e instala a integração (seção 9) |
| `index [--ref <ref>] [--full]` | reindexação incremental; `--ref` indexa uma ref git sem checkout |
| `derive <task> [--from <ref>]` | cria o grafo da feature (seção 6.4) |
| `brief "<pedido>"` | âncoras, símbolos, contorno, padrão, escritores e notas para um pedido em texto |
| `query callers <sym> [--depth n]` | quem chama o símbolo (profundidade até 3) |
| `query callees <sym> [--depth n]` | o que o símbolo chama |
| `query impact <sym>` | callers, consumidores do tipo (`RECEBE`, `RETORNA`), Protocol e implementações, testes, notas |
| `query writers <attr>` e `query readers <attr>` | símbolos que escrevem ou leem o atributo |
| `query word <w>` | a ficha do conceito: estatísticas (inclui coerência), verbos, papéis, camadas, vizinhas por tipo de ponte com os pesos e os símbolos com as pontes que os trazem |
| `query aliases` | pares de palavras que provavelmente são o mesmo conceito (prefixo comum, ou vizinhança parecida), com o `note add` pronto; somente leitura |
| `query type <T>` | quem produz e quem consome o tipo |
| `query tests <sym>` | testes que chamam o símbolo (exato) e por sobreposição de palavras (`~`) |
| `query symbol <qname>` | endereço, assinatura, arestas diretas, notas |
| `note add \| list \| show \| confirm \| recheck` | ciclo das notas (seção 7) |
| `status` | base e features, notas por estado, tasks promovíveis, cobertura do índice |
| `promote <task>` e `drop <task>` | seção 6.4 |
| `serve` | servidor MCP por stdio (seção 9) |

`<sym>` aceita `qname` completo ou sufixo único (`CalculateLimitUseCase.execute`). Sufixo ambíguo devolve as candidatas e exit 2.

### 8.3 Formato de saída (texto)

Uma linha por item, endereço sempre presente, `~` marcando aresta aproximada, nenhuma linha de código. Abreviações de tipo: `C` classe, `F` função, `M` método, `A` atributo, `T` teste.

```
brief "antecipar fatura com desconto"  task=lend-9452 base=a1b2c3d idx=ok
âncoras: desconto(0.82) antecipar(0.77) fatura(0.64)   ignoradas: service get data
símbolos:
  F calculate_discount          domain/discount.py:L15-L38
  M AnticipateInvoiceUseCase.execute   application/usecases/anticipate_invoice_usecase.py:L20-L71
contorno:
  AnticipateInvoiceUseCase.execute -> calculate_discount :L41 ; -> InvoiceRepository.save ~ :L58
padrão(desconto): verbos calculate,apply · papéis usecase,repository,schema · camadas domain,application,infra
escritores: Invoice.status <- AnticipateInvoiceUseCase.execute, CancelInvoiceUseCase.execute
notas:
  n-4k2m9q1x regra confirmada  calculate_term_days: Prazo é contado em dias corridos.
cobertura: 41 resolvidas, 3 aprox, 2 não resolvidas, 0 arquivos com erro
```

`--json` devolve `{"schema":1,"cmd":...,"task":...,"index":{...},"data":...,"cobertura":{...}}`, com os mesmos itens e endereços como campos (`path`, `line_start`, `line_end`, `approx`).

No `brief`, cada símbolo traz entre colchetes as pontes que o trouxeram (`nome`, `docstring`, `teste`, `chamada`, `tipo`, `import`), a produção vem primeiro (até 15 símbolos) e os testes ficam numa seção `testes:` ao final (até 5); o contorno resume os testes que chamam como `<- +N testes`.

Quando a saída passa de `--max-chars`, os itens de menor ranking são cortados (no `brief`, os testes primeiro, com `+K testes omitidos` na seção) e a última linha diz `+N omitidos`, somando as seções. A ordenação é estável (ranking e depois `qname`).

### 8.4 Deltas por task

Com `--task` e uma chamada anterior do mesmo comando e alvo, `brief` e `query impact` devolvem por padrão só os itens novos ou alterados desde a última chamada, mais a linha `inalterados: N`. Os itens já vistos ficam em `session_seen`. `--full` devolve tudo; `--since-last` força o delta mesmo fora de `--task`.

### 8.5 Algoritmo do `brief`

1. Tokenizar o pedido: minúsculas, sem acentos, separado por não letras, sem stopwords (listas pt e en embutidas).
2. Casar cada token com `word`: igualdade exata; alias de nota `glossario`; prefixo de pelo menos 5 caracteres (marcado `~`).
3. Âncoras = palavras casadas, não `esqueleto`, ordenadas por `especificidade × centralidade`, no máximo 5.
4. Símbolos candidatos = símbolos com ocorrência de alguma âncora. Pontuação = soma, por âncora, do peso da palavra × bônus do slot (`object` 1,0; `action` 0,7; `role` 0,2). Expandir um salto por `CHAMA`, `RECEBE` e `RETORNA` com fator 0,5. Retornar os 15 primeiros.
5. Montar as seções: âncoras, símbolos, contorno (callers e callees dos 5 melhores), padrão (verbos, papéis e camadas da âncora principal), escritores dos atributos tocados, notas (de qualquer nó listado ou âncora de palavra), cobertura.

## 9. Integração com o harness e MCP

O CLI nunca conversa com a IA. O harness conversa com o CLI, instruído por texto e, opcionalmente, por MCP. Não há chamada de API de provedor em nenhum ponto.

### 9.1 `init <harness>`

Ordem de execução, idempotente em cada passo:

1. Resolver a raiz git e `COMMON`; criar `$COMMON/memo/`.
2. Gravar `memo.toml` com as sementes, se ausente. Nunca sobrescrever um existente.
3. Indexar a base a partir de `main_branch` (`--ref`), mesmo que a branch atual seja outra.
4. Chamar o adaptador do harness (abaixo).

`--print` mostra o que seria escrito e não escreve nada. Rodar `init` duas vezes não duplica nada.

### 9.2 Adaptadores

Cada harness implementa a trait `HarnessAdapter { id, detect, install, uninstall, doctor }`. Os adaptadores gravam até dois artefatos:

- **Instruções:** um bloco no arquivo de instruções do harness, entre `<!-- memo:begin -->` e `<!-- memo:end -->`. Reinstalar substitui só o conteúdo entre os marcadores.
- **MCP** (com `--mcp`): o registro do servidor `memo serve` na config MCP do harness.

| Harness | Arquivo de instruções | Config MCP | Situação |
| --- | --- | --- | --- |
| `claude` | `CLAUDE.md` | `.mcp.json` | a confirmar na documentação atual |
| `codex` | `AGENTS.md` | config do codex | a confirmar na documentação atual |
| `agy` | desconhecido | desconhecido | não implementar até haver formato documentado |

Os formatos de config mudam rápido. O agente implementador deve ler a documentação vigente de cada harness antes de escrever o adaptador e registrar no PR o link e a data consultada. Um formato desconhecido gera erro claro (exit 1), nunca um palpite.

### 9.3 Bloco de instruções instalado

O conteúdo é fixo, curto e versionado com o binário:

```markdown
<!-- memo:begin -->
## memo (memória estrutural)
- Antes de começar uma tarefa: `memo brief "<pedido>"`. Leia só os trechos `caminho:Linicio-Lfim` indicados.
- Antes de alterar a assinatura ou o comportamento de um símbolo: `memo query impact <símbolo>`.
- `~` marca relação aproximada. Resultado vazio não prova ausência: confirme com busca no código.
- Ao terminar, registre só conhecimento durável que o código não mostra, com `memo note add`: uma afirmação por nota, ancorada no símbolo mais específico. Não registre o que o diff já mostra.
- Use `--task <id>` em toda chamada durante uma tarefa.
<!-- memo:end -->
```

### 9.4 Servidor MCP

`memo serve` fala MCP por stdio e chama as mesmas funções do núcleo que o CLI. Quatro ferramentas, com descrições de no máximo duas linhas cada, para gastar pouco contexto:

| Ferramenta | Equivale a |
| --- | --- |
| `brief(text, task?)` | `memo brief` |
| `query(kind, target, depth?, task?)` | `memo query <kind>` |
| `note_add(tipo, ancora, texto, task?, confirm?)` | `memo note add` |
| `status(task?)` | `memo status` |

A saída é o mesmo texto compacto da seção 8.3, e erros usam `isError`. O agente implementador deve verificar o estado atual do SDK MCP para Rust (por exemplo, `rmcp`) antes de adotá-lo; se não estiver estável, implementar o mínimo de JSON-RPC por stdio à mão.

## 10. Eventos e hooks

Eventos são fatos imutáveis gravados na tabela `event`, em lote por execução de indexação (`batch`). No MVP não há hooks executáveis externos: o único efeito dos eventos é mudar o estado de notas (seção 7.3) e alimentar o resumo de `status`.

| Evento | Quando |
| --- | --- |
| `SYMBOL_CREATED` | `qname` novo, sem par de renomeação |
| `SYMBOL_DESTROYED` | `qname` sumiu, sem par de renomeação |
| `SYMBOL_SIGNATURE_CHANGED` | `sig_hash` mudou |
| `SYMBOL_BODY_CHANGED` | `body_hash` mudou e `sig_hash` não |
| `SYMBOL_RENAMED` | um `qname` sumiu e outro surgiu no mesmo lote com `sig_hash` e `body_hash` iguais, mesmo `kind` e pareamento único (o `payload` guarda `{de, para}`) |
| `WORD_CREATED` e `WORD_DESTROYED` | cascata: primeira e última ocorrência da palavra no grafo |
| `NOTE_ADDED`, `NOTE_CONFIRMED`, `NOTE_STALE`, `NOTE_ORPHANED`, `NOTE_SUPERSEDED`, `NOTE_PROMOTED` | ciclo das notas |

Regras:

- Mudança só de linha não gera evento. A linha é atributo, não identidade.
- Renomeação com corpo alterado conta como `DESTROYED` mais `CREATED`.
- Cada evento guarda `subject` (`qname`, palavra ou id de nota) e `payload` em JSON com o antes e o depois dos hashes quando houver.
- O resumo impresso ao fim de `index` é uma linha: `eventos: +12 símbolos, ~5 alterados, 1 renomeado, 2 notas stale`.

Hooks de aresta (`LINKED`, `STRENGTHENED`), de hipótese e de runtime estático ficam fora do MVP.

## 11. Marcos e critérios de aceitação

Os marcos 1 a 3 já entregam valor sem nenhuma nota: o agente consulta o grafo e recebe o briefing. Cada marco só fecha com os testes de aceitação abaixo passando.

### 11.1 Marcos, em ordem

1. **Extração e armazenamento.** Parse Python, esquema, indexação incremental, palavras e slots, `query symbol` e `query word`.
2. **Chamadas, tipos e efeitos.** Regras 1 a 7 da seção 5.4, efeitos da seção 5.5, `callers`, `callees`, `impact`, `writers`, `readers`, `type`, `tests`, com `cobertura`.
3. **Pesos e `brief`.** `word_stats`, `brief`, `--max-chars`, deltas por task.
4. **Notas e eventos.** `note *`, `STALE`, `ORFÃ`, renomeação, `derive`, `promote`, `drop`, `status`.
5. **Harness e MCP.** `init` com adaptadores confirmados na documentação, `serve`.
6. **Opcional: `extract` para revisão.** Inventário mecânico de um diff: símbolos tocados, impacto, imports que violam camada (`domain` importando `infra`) ou contexto, assinaturas públicas alteradas × consumidores.

### 11.2 Fixture de teste

Criar um repositório Python em `tests/fixtures/` que siga o layout `app/<context>/{domain,application,infra}`, com: um caso de uso em classe com `execute`, DI por Protocol e implementação concreta na `infra`, um atributo escrito por dois casos de uso, testes no formato `given_..._when_..._then_...`, e um segundo contexto que importa o primeiro de forma proibida. Os testes de aceitação rodam sobre ela e sobre cópias mutadas (renomeação, mudança de assinatura, mudança só de comentário).

### 11.3 Testes de aceitação

| # | Critério | Marco |
| --- | --- | --- |
| A1 | Duas indexações seguidas: a segunda não escreve e não emite eventos | 1 |
| A2 | `CalculateLimitUseCase` gera `calculate` (action), `limit` (object), `usecase` (role) | 1 |
| A3 | Mudar só comentário ou docstring não altera `body_hash` | 1 |
| A4 | `self._repo.save()` com `_repo` anotado como Protocol resolve para o método do Protocol (exata) e liga implementações com `~` | 2 |
| A5 | `callers` e `impact` incluem consumidores tipados só pelo Protocol e declaram `cobertura` | 2 |
| A6 | `writers Invoice.status` lista os dois casos de uso da fixture | 2 |
| A7 | `brief` é determinístico (mesma entrada, mesma saída byte a byte) e palavras `esqueleto` nunca viram âncora | 3 |
| A8 | Saída respeita `--max-chars` e termina com `+N omitidos` quando corta | 3 |
| A9 | Renomear função sem mudar o corpo: `SYMBOL_RENAMED`, nota reancorada, sem `stale` | 4 |
| A10 | Mudar a assinatura: nota vira `stale`; `note recheck` a restaura | 4 |
| A11 | `derive` e edição de um arquivo: só a feature muda, `base.db` fica idêntico | 4 |
| A12 | `note add` em palavra `esqueleto` falha com exit 4; em símbolo inexistente, exit 2 | 4 |
| A13 | `init` duas vezes não duplica o bloco de instruções; `--print` não escreve | 5 |
| A14 | Arquivo com erro de sintaxe não aborta e aparece em `cobertura.arquivos_com_erro` | 1 |

### 11.4 Metas de desempenho e qualidade

Metas iniciais, a revisar na primeira medição em repositório real: indexação completa de 5.000 arquivos Python em até 60 s; reindexação sem mudanças em até 300 ms; `brief` em até 500 ms. Qualidade: `cargo fmt`, `cargo clippy -- -D warnings` e `cargo test` limpos, com golden files (`insta`) para toda saída da seção 8.

### 11.5 Validação de utilidade (manual, em repositório real)

1. As 20 palavras de maior `centralidade` com `especificidade` acima do corte parecem o glossário do domínio? Se vierem `manager`, `helper` e `base`, ajustar `skeleton` e `min_specificity`.
2. O `brief` de uma feature já implementada aponta os arquivos e símbolos que foram de fato tocados?
3. Depois de mudar um símbolo, só as notas dele ficam `stale`?

## 12. Fora do escopo e pontos em aberto

### 12.1 Fora do escopo do MVP

- Outras linguagens (a trait `LanguageExtractor` prevê, nada além de Python é implementado).
- Associações positivas e negativas entre palavras, fila de hipóteses e seus hooks.
- Ciclo de vida por valor (def-use dentro do corpo), complexidade ciclomática e malha arquivo × arquivo.
- Instrumentação ou traces de runtime.
- Skill de capacidade para o fluxo de TASK; a integração por enquanto é o bloco de instruções e o MCP.
- Sincronização de notas entre máquinas e exportação para o repositório.
- Checagens de revisão por padrão (campo monetário com `float`, `except` sem ação, `raise Exception`, N+1); candidatas a evoluir o `extract`.
- Hooks executáveis externos e interface gráfica.

### 12.2 Pontos em aberto, com o padrão adotado

| # | Ponto | Padrão até decisão em contrário |
| --- | --- | --- |
| 1 | Nome definitivo do binário | `memo` |
| 2 | Formato de config do harness `agy` | não implementar; erro claro |
| 3 | Plural e stemming de palavras | sem stemming; sinônimos e plurais por nota `glossario` com `aliases` |
| 4 | Repositório com nomes em português e inglês | sem tradução; resolver por `glossario` |
| 5 | Limiares (`min_specificity` 0,35; 5 âncoras; 15 símbolos; amortecimento 0,85) | valores iniciais, calibrar no repositório real pelo critério 11.5 |
| 6 | Exportar notas para o repositório | fora do MVP; avaliar um `note export` depois |
| 7 | Vários repositórios | um `$COMMON/memo` por repositório, sem índice global |
| 8 | `stale` por mudança só de corpo pode gerar ruído | `stale_on` padrão com assinatura e corpo; relaxar em `memo.toml` se incomodar |
| 9 | Resolução por unicidade de nome (regra 7 da seção 5.4) pode ligar o símbolo errado | sempre marcada `~`; medir a taxa de erro na fixture e no repositório real |
