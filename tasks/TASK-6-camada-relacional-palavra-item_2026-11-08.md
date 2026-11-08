# TASK-6 — Camada relacional palavra ↔ item completa e primeira inferência sobre ela
branch: master
workdir: raiz do repositório (onde está o `Cargo.toml`)
created_at: 2026-11-08

## Status
- Estado: review (implementado e testado; falta a revisão de código)
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

### Fora do escopo
- Reestruturar o código para refletir os conceitos (item, linha, cluster, palavra ↔ palavra como tipos e módulos próprios). É a task seguinte, depois que a base e a inferência mínima provarem o desenho.
- Ranking do nível por palavra pelas pontes e pela representatividade.
- Resolução de chamadas ambíguas por evidência (continua como está).
- Identificadores soltos (atribuição, `isinstance`, retorno de nome); cluster como item.
- Outras linguagens.
- Medições em repositórios reais como critério de projeto (servem depois, como verificação).

---

## 4. Critérios de Aceitação (rascunho)
Todos verificados com fixtures sintéticas, sem repositório real.

- [x] **Cobertura de uso:** para toda `raw_ref`, as palavras de `text` e `receiver` estão entre as ocorrências do símbolo dono (teste de propriedade)
- [x] **Cobertura de tipo:** para todo símbolo com `type_text`, as palavras do tipo estão entre as ocorrências do símbolo (teste de propriedade)
- [x] **Independência da resolução:** a mesma fixture com `resolved` em 0, 1 e 2 gera o mesmo conjunto de ocorrências
- [x] **Chamada encadeada e tipo sem aresta:** uma fixture com `Classe(arg).metodo()` e uma anotação `x: Classe` tem a palavra `classe` ligada ao símbolo que as contém, sem nenhuma aresta
- [x] as estatísticas, pontes, `brief` e `query word` não mudam com as fontes novas (goldens atuais seguem verdes, sem revisão)
- [x] `impact` de um alvo cujo nome só aparece em chamada encadeada ou anotação de tipo lista esses itens no nível por palavra, com `~`, e não lista nada que já esteja nas arestas diretas (fixture sintética)
- [x] o nível por palavra vem separado das arestas verificadas e respeita `--max-chars` e o `+N omitidos`
- [x] `cobertura` informa a camada relacional separada da estática (golden `insta`)
- [x] mesma árvore e mesma config geram a mesma saída byte a byte
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas

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

---

## 7. Estratégia de Testes
escritor: test-analyst (preenchido em TEST_ANALYSIS, depois da DEFINITION)

---

## 8. Dependências
- tasks predecessoras: TASK-4
- módulos | serviços externos: nenhum
- flags | configurações: a definir (§9 Q2)

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
- Aberta Q6: o nível por palavra do `impact` usa só as palavras do nome do alvo ou também as do qualificado (módulo)? Menor mudança: só o nome, e o qualificado desempata se houver ambiguidade de alvo.

### Premissas
- [DEFINITION] O volume de `occurrence` vai crescer; medir o custo no gerador sintético de `docs/medicoes.md`, sem bloquear o aceite.

### Plano de Execução (rascunho)
1. DEFINITION: confirmar Q6, N_PARTITE sobre as duas camadas, critérios finais — depende de: NONE
2. TEST_ANALYSIS: §7 — depende de: 1
3. ENGINEERING, nesta ordem: (a) emitir `ref` e `type` em `index.rs`, com testes de propriedade; (b) conferir os pontos de `representativeness.rs` e `query.rs` contra a fonte; (c) nível por palavra no `impact` (`query.rs:688`); (d) rodapé de cobertura; (e) spec e README — depende de: 2
4. CODE_REVIEW — depende de: 3
5. Depois: task de reestruturação do código pelos conceitos (item, linha, cluster, palavra ↔ palavra) e do ranking do nível por palavra.

### Code Review
(vazio)

## 10. Log de Decisões e Alterações

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
