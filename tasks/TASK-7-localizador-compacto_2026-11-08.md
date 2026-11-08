# TASK-7 — O memo como localizador: notas inteiras, saída compacta, trechos para ler e relações que faltam
branch: feat/task-6-completion (mesmo PR da TASK-6, por decisão do humano)
workdir: raiz do repositório (onde está o `Cargo.toml`)
created_at: 2026-11-08

## Status
- Estado: DEFINITION
- Owner: Erick Duarte
- ROLE requerida: eng

> TASK autônoma. Leia `tasks/README.md`, `README.md` e `docs/`. As decisões estão no §9.

---

## 1. Objetivo
Fazer o `memo` funcionar como **localizador**: a resposta deve ser curta, mostrar o texto inteiro das notas a quem usa o MCP e apontar os trechos de código que vale a pena ler. Junto, fechar duas lacunas de relação: leituras de atributo pela classe e os membros de um Enum. E fazer `query word` aceitar palavras compostas. (REF: DOR#CRITERIA)

---

## 2. Contexto
- **Origem:** relatório de uso real do `memo` por agentes num repositório Python privado (2026-11-08). Do relatório entram só os problemas do próprio `memo`, gerais para qualquer repositório Python. Ficam fora os adaptadores de framework, schema de banco, infraestrutura e o protocolo de benchmark (§3).
- **Observação principal do relatório:** consultar o `memo` e depois ler os trechos indicados saiu mais barato e deu respostas melhores do que usar só o `memo`. Um agente que só consultava o grafo continuou fazendo consultas atrás de informação que três leituras curtas resolveriam. O `memo` é um localizador: a resposta deve levar o agente ao trecho certo, não a mais consultas.
- **Estado atual:**
  - **MCP** (`crates/memo-mcp/src/lib.rs`): quatro ferramentas (`brief`, `query`, `note_add`, `status`). No `brief` e no `symbol`, a nota aparece como resumo de 120 caracteres (`notes.rs:14`, `SUMMARY_CHARS`), e quem usa só o MCP não tem como ler o texto inteiro.
  - **Testes:** aparecem por padrão. `--no-tests` esconde (TASK-2).
  - **Listas longas:** o teto padrão é `--max-chars 6000`. O corte vira `+N omitidos` sem dizer onde estão os itens omitidos. O `symbol` lista todas as importações. `--full` já é usado pelo delta por task.
  - **Leitura de atributo:** o extrator só gera `LE` para cadeias `self.x` (`extract.rs:682-688`). `Classe.attr` numa expressão não vira relação, e `readers` dá vazio.
  - **Enum:** o `symbol` de uma classe Enum mostra as arestas, não os membros.
  - **`query word`:** o grafo guarda as palavras separadas (`order`, `payments`), então `query word order_payments` dá `not found`.
- Stack: Rust, SQLite, `tree-sitter-python`.

---

## 3. Escopo

### Dentro do escopo
1. **Notas pelo MCP (D1):** ferramentas `note_show` e `note_list` no MCP, com o texto inteiro.
2. **Testes ocultos por padrão (D2):** em todas as consultas e no `brief`. `--tests` mostra, `--no-tests` continua aceito e `query tests` sempre mostra.
3. **Listas longas (D3):** uma seção com mais de 10 itens mostra os 10 primeiros e resume o resto numa linha por módulo com contagem. No `symbol`, as importações saem só como contagem por módulo. `--all` desliga o agrupamento.
4. **Seção `ler:` (D4)** no `symbol` e no `brief`, mais as instruções do servidor MCP recomendando o fluxo localizar → ler → consultar de novo só o que faltar.
5. **Leitura de atributo pela classe (D5):** `X.attr` lido em qualquer expressão gera `LE`. É exato quando `X` resolve para uma classe do repositório que tem `attr`, e aproximado (`~`) quando não resolve com certeza.
6. **Membros de Enum (D6):** linha `membros:` no `symbol` de classes que herdam de `Enum`, `IntEnum`, `StrEnum` ou `Flag`.
7. **`query word` composta (D7):** tira os `_` do começo e do fim, divide em `_` e `.` e cruza as partes.
8. Spec, README, `docs/funcionalidades.md`, `docs/limitacoes.md`, instruções do bloco do `memo init`.

### Fora do escopo
- Sub-regiões de símbolos grandes, `query at <arquivo>:<linha>`, termos compostos no `brief`, modo de visão geral e o aviso `possivelmente incompleto:`. Ficam para uma TASK seguinte.
- Adaptadores de framework (rotas web, carregamento dinâmico de jobs), schema de banco e migrations, manifests de infraestrutura, escritas via construtores de query de ORM.
- Subconjuntos de Enum calculados por métodos, e resumo de predicados de query.
- O protocolo de benchmark com agentes.

---

## 4. Critérios de Aceitação
Todos verificados com fixtures sintéticas.

- [ ] **C1, notas pelo MCP:** `tools/list` do servidor lista `note_show` e `note_list`; `note_show` de uma nota com 1.200 caracteres devolve o texto inteiro; `note_list` devolve as mesmas linhas de `memo note list`
- [ ] **C2, testes ocultos:** sem flag, `brief` e as consultas `symbol`, `callers`, `callees`, `impact`, `writers`, `readers`, `type` e `word` não listam símbolos de teste e a `cobertura` diz `N testes ocultos`; com `--tests`, listam; `--no-tests` dá a mesma saída que sem flag; `query tests` lista testes sem flag
- [ ] **C3, listas longas:** uma seção com 25 itens mostra 10 e uma linha `+15 em: <módulo> ×n · <módulo> ×m …`, com os módulos ordenados por contagem decrescente e depois pelo nome; com `--all`, mostra os 25; `--max-chars` e `+N omitidos` continuam valendo
- [ ] **C4, importações no symbol:** o `symbol` de um alvo importado por 30 módulos mostra uma linha de contagem por módulo, sem listar cada importação; com `--all`, lista
- [ ] **C5, `ler:` no symbol:** o `symbol` termina com `ler:` contendo o próprio alvo (`alvo`) e até 4 chamadores exatos d1 (`chama o alvo em :L<n>`), no formato `caminho:Lini-Lfim — motivo`; trecho com mais de 60 linhas vira uma janela de 20 linhas ao redor da chamada (chamador) ou do início (alvo); a ordem é a da lista de chamadores
- [ ] **C6, `ler:` no brief:** o `brief` termina com `ler:` contendo os 5 primeiros símbolos de produção da lista (`carrega <palavra>`), com a mesma regra de janela
- [ ] **C7, instruções:** as instruções do servidor MCP e o bloco do `memo init` recomendam localizar, ler os trechos de `ler:` e só então consultar de novo o que faltar
- [ ] **C8, `X.attr` exato:** `Classe.attr` lido numa função, com `Classe` importada e dona de `attr`, faz a função aparecer em `readers Classe.attr` sem `~`
- [ ] **C9, `X.attr` aproximado:** `x.attr` lido com `x` sem tipo resolvível, e `attr` existente em exatamente uma classe do repositório, faz a função aparecer em `readers` com `~`; se `attr` existe em várias classes, aparece com `~` em cada uma; se não existe em nenhuma, não vira relação
- [ ] **C10, Enum:** o `symbol` de `class Status(StrEnum)` com `A = "a"`, `B = auto()` e `C = 3` mostra `membros: A = "a" · B · C = 3`; uma classe que não é Enum não mostra `membros:`; mais de 10 membros seguem a regra de C3
- [ ] **C11, word composta:** `query word order_payments` e `query word order.payments` mostram `interpretado como: order + payments`, a ficha curta de cada parte e os símbolos cujo nome contém as duas partes; `query word __init__` e `query word _order_` tratam a entrada como `init` e `order`; uma entrada sem partes conhecidas continua `not found` (exit 2)
- [ ] **C12:** a mesma árvore dá a mesma saída byte a byte; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas; os goldens alterados são lidos antes de aceitos

---

## 5. Restrições e Regras
- técnicas: princípios 1 a 7 da spec §2; `memo-core` não depende de `clap` nem de MCP; o MCP só chama o CLI.
- negócio: o `memo` entrega fatos, não julga código; o "motivo" de `ler:` é um fato (relação ou palavra), nunca uma avaliação.
- dependências obrigatórias: TASK-6 implementada (rodada 3).
- regra de dados: o banco é derivado; mudança de extração ou esquema sobe `schema_version` (5 → 6) e recria; as notas não são tocadas.

---

## 6. Impacto em Dados
- [ ] Nenhuma alteração de schema
- [x] Uso de entidades existentes (`edge` com `LE`, `symbol`)
- [ ] Leitura apenas
- [x] Escrita controlada (novas arestas `LE`; membros de Enum, se exigirem coluna ou tabela, sobem `schema_version`)

---

## 7. Estratégia de Testes
escritor: test-analyst (preenchido em TEST_ANALYSIS, depois da DEFINITION)

---

## 8. Dependências
- tasks predecessoras: TASK-6
- módulos | serviços externos: nenhum
- flags | configurações: `--tests` (novo), `--all` (novo); `--no-tests` mantido

---

## 9. Observações

### Decisões (grill-me)
- Decidido (humano, 2026-11-08): o escopo são os problemas do próprio `memo` no relatório de uso, gerais para qualquer repositório Python; nada específico do repositório medido nem de busca textual. Entram as fases 1 e 2 do relatório e o item de palavras compostas.
- Decidido D1 (humano, recomendação aceita): o MCP ganha `note_show` e `note_list`, com o texto inteiro; o resumo de 120 caracteres continua em `brief` e `symbol`, sempre com o id.
- Decidido D2 (humano, recomendação aceita): testes ocultos por padrão em todas as consultas e no `brief`, com `N testes ocultos` na cobertura; `--tests` mostra; `--no-tests` é sinônimo do padrão; `query tests` sempre mostra.
- Decidido D3 (humano, recomendação aceita): seção com mais de 10 itens mostra os 10 primeiros na ordem atual e resume o resto por módulo com contagem; no `symbol`, importações só como contagem por módulo; `--all` desliga o agrupamento, e `--max-chars` continua valendo.
- Decidido D4 (humano, recomendação aceita): `ler:` com regra fixa. No `symbol`: alvo e até 4 chamadores exatos d1. No `brief`: os 5 primeiros símbolos de produção. Trecho com mais de 60 linhas vira janela de 20 linhas. O motivo é um fato (`alvo`, `chama o alvo em :L<n>`, `carrega <palavra>`).
- Decidido D5 (humano): `X.attr` lido gera `LE` também quando `X` não resolve com certeza, marcado `~` (a recomendação era só exato e foi recusada). A ENGINEERING define a resolução aproximada pelo nome do atributo (C9).
- Decidido D6 (humano, recomendação aceita): `membros:` com `NOME = valor` para valor literal (string, número) e só `NOME` nos demais casos; classes que herdam de `Enum`, `IntEnum`, `StrEnum` ou `Flag`.
- Decidido D7 (humano, recomendação aceita): `query word` tira os `_` do começo e do fim, divide em `_` e `.` com o classificador do índice; uma parte dá a ficha normal; várias dão `interpretado como:`, a ficha curta de cada parte e os símbolos que contêm todas.

### Premissas
- [DEFINITION] Agrupar por módulo usa o módulo do arquivo do item (`module_of`).
- [DEFINITION] A detecção de Enum é pela base declarada (nome `Enum`, `IntEnum`, `StrEnum`, `Flag` ou `enum.<nome>`), sem resolver herança indireta. Uma classe que herda de uma base própria derivada de Enum não é detectada (limitação a registrar).
- [DEFINITION] `LE` aproximado pode aumentar o número de arestas; medir no gerador sintético, sem bloquear.

### Plano de Execução
1. DEFINITION: este documento e a aprovação — depende de: NONE
2. TEST_ANALYSIS: §7 — depende de: 1
3. ENGINEERING, em fases, cada uma com `test:` red e depois `feat:` green, um arquivo por commit:
   (a) D1 notas pelo MCP; (b) D2 testes ocultos; (c) D3 listas longas e importações; (d) D4 `ler:` e instruções; (e) D5 `X.attr`; (f) D6 Enum; (g) D7 `query word` composta — depende de: 2
4. Docs e `tasks/README.md` — depende de: 3
5. CODE_REVIEW — depende de: 4

### Code Review
(vazio)

## 10. Log de Decisões e Alterações

### [2026-11-08] — Criação da TASK-7
- Tipo: criação
- Contexto: relatório de uso real do `memo` por agentes. O humano pediu para tratar só os problemas do `memo`, no mesmo PR da TASK-6.
- Decisão tomada: escopo e D1 a D7 (§9), perguntados um a um.
- Impacto: `Estado: DEFINITION`.
- Não verificado: o efeito do `LE` aproximado no volume de arestas; detecção de Enum por herança indireta.
