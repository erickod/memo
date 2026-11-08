# TASK-6 — Camada relacional palavra ↔ item completa e primeira inferência sobre ela
branch: feat/task-6-completion (primeira rodada: master, PR #1)
workdir: raiz do repositório (onde está o `Cargo.toml`)
created_at: 2026-11-08

## Status
- Estado: DEFINITION (segunda rodada; a primeira foi mergeada no PR #1 sem revisão de código, ver §10)
- Owner: Erick Duarte
- ROLE requerida: eng (começa em DEFINITION)

> Rascunho autônomo. Leia `tasks/README.md`, `README.md` e `docs/`. As decisões abertas estão no §9.

---

## 1. Objetivo
Garantir, por construção do índice, que toda palavra que aparece em um item do código vira uma relação palavra ↔ item, em qualquer repositório, sem depender de resolução, de linguagem ou de padrão de projeto; e usar essa base na primeira inferência: o `impact` passa a listar, além das arestas verificadas, os itens que contêm o nome do alvo (marcados `~`). Caminho de menor mudança: sem tabela nova, sem reestruturar o código. (REF: DOR#CRITERIA)

---

## 2. Contexto
- **Modelo conceitual do motor, em duas camadas:**
  - **Relacional:** palavra ↔ item. Item é qualquer unidade endereçável: símbolo (função, classe, método, módulo), arquivo, linha, import, ponto de chamada, cluster e a própria palavra (palavra ↔ palavra). É incidência, não inferência: existe sempre que a palavra está no item.
  - **Inferência:** o que se deduz da camada relacional (arestas `CHAMA`/`CRIA`/`RECEBE`, resolução de ambiguidade, pontes, coerência, ranking, `impact` por palavra). Pode mudar depois sem reconstruir a base.
- **Princípio:** se a camada relacional estiver bem construída, só a inferência precisa de ajuste depois.
- **Estado atual** (`store.rs`, `sources.rs`, `index.rs`): `occurrence(owner_kind, owner_id, source, pos)` tem quatro fontes: `name` (símbolo), `path` (arquivo), `import` e `test_doc` (símbolo de teste). O texto das referências (`raw_ref`: chamadas, receptor, `RECEBE`, `RETORNA`...) e o `type_text` dos símbolos não geram ocorrências.
- **Consequência:** uma palavra que está no código mas só em chamada encadeada ou em anotação de tipo não existe como relação. Hoje a única forma de chegar a esse item é a aresta estática, e quando a resolução não chega lá (`raw_ref.resolved = 0`), o item some do `impact`.
- **Linha já existe, só não como conceito no código:** `raw_ref.line`, `edge.line` e `symbol.line_start/line_end` já dão a linha de cada uso e de cada símbolo. Nenhuma tabela ou coluna de linha é criada nesta task; a reestruturação do código para refletir "linha" como item fica para depois (§3, fora do escopo).
- **Mecanismo de menor mudança:** `rebuild_doc_words` (`index.rs`) já grava `test_doc` com `symbol_id = NULL`. Várias consultas de estatística e do `brief` filtram por `symbol_id` (`stats.rs:62`, `brief.rs:147`, `brief.rs:407`, `query.rs:373`, `394`, `631`). As fontes novas gravadas do mesmo modo ficam invisíveis para elas, sem alterar nada. Pontos a conferir na ENGINEERING: `representativeness.rs:55` e `query.rs:303-307` juntam por `owner_id` e precisam ser checados contra `owner_kind`/`source`.
- A contagem `não resolvidas` descreve a camada de inferência. Não mede a completude da camada relacional.
- **Segunda rodada (depois do PR #1):** na primeira, as palavras de `ref` ficaram com o **símbolo** como dono, e o `por palavra` casa só as palavras de nome do alvo, que para um método são só o último segmento (`index.rs:294`). Assim, `impact a.Alpha.run` lista qualquer `.run()` do repositório (`Beta(1).run()`, `obj.run()`) e não diz em que linha está o uso. Como `run` e `alpha` ficam penduradas no símbolo, não dá para saber se vieram do mesmo uso. A revisão do merge está no §10.
- Stack: Rust, SQLite, `tree-sitter-python`.

---

## 3. Escopo

### Dentro do escopo
**Camada relacional (base):**
- Toda `raw_ref` (qualquer `kind`) emite ocorrências das palavras de `text` e `receiver`, `source = 'ref'`, dono no símbolo de origem, `symbol_id = NULL`.
- Todo `type_text` de símbolo emite ocorrências, `source = 'type'`, mesmo modo.
- As ocorrências não dependem do estado `resolved`.
- A linha do uso é recuperada pelo join com `raw_ref.line`; nenhuma coluna nova.

**Inferência (primeira consumidora):**
- `impact` ganha o nível por palavra: itens cujas ocorrências `ref`/`type` contêm as palavras do nome do alvo e que não estão nas arestas diretas. Marca `~`, ordenados por `qname` (ranking por pontes fica para depois). Resultado separado do d1/d2 verificado.
- Rodapé de `cobertura` separa a camada relacional da estática.

**Documentação:** spec (§5.2 e o `impact`) e README.

**Segunda rodada (decisões D7 a D11, §9):**
- *Relacional:* o ponto de uso vira item. `raw_ref` ganha `id` explícito; as palavras de `ref` passam a ter o uso como dono (`owner_kind = 'ref'`, `owner_id = raw_ref.id`), com `symbol_id` nulo. As palavras de `type` continuam com o símbolo como dono. `schema_version` sobe de 3 para 4.
- *Relacional:* reconstrução incremental: só os usos e os tipos dos arquivos reindexados são regravados (hoje se refaz o repositório inteiro a cada índice).
- *Inferência:* o `por palavra` do `impact` passa a ser por uso: usos cujo texto contém as palavras do nome do alvo, cada um com a linha e um nível de evidência:
  - `forte`: o mesmo uso contém as palavras de uma classe da família do dono do alvo;
  - `média`: as palavras de uma classe da família estão em outro uso ou no tipo do mesmo símbolo;
  - `só nome`: nenhuma evidência; aparece só como contagem (`+N só pelo nome`), e a lista completa sai no JSON.
  A família é a classe dona, as bases e as subclasses (arestas `ESPECIALIZA` exatas, nos dois sentidos) e os Protocols que ela implementa. Alvo que não é método (função, classe, módulo) não tem família; a evidência é o módulo (D13): o uso é `forte` se está no mesmo módulo do alvo ou se o arquivo do uso importa o alvo ou o módulo dele (`import.target` igual ao `qname` do alvo ou ao módulo); sem isso, é `só nome`. Não existe `média` para esses alvos.
- *Inferência:* o campo `nao_resolvidas_por_palavra` sai da `cobertura` e do JSON; a completude da camada relacional passa a ser garantida por teste de propriedade.

### Fora do escopo
- Reestruturar o código para refletir os conceitos (item, linha, cluster, palavra ↔ palavra como tipos e módulos próprios). É a task seguinte, depois que a base e a inferência mínima provarem o desenho.
- Ranking do nível por palavra pelas pontes e pela representatividade.
- Resolução de chamadas ambíguas por evidência (continua como está).
- Identificadores soltos (atribuição, `isinstance`, retorno de nome); cluster como item.
- Outras linguagens.
- Medições em repositórios reais como critério de projeto (servem depois, como verificação).

---

## 4. Critérios de Aceitação
Todos verificados com fixtures sintéticas, sem repositório real.

### Primeira rodada (PR #1)
Revistos na revisão do merge (§10, 2026-11-08): os desmarcados não tinham teste que verificasse o que dizem e passam para a segunda rodada.

- [ ] **Cobertura de uso:** para toda `raw_ref`, as palavras de `text` e `receiver` estão entre as ocorrências do símbolo dono (teste de propriedade). *Só havia testes de exemplo; reescrito em C1.*
- [x] **Cobertura de tipo:** para todo símbolo com `type_text`, as palavras do tipo estão entre as ocorrências do símbolo. *Só exemplo; a propriedade entra em C2.*
- [ ] **Independência da resolução:** a mesma fixture com `resolved` em 0, 1 e 2 gera o mesmo conjunto de ocorrências. *O teste comparava duas chamadas diferentes; reescrito em C3.*
- [x] **Chamada encadeada e tipo sem aresta:** uma fixture com `Classe(arg).metodo()` e uma anotação `x: Classe` tem a palavra `classe` ligada ao símbolo que as contém, sem nenhuma aresta
- [x] as estatísticas, pontes, `brief` e `query word` não mudam com as fontes novas (goldens atuais seguem verdes, sem revisão)
- [x] `impact` de um alvo cujo nome só aparece em chamada encadeada ou anotação de tipo lista esses itens no nível por palavra, com `~`, e não lista nada que já esteja nas arestas diretas (fixture sintética)
- [ ] o nível por palavra vem separado das arestas verificadas e respeita `--max-chars` e o `+N omitidos`. *Sem teste de `--max-chars`; reescrito em C8.*
- [ ] `cobertura` informa a camada relacional separada da estática (golden `insta`). *O número repetia o total; substituído por D10.*
- [ ] mesma árvore e mesma config geram a mesma saída byte a byte. *Sem teste do `por palavra`; reescrito em C9.*
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas

### Segunda rodada
- [ ] **C1, cobertura de uso (propriedade):** para toda `raw_ref` de uma fixture variada (chamada simples, encadeada, receptor `self`, tipo recebido e devolvido, resolvida, aproximada e não resolvida), as palavras de `callee_text` e `receiver` são exatamente as ocorrências `ref` com dono `('ref', raw_ref.id)`
- [ ] **C2, cobertura de tipo (propriedade):** para todo símbolo com `type_text`, as palavras do tipo são exatamente as ocorrências `type` do símbolo
- [ ] **C3, independência da resolução:** a mesma fixture com as `raw_ref` forçadas a `resolved` 0, 1 e 2 gera o mesmo conjunto de ocorrências `ref` e `type`
- [ ] **C4, incremental:** reindexar depois de alterar um arquivo regrava só os usos e tipos desse arquivo (os `raw_ref.id` e ocorrências dos outros arquivos ficam iguais) e o resultado é igual ao de um índice do zero
- [ ] **C5, evidência:** para o alvo `Alpha.run`: `Alpha(1).run()` e `SubAlpha().run()` (subclasse) e `p.run()` com `p: RunnerProtocol` implementado por `Alpha` são `forte` ou `média` conforme o mesmo uso ou o mesmo símbolo; `Beta(1).run()` e `obj.run()` são `só nome`
- [ ] **C5b, alvo sem família:** para a função `m.helper`: `helper()` no próprio módulo `m`, e `from m import helper; helper()` ou `import m; m.helper()` em outro arquivo, são `forte`; `helper()` num arquivo que não importa `m` nem `m.helper` é `só nome`
- [ ] **C6, linha:** cada item `forte` e `média` traz a linha do uso
- [ ] **C7, só nome:** os usos `só nome` aparecem no texto só como `+N só pelo nome`, e no JSON como lista
- [ ] **C8:** o `por palavra` respeita `--limit`, `--max-chars` e o `+N omitidos`, não repete o que as arestas já cobrem, e some com `--no-tests` quando o uso é de teste
- [ ] **C9:** mesma árvore e mesma config dão a mesma saída do `impact`, byte a byte, entre duas indexações do zero
- [ ] **C10:** `nao_resolvidas_por_palavra` não existe mais na `cobertura` nem no JSON; goldens revistos um a um
- [ ] **C11:** estatísticas, pontes, `brief` e `query word` não mudam (goldens desses comandos sem revisão)
- [ ] **C12:** custo medido no gerador de `docs/medicoes.md` (índice completo, sem mudança e com um arquivo alterado), antes e depois, registrado no §10; não bloqueia o aceite
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas

---

## 5. Restrições e Regras
- técnicas: princípios 1 a 7 da spec §2; `memo-core` não depende de `clap` nem de MCP; a camada relacional não importa nada da de inferência.
- negócio: o `memo` entrega fatos, não julga código.
- dependências obrigatórias: TASK-4 concluída (`done`).
- regra de dados: o banco é derivado e descartável; mudança de esquema sobe `schema_version` e recria.

---

## 6. Impacto em Dados
- [x] Nenhuma alteração de schema (`occurrence` já tem `owner_kind`, `source` e aceita `symbol_id` nulo)
- [x] Uso de entidades existentes
- [ ] Leitura apenas
- [x] Escrita controlada (novas linhas em `occurrence`; o banco é derivado, então `schema_version` sobe para reindexar)
- Segunda rodada: **alteração de schema** (`raw_ref.id INTEGER PRIMARY KEY`, `owner_kind = 'ref'` em `occurrence`, índice para buscar usos por dono, se a medição pedir); `schema_version` 3 → 4, o banco é descartado e recriado; notas intactas.

---

## 7. Estratégia de Testes
escritor: test-analyst (preenchido em TEST_ANALYSIS, depois da DEFINITION)

---

## 8. Dependências
- tasks predecessoras: TASK-4
- módulos | serviços externos: nenhum
- flags | configurações: nenhuma nova

---

## 9. Observações

### Decisões (grill-me)
- Decidido (2026-11-08, humano): a base palavra ↔ item vale por construção, não por conferência posterior; a camada de inferência fica separada e ajustável depois.
- Decidido: exemplos de repositórios reais não definem a task; vale a propriedade geral.
- Decidido (humano): o motor tem duas partes conceituais, relacional (palavra ↔ item; itens: símbolo, arquivo, palavra, função, classe, linha, método, cluster...) e inferência. Com a relacional bem construída, só a inferência é ajustada depois.
- Decidido (humano): a inferência entra nesta task, no menor passo possível; a reestruturação do código para refletir os conceitos vem depois.
- Decidido (humano): caminho mais rápido e coeso, com o menor escopo de mudança.
- Decidido (consequência do critério acima, Q1): linha não ganha estrutura nova; já existe em `raw_ref.line`, `edge.line` e no intervalo do símbolo. Cluster fica fora.
- Decidido (consequência, Q2): as fontes novas não entram nas estatísticas nem nas pontes; gravam com `symbol_id = NULL`, como `test_doc`.
- Decidido (consequência, Q3): só chamadas (`raw_ref`) e tipos (`type_text`).
- Decidido (Q4): o `impact` por palavra é a inferência desta task.
- Adiada (Q5): palavra ↔ palavra como relação própria; segue derivada, até a reestruturação.
- ~~Aberta Q6~~: fechada por D7.

**Segunda rodada (2026-11-08, perguntas uma a uma ao humano):**
- Decidido D7 (Q6, humano, recomendação aceita): o ponto de uso (`raw_ref`) vira item da camada relacional, e o `por palavra` classifica cada uso por evidência (`forte`, `média`, `só nome`) em vez de filtrar. Revê, com aval explícito, o "menor passo" (D de 2026-11-08) e a Q1 (a linha continua sem coluna nova: vem do item uso, por `raw_ref.line`).
- Decidido D8 (humano, recomendação aceita): os usos `só nome` aparecem só como contagem `+N só pelo nome`; o JSON traz a lista.
- Decidido D9 (humano, recomendação aceita): a família de evidência é a classe dona, as bases e as subclasses (`ESPECIALIZA` exata, nos dois sentidos) e os Protocols que ela implementa.
- Decidido D10 (humano, recomendação aceita): `nao_resolvidas_por_palavra` sai da `cobertura` e do JSON; a completude vira teste de propriedade (C1, C2).
- Decidido D11 (humano, recomendação aceita): a reconstrução das palavras de uso fica incremental, com medição antes e depois.
- Decidido D12 (humano): plano aprovado; fluxo DEFINITION → TEST_ANALYSIS → ENGINEERING (`test:` red, depois `feat:` green, sem `Co-Authored-By`, cada alteração aprovada antes) → medição → docs.
- Decidido D13 (humano; a recomendação era tratar como `forte` só pelo nome, e foi recusada): alvo que não é método (função, classe, módulo) usa o módulo como evidência. O uso é `forte` se está no mesmo módulo do alvo ou se o arquivo do uso importa o alvo ou o módulo dele; sem isso, é `só nome`. Os imports são gravados por arquivo (`import.file_id`), então a evidência é do arquivo do uso, não do símbolo.

### Premissas
- [DEFINITION] O volume de `occurrence` vai crescer; medir o custo no gerador sintético de `docs/medicoes.md`, sem bloquear o aceite.
- [DEFINITION, 2ª rodada] Mudar o dono de `ref` de símbolo para uso não altera estatísticas, pontes nem `brief`, porque todas as consultas desses módulos já filtram por `source` ou por `symbol_id` (conferido no PR #1, `6895972`). A ENGINEERING confere de novo, agora contra `owner_kind = 'ref'`.

### Plano de Execução
Primeira rodada: passos 1 a 3 feitos no PR #1; passo 4 feito como revisão do merge (§10).

Segunda rodada, na branch `feat/task-6-completion`:
1. DEFINITION: este documento (D7 a D13, C1 a C12) e a aprovação única — depende de: NONE
2. TEST_ANALYSIS: §7 com os testes de C1 a C11 — depende de: 1
3. ENGINEERING, nesta ordem, cada passo com `test:` red e depois `feat:`/`fix:` green: (a) `raw_ref.id` e `schema_version` 4; (b) palavras de `ref` com o uso como dono; (c) reconstrução incremental; (d) `por palavra` por uso, com evidência (família para método, módulo para os demais) e linha; (e) `+N só pelo nome` no texto e lista no JSON; (f) remover `nao_resolvidas_por_palavra` — depende de: 2
4. Medição C12 — depende de: 3
5. Spec, README, `docs/funcionalidades.md`, `docs/limitacoes.md`, `tasks/README.md` — depende de: 3
6. CODE_REVIEW — depende de: 5
7. Depois: task de reestruturação do código pelos conceitos (item, linha, cluster, palavra ↔ palavra) e do ranking do nível por palavra.

### Code Review
(vazio)

## 10. Log de Decisões e Alterações

### [2026-11-08] — Revisão do merge do PR #1 e reabertura em DEFINITION
- Tipo: revisão e decisão
- Contexto: o PR #1 (`8835ee4`, commits `12359d6` a `819c122`) foi mergeado em `master` com a TASK em `review`, sem revisão de código; o `tasks/README.md` ficou com a TASK-6 como `draft`, "Nada ainda". A revisão foi feita commit a commit, na branch `feat/task-6-completion`.
- Achados:
  - 🔴 O `por palavra` casa só as palavras de nome do alvo, que para um método são só o último segmento (`index.rs:294`): `impact a.Alpha.run` lista qualquer `.run()`. O teste existente não pega o caso porque o símbolo negativo não contém `run`.
  - 🟡 `nao_resolvidas_por_palavra` conta as não resolvidas cujo símbolo dono tem *alguma* ocorrência `ref`; como toda `raw_ref` com texto gera ocorrência, repete o total (golden: `2 não resolvidas (2 por palavra)`).
  - 🟡 Critérios marcados sem teste que os verificasse: cobertura de uso como propriedade, independência da resolução na mesma fixture, `--max-chars` no `por palavra`, determinismo do `por palavra` (desmarcados no §4).
  - 🟡 `line` nulo no `por palavra`, embora `raw_ref.line` exista.
  - 🟡 `rebuild_use_words` apaga e regrava as palavras de uso do repositório inteiro a cada índice; a medição não cobriu um arquivo alterado.
  - 🟡 Processo: os `feat:` vieram antes dos `test:` (sem fase red) e levam `Co-Authored-By`, contra a regra do `tasks/README.md`. O histórico mergeado não será reescrito.
  - 🟢 O filtro do `por palavra` em `hide.rs` é redundante com `s.is_test = 0` do SQL, mas também cobre arquivos de teste pelo caminho; mantido.
- Decisão tomada: D7 a D13 (§9); novos critérios C1 a C12 e C5b (§4); plano da segunda rodada (§9).
- Impacto: `Estado: DEFINITION`; `branch: feat/task-6-completion`; `schema_version` vai a 4.
- Não verificado: o efeito de `owner_kind = 'ref'` em consultas que juntam por `owner_id` sem `owner_kind` (a conferir na ENGINEERING); o custo da busca da família por uso.

### [2026-11-08] — Implementação (relacional e `impact` por palavra)
- Tipo: implementação
- Feito: `use_words` em `sources.rs`; `rebuild_use_words` em `index.rs` (fontes `ref` e `type`, `symbol_id` nulo, independentes de `resolved`); `by_word` em `query::impact`, `hide`, `delta` e `render` (seção `por palavra`, sempre `~`, sem repetir o que as arestas já cobrem); spec §5.2 e §8.2; testes em `tests/use_words.rs`; 311 testes, `fmt` e `clippy` verdes.
- Achado: duas consultas de `bridges.rs` (`symbol_words`, `symbol_ids_with_word`) não filtravam por `source` e deixaram as fontes novas vazarem para a ponte `teste` (um golden de `query word` mudou). Corrigido com `source IN ('name','test_doc')`; `tags.rs`, `representativeness.rs` e `query.rs:303-307` já filtravam.
- Achado: a chamada encadeada `Classe(arg).metodo()` já gera `CRIA` para a classe; a lacuna do `impact` é o **método** ambíguo, que agora aparece em `por palavra`. O alvo de nível classe usado só como tipo já tem `RECEBE`/`RETORNA`.
- Feito depois: `cobertura` ganhou `(N por palavra)` ao lado das não resolvidas (campo `nao_resolvidas_por_palavra`; aparece só quando há não resolvidas); `schema_version` 2 → 3 (sem isso, um índice existente não ganharia as palavras novas até os arquivos mudarem; achado na medição); README e `docs/funcionalidades.md`.
- Medição (gerador sintético de `docs/medicoes.md`, 5.000 arquivos, `--release`, uma execução cada, antes → depois): índice completo 1,45 s → 1,85 s (+28%); banco 29,5 → 35,1 MiB (+19%); sem mudança 0,04 s → 0,04 s; `brief` 0,66 s → 0,68 s (dentro do ruído de uma execução). Este gerador é leve; num repositório real o custo pode diferir.
- Não feito: `por palavra` sem número de linha (`line` fica nulo); medição em repositório real; revisão de código.
- Limitação conhecida: nome de uma palavra só (`run`, `get`) casa muitos símbolos; o ranking pelas pontes é a task seguinte.

### [2026-11-08] — Criação do rascunho da TASK-6
- Tipo: criação
- Contexto: relatório de integração das skills mostrou o `impact` perdendo consumidores que o `grep -w` achava; a discussão separou o motor em camada relacional (palavra ↔ item) e de inferência.
- Decisão tomada: a task cobre a camada relacional e a primeira inferência (`impact` por palavra), no menor escopo; a reestruturação conceitual do código fica para depois.
- Impacto: `Estado: draft`.
- Não verificado: o impacto no tamanho do banco e nas estatísticas de palavra.
