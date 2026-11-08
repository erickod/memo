# TASK-6 — Camada relacional palavra ↔ item completa e primeira inferência sobre ela
branch: feat/task-6-completion (primeira rodada: master, PR #1)
workdir: raiz do repositório (onde está o `Cargo.toml`)
created_at: 2026-11-08

## Status
- Estado: DEFINITION (terceira rodada: inferência aditiva, depois da medição no `bope` e da CODE_REVIEW com 🔴 R1; ver §9 e §10). A primeira rodada foi mergeada no PR #1 sem revisão.
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
  A família é a classe dona, as bases e as subclasses (arestas `ESPECIALIZA` exatas, nos dois sentidos) e os Protocols que ela implementa (`ESPECIALIZA` exata ou estrutural, D9a). Alvo que não é método (função, classe, módulo) não tem família; a evidência é o módulo (D13): o uso é `forte` se está no mesmo módulo do alvo ou se o arquivo do uso importa o alvo ou o módulo dele (`import.target` igual ao `qname` do alvo ou ao módulo); sem isso, é `só nome`. Não existe `média` para esses alvos.
- *Inferência:* o campo `nao_resolvidas_por_palavra` sai da `cobertura` e do JSON; a completude da camada relacional passa a ser garantida por teste de propriedade.

**Terceira rodada (decisões D17 a D21, §9):**
- *Inferência aditiva:* o `por palavra` lista **todo** item que a primeira rodada listava (símbolos de produção cujas palavras de uso, `ref` ou `type`, contêm as palavras do nome do alvo, menos os cobertos por aresta), agora por uso e com a linha. O nível de evidência é **rótulo e ordem**, nunca filtro. A lista `só nome` à parte e a linha `+N só pelo nome` saem (revoga a D8).
- *Evidência por identificador:* uma classe da família conta só quando o nome dela aparece como identificador inteiro no texto ou no receptor de um uso (`Alpha(`, `Alpha.`, `[Alpha]`); `SubAlpha` não conta como `Alpha`, e `ClearOrderDebtCodeRepository` não conta como `OrderDebtCodeRepository`.
- *Item de tipo:* um atributo cujo tipo anotado contém as palavras do nome do alvo volta como item próprio, com o rótulo `tipo`; a linha é a do atributo, e o nível segue a mesma regra de identificador sobre o texto do tipo.
- *Desempenho:* `impact` de cada um dos 6 alvos da medição em até 500 ms no clone do `bope`; palavras dos usos candidatos carregadas em lote e índice `raw_ref(src, line)`; `schema_version` 4 → 5.
- *Revisão:* R1 (D16: notas `word:` e snapshot de eventos só com fontes de vocabulário) e Y2 (`by_word` dividido em funções).

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
- [x] **C1, cobertura de uso (propriedade):** para toda `raw_ref` de uma fixture variada (chamada simples, encadeada, receptor `self`, tipo recebido e devolvido, resolvida, aproximada e não resolvida), as palavras de `callee_text` e `receiver` são exatamente as ocorrências `ref` com dono `('ref', raw_ref.id)`
- [x] **C2, cobertura de tipo (propriedade):** para todo símbolo com `type_text`, as palavras do tipo são exatamente as ocorrências `type` do símbolo
- [x] **C3, independência da resolução:** a mesma fixture com as `raw_ref` forçadas a `resolved` 0, 1 e 2 gera o mesmo conjunto de ocorrências `ref` e `type`
- [x] **C4, incremental:** reindexar depois de alterar um arquivo regrava só os usos e tipos desse arquivo (os `raw_ref.id` e ocorrências dos outros arquivos ficam iguais) e o resultado é igual ao de um índice do zero
- [x] **C5, evidência:** para o alvo `Alpha.run`: `Alpha(1).run()` e `SubAlpha().run()` (subclasse) e `p.run()` com `p: RunnerProtocol` implementado por `Alpha` são `forte` ou `média` conforme o mesmo uso ou o mesmo símbolo; `Beta(1).run()` e `obj.run()` são `só nome`
- [x] **C5b, alvo sem família:** para a função `m.helper`: `helper()` no próprio módulo `m`, e `from m import helper; helper()` ou `import m; m.helper()` em outro arquivo, são `forte`; `helper()` num arquivo que não importa `m` nem `m.helper` é `só nome`
- [x] **C6, linha:** cada item `forte` e `média` traz a linha do uso
- [x] **C7, só nome:** os usos `só nome` aparecem no texto só como `+N só pelo nome`, e no JSON como lista
- [x] **C8:** o `por palavra` respeita `--limit`, `--max-chars` e o `+N omitidos`, não repete o que as arestas já cobrem, e nunca lista nem conta usos de teste, com ou sem `--no-tests` (revisto na ENGINEERING, D15)
- [x] **C9:** mesma árvore e mesma config dão a mesma saída do `impact`, byte a byte, entre duas indexações do zero
- [x] **C10:** `nao_resolvidas_por_palavra` não existe mais na `cobertura` nem no JSON; goldens revistos um a um
- [x] **C11:** estatísticas, pontes, `brief` e `query word` não mudam (goldens desses comandos sem revisão)
- [ ] **C12:** *não medido: pulado por decisão do humano (2026-11-08); não bloqueia o aceite.* custo medido no gerador de `docs/medicoes.md` (índice completo, sem mudança e com um arquivo alterado), antes e depois, registrado no §10; não bloqueia o aceite
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas

### Terceira rodada
C7 da segunda rodada fica revogado pela D17.
- [ ] **C13, aditividade:** para todo alvo de uma fixture com usos em `ref` e em `type`, o conjunto de símbolos listados no `por palavra` contém o conjunto da regra da primeira rodada (palavras de `ref` ou `type` do símbolo contêm todas as palavras do nome do alvo; menos arestas, alvo e testes), calculado à parte no teste
- [ ] **C14, sem filtro:** não existe mais `by_word_name_only` no JSON nem `+N só pelo nome` no texto; usos sem evidência aparecem listados com o rótulo `só nome`
- [ ] **C15, identificador exato:** para `Alpha.run`, `AlphaBeta(1).run()` não é `forte` nem `média`; `SubAlpha(1).run()` é `forte`; para um alvo cuja família tem o Protocol `OrderDebtCodeRepository`, `ClearOrderDebtCodeRepository(s).run()` não é `forte` nem `média`
- [ ] **C16, tipo:** um atributo `receipts: list[ReceiptOut]` aparece no `impact` de um alvo cujas palavras do nome estão no tipo, como item com rótulo `tipo` e a linha do atributo
- [ ] **C17, ordem:** itens ordenados por nível (`forte`, `média`, `só nome`), depois `qname` e linha; mesma árvore dá a mesma saída byte a byte
- [ ] **C18, desempenho:** os 6 alvos da medição de 2026-11-08 rodam `impact` em até 500 ms cada no clone do `bope` (binário `--release`, `--no-index`), medido e registrado no §10
- [ ] **C19, R1:** nota `word:x` vira `orfa` quando `x` só aparece em texto de uso; o snapshot de eventos não lista palavra que só existe em `ref` ou `type`
- [ ] **C20, Y2:** `by_word` dividido em funções com uma responsabilidade cada; nenhuma função nova acima de 60 linhas
- [ ] **C21:** `--limit`, `--max-chars` e `+N omitidos` seguem valendo no `por palavra`; `cargo fmt --check`, `clippy -D warnings` e `cargo test` sem falhas

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
escritor: test-analyst (segunda rodada; a primeira não preencheu este §)

**Cenários** — Happy Path: uso como dono das palavras de `ref`; tipo no símbolo; evidência `forte`, `média` e `só nome` para método (classe dona, base, subclasse, Protocol) e para alvo sem família (mesmo módulo, import do alvo, import do módulo); linha do uso; contagem `só nome` no texto e lista no JSON · Error Scenarios: nenhum comando novo; alvo inexistente segue com exit 2 (coberto hoje) · Edge Cases: as três situações de `resolved` (0, 1, 2) na mesma fixture; resolução que muda sem o arquivo do uso mudar; reindexação incremental igual à do zero; uso já coberto por aresta; uso em arquivo de teste com `--no-tests`; corte por `--max-chars`; determinismo; `schema_version` 3 → 4.

**Fixtures**
- **U** (`a.py` e `m.py`): em `a.py`, `class Runner(Protocol)` com `run`; `class Alpha` com `run` (implementa `Runner` por estrutura); `class SubAlpha(Alpha)` sem `run` próprio; `class Beta` com `run`. Em `m.py`, `from a import Alpha, SubAlpha, Beta, Runner` e: `strong_owner()` com `Alpha(1).run()`; `strong_sub()` com `SubAlpha(1).run()`; `medium_owner(items: list[Alpha])` com `for it in items: it.run()`; `medium_protocol(items: list[Runner])` com `for it in items: it.run()`; `only_beta()` com `Beta(1).run()`; `only_obj(obj)` com `obj.run()`.
- **F** (alvo sem família): `m.py` com `def helper()` e `def local_use(): helper()`; `n.py` com `from m import helper` e `def imported_use(): helper()`; `k.py` com `import m` e `def module_use(): m.helper()`; `z.py` e `y.py`, cada um com seu `def helper()`, e `w.py` sem import de `m`, com `def other_use(): helper()`.
- **R** (as três situações de `resolved`): `def helper()` e `def one(): helper()` (1); `class Alpha` e `class Beta` com `run` e `def two(): Alpha(1).run()` (0); uma chamada que o resolvedor marca como aproximada (2), escolhida na ENGINEERING a partir de `resolve.rs:587`.
- **Ressalva:** se o resolvedor criar aresta para algum caso de U ou F (por exemplo, `it.run()` resolvido pelo tipo do laço), o caso sai do `por palavra` por C8. Ele é então trocado por outro sem aresta, e a troca é registrada no §10. Os testes unitários de evidência não dependem disso.

**Rastreabilidade**
- C1 → `each_ref_owns_exactly_its_text_words`
- C2 → `type_words_match_type_text`
- C3 → `call_made_ambiguous_elsewhere`, e C1 sobre a fixture R
- C4 → `other_files_use_rows_untouched`, `incremental_equals_fresh_index`
- C5 → os unitários `owner_class_word`, `subclass_word`, `protocol_word`, `owner_class_only_in_other_use`, `no_family_word`, e o de integração `family_uses_when_impact`
- C5b → os unitários `function_target_same_module`, `function_target_imported`, `module_of_target_imported`, `function_target_not_imported`, e o de integração `homonym_call_when_impact`
- C6 → `by_word_item_when_impact_then_line`
- C7 → `name_only_uses_when_impact_text` (golden), `name_only_uses_when_impact_json`
- C8 → `edge_consumer_when_impact` (existente), `max_chars_when_impact_by_word`, `use_in_test_file_when_impact_then_never_listed`
- C9 → `same_tree_twice_when_impact`
- C10 → golden `index_command` revisto, `coverage_json_when_rendered_then_no_by_word_field`
- C11 → não há teste novo: os goldens de `brief`, `query word`, `status` e as estatísticas não podem mudar (a revisão rejeita diff neles)
- C12 → não coberto por teste: medição no §10 (não bloqueia)
- schema → `old_schema_version_when_opened` (existente, passa a esperar `'4'`)
- não coberto: base ou subclasse ligada só por aresta aproximada (D9 usa só `ESPECIALIZA` exata para elas); hierarquia com mais de um nível (o teste usa um nível e a consulta é recursiva por construção, a conferir na revisão)

**Lifecycle Coverage**
- [ ] Uso (`raw_ref`): nasce na inserção do arquivo, morre quando o arquivo é reindexado ou removido; as palavras nascem e morrem junto → `stale_use_words_removed` (existente, adaptado ao dono `ref`), `other_files_use_rows_untouched`
- [ ] Palavra de tipo: nasce e morre com o símbolo → `type_words_match_type_text`, `incremental_equals_fresh_index`
- [ ] `resolved` muda sem o uso mudar (outro arquivo cria ambiguidade): as palavras não mudam → `call_made_ambiguous_elsewhere`
- [ ] Evidência: derivada a cada consulta, sem estado gravado → unitários de evidência e `same_tree_twice_when_impact`

### Testes obsoletos pela D7 (trocar só com aval explícito)
- `test_given_chained_call_when_indexed_then_receiver_words_belong_to_caller` e `test_given_resolved_and_unresolved_calls_when_indexed_then_both_emit_words`: afirmam que o símbolo é o dono das palavras de `ref`. Passam a afirmar que o dono é o uso, e o símbolo é alcançado por `raw_ref.src`. A intenção continua a mesma.
- `test_given_unresolved_call_when_impact_then_by_word_lists_caller`: passa a afirmar o nível (`m.go` é `forte`) em vez de só a presença.
- Golden `query__given_index_command_when_run_then_golden_summary_line`: perde `(2 por palavra)` (D10).
- Golden `query__given_method_when_query_impact_then_sections_callers_consumers_protocol_tests`: a seção `por palavra` pode ganhar a linha `+N só pelo nome`. O diff é lido antes de aceitar.

### Testes Unitários
Função pura de evidência (`memo-core`). Ela recebe as palavras do uso, as palavras dos outros usos e do tipo do mesmo símbolo, os conjuntos de palavras da família e, para alvo sem família, o módulo do uso, os imports do arquivo do uso e o `qname` do alvo. Devolve `forte`, `média` ou `só nome`.
- [ ] test_given_use_with_owner_class_word_when_classified_then_strong  (65 chars)
    - GIVEN target a.Alpha.run and a use 'Alpha(1).run'; WHEN classified; THEN strong
- [ ] test_given_use_with_subclass_word_when_classified_then_strong  (62 chars)
    - GIVEN target a.Alpha.run, family {Alpha, SubAlpha, Runner} and a use 'SubAlpha(1).run'; WHEN classified; THEN strong
- [ ] test_given_use_with_protocol_word_when_classified_then_strong  (62 chars)
    - GIVEN target a.Alpha.run, family with Runner and a use 'Runner.run'; WHEN classified; THEN strong
- [ ] test_given_owner_class_only_in_other_use_when_classified_then_medium  (71 chars)
    - GIVEN a use 'it.run' and the symbol type 'list[Alpha]'; WHEN classified for a.Alpha.run; THEN medium
- [ ] test_given_no_family_word_when_classified_then_name_only  (56 chars)
    - GIVEN a use 'Beta(1).run' and no family word in the symbol; WHEN classified for a.Alpha.run; THEN name only
- [ ] test_given_function_target_same_module_when_classified_then_strong  (67 chars)
    - GIVEN target m.helper and a use 'helper' in module m; WHEN classified; THEN strong
- [ ] test_given_function_target_imported_when_classified_then_strong  (64 chars)
    - GIVEN target m.helper and a use 'helper' in a file importing 'm.helper'; WHEN classified; THEN strong
- [ ] test_given_module_of_target_imported_when_classified_then_strong  (65 chars)
    - GIVEN target m.helper and a use 'm.helper' in a file importing 'm'; WHEN classified; THEN strong
- [ ] test_given_function_target_not_imported_when_classified_then_name_only  (73 chars)
    - GIVEN target m.helper and a use 'helper' in module w, which imports neither m nor m.helper; WHEN classified; THEN name only

### Testes de Integração (`memo-core`, `tests/use_words.rs`)
- [ ] test_given_varied_refs_when_indexed_then_each_ref_owns_exactly_its_text_words  (79 chars)
    - GIVEN fixtures U and R (calls, chained calls, receivers, typed params and returns, resolved 0, 1 and 2 all present); WHEN indexed; THEN for every raw_ref, the set of 'ref' occurrences owned by ('ref', id) equals use_words(callee_text) ∪ use_words(receiver)
- [ ] test_given_typed_symbols_when_indexed_then_type_words_match_type_text  (71 chars)
    - GIVEN fixture U; WHEN indexed; THEN for every symbol with type_text, its 'type' occurrences equal use_words(type_text), and symbols without type_text have none
- [ ] test_given_call_made_ambiguous_elsewhere_when_reindexed_then_use_words_unchanged  (81 chars)
    - GIVEN 'one()' calling 'helper()' resolved to a single definition; WHEN another file adding a second 'helper' is indexed (the caller file untouched); THEN the call's resolved changes and the words of that use are identical
- [ ] test_given_one_file_changed_when_reindexed_then_other_files_use_rows_untouched  (79 chars)
    - GIVEN fixture U indexed; WHEN only a.py changes and the repository is reindexed; THEN raw_ref ids and their occurrences for m.py are the same as before
- [ ] test_given_incremental_reindex_when_compared_then_equals_fresh_index  (69 chars)
    - GIVEN fixture U indexed and then a.py changed and reindexed; WHEN compared with a fresh index of the final tree; THEN the multiset of (word, source, owner symbol qname, line) is equal
- [ ] test_given_family_uses_when_impact_then_strong_medium_and_name_only_split  (75 chars)
    - GIVEN fixture U; WHEN impact runs on a.Alpha.run; THEN strong = {strong_owner, strong_sub}, medium = {medium_owner, medium_protocol}, name only = {only_beta, only_obj}, minus whatever edges already cover
- [ ] test_given_homonym_call_when_impact_then_name_only_counted  (59 chars)
    - GIVEN fixture F; WHEN impact runs on m.helper; THEN other_use is counted as name only and never listed as strong or medium
- [ ] test_given_by_word_item_when_impact_then_line_is_use_line  (58 chars)
    - GIVEN fixture U; WHEN impact runs on a.Alpha.run; THEN each strong and medium item carries the line of its raw_ref
- [ ] test_given_same_tree_twice_when_impact_then_identical_bytes  (60 chars)
    - GIVEN fixture U indexed fresh twice into two stores; WHEN impact on a.Alpha.run is serialized to JSON; THEN both outputs are byte-identical
- [ ] adaptados (ver obsoletos): `chained_call_...`, `resolved_and_unresolved_calls_...`, `unresolved_call_when_impact_...`; mantidos: `type_annotation_...`, `use_words_when_stored_then_symbol_id_is_null`, `changed_file_..._stale_use_words_removed`, `edge_consumer_..._does_not_repeat_it`

### Testes de Integração (`memo-cli`, `tests/query.rs`)
- [ ] test_given_name_only_uses_when_impact_text_then_only_count_line  (66 chars)
    - GIVEN fixture U; WHEN 'memo query impact Alpha.run' runs; THEN golden: 'por palavra' lists strong and medium with '~' and the line, then '+2 só pelo nome'
- [ ] test_given_name_only_uses_when_impact_json_then_listed  (55 chars)
    - GIVEN fixture U; WHEN impact runs with --json; THEN data.by_word has each item with its level and line, and the name-only items are listed
- [ ] test_given_max_chars_when_impact_by_word_then_cut_with_omitted  (63 chars)
    - GIVEN fixture U; WHEN impact runs with a --max-chars that cuts inside 'por palavra'; THEN the output ends with '+N omitidos' and stays under the limit
- [ ] test_given_use_in_test_file_when_impact_then_never_listed  (62 chars) — substitui `use_in_test_file_when_no_tests_then_by_word_hides_it` (D15)
    - GIVEN fixture U plus tests/test_m.py with a helper calling 'Alpha(1).run()'; WHEN impact runs with and without --no-tests; THEN the test use is never listed nor counted as name only
- [ ] test_given_coverage_json_when_rendered_then_no_by_word_field  (61 chars)
    - GIVEN any repository; WHEN a query runs with --json; THEN cobertura has no 'nao_resolvidas_por_palavra' key

### Terceira rodada (TEST_ANALYSIS)
**Fixtures novas**
- **U2**: a fixture U mais `class AlphaBeta` com `run` em `a.py`, e em `m.py`: `def lookalike(): AlphaBeta(1).run()`; `class Holder` com `runner: Alpha` e `backups: list[Alpha]`; `def spread(): x = Alpha; x.go(); obj.run()` (as palavras do alvo espalhadas em usos diferentes do mesmo símbolo).
- **P** (Protocol estrutural): `class OrderDebtCodeRepository(Protocol)` com `run`; `class UpdateGroupRepository` com `run` (implementa por estrutura); `class ClearOrderDebtCodeRepository` com `run`; `def caller(s): ClearOrderDebtCodeRepository(s).run()`.

**Rastreabilidade**
- C13 → `additive_when_impact_then_round_one_symbols_all_listed`
- C14 → `no_evidence_when_impact_then_listed_with_name_only_label` (memo-core), `name_only_uses_when_impact_text_then_listed_with_label` (golden, substitui `..._then_only_count_line`), `name_only_uses_when_impact_json_then_listed` (adaptado: lista única), `coverage_json_...` (mantido)
- C15 → unitários `lookalike_class_when_classified_then_not_strong`, `protocol_lookalike_when_classified_then_name_only`; integração `lookalike_class_when_impact_then_not_strong`, `protocol_lookalike_when_impact_then_name_only`
- C16 → `typed_attribute_when_impact_then_listed_with_type_label`
- C17 → `items_when_impact_then_ordered_by_level_qname_line`, `same_tree_twice_when_impact` (mantido)
- C18 → não coberto por teste: medição no `bope`, no §10
- C19 → `word_anchor_only_in_use_text_when_reindexed_then_note_orfa`, `word_anchor_only_in_use_text_when_added_then_not_found`, `word_only_in_use_text_when_indexed_then_no_word_event`
- C20 → não coberto por teste: conferido por contagem de linhas na revisão
- C21 → `max_chars_when_impact_by_word` (mantido), `fmt`, `clippy`, `test`

**Testes obsoletos pela D17 e D18 (aval dado na aprovação única da rodada 3)**
- Unitários de `tests/evidence.rs`: passam a receber textos (identificadores), não conjuntos de palavras; os nove casos e os valores esperados ficam iguais.
- `family_uses_when_impact_then_strong_medium_and_name_only_split` e `homonym_call_when_impact_then_name_only_counted`: `só nome` passa a vir de `by_word`, não de `by_word_name_only`.
- `use_in_test_file_when_impact_then_never_listed`: deixa de olhar `by_word_name_only`.
- Golden `query__given_name_only_uses_when_impact_text_then_only_count_line`: substituído.

**Testes novos (nomes e docstrings)**
- [ ] test_given_lookalike_class_when_classified_then_not_strong  (59 chars) — GIVEN family {Alpha} and a use 'AlphaBeta(1).run'; WHEN classified; THEN name only
- [ ] test_given_protocol_lookalike_when_classified_then_name_only  (61 chars) — GIVEN family {OrderDebtCodeRepository} and a use 'ClearOrderDebtCodeRepository(s).run'; WHEN classified; THEN name only
- [ ] test_given_additive_when_impact_then_round_one_symbols_all_listed  (66 chars) — GIVEN fixture U2; WHEN impact runs on a.Alpha.run; THEN every symbol whose ref or type words contain the target name words (minus edges, target and tests) is listed
- [ ] test_given_no_evidence_when_impact_then_listed_with_name_only_label  (68 chars) — GIVEN fixture U; WHEN impact on a.Alpha.run; THEN only_beta and only_obj are listed with level so_nome
- [ ] test_given_lookalike_class_when_impact_then_not_strong  (55 chars) — GIVEN fixture U2; WHEN impact on a.Alpha.run; THEN lookalike is listed as so_nome
- [ ] test_given_protocol_lookalike_when_impact_then_name_only  (57 chars) — GIVEN fixture P; WHEN impact on UpdateGroupRepository.run; THEN caller is listed as so_nome
- [ ] test_given_typed_attribute_when_impact_then_listed_with_type_label  (67 chars) — GIVEN fixture U2; WHEN impact on a.Alpha (class); THEN Holder.backups is listed with origin 'tipo' and the attribute line
- [ ] test_given_items_when_impact_then_ordered_by_level_qname_line  (63 chars) — GIVEN fixture U2; WHEN impact on a.Alpha.run; THEN items are sorted by level, then qname, then line
- [ ] test_given_word_anchor_only_in_use_text_when_reindexed_then_note_orfa  (72 chars) — GIVEN a note word:x and x left only in a call text; WHEN reindexed; THEN the note is orfa
- [ ] test_given_word_anchor_only_in_use_text_when_added_then_not_found  (68 chars) — GIVEN a word that only appears in a call text; WHEN a note anchored on it is added; THEN NotFound
- [ ] test_given_word_only_in_use_text_when_indexed_then_no_word_event  (68 chars) — GIVEN a new call to an unknown name; WHEN reindexed; THEN no WORD_CREATED event for its words

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
- Decidido D9a (humano, recomendação aceita, na ENGINEERING): o Protocol que a classe implementa só por estrutura entra na família. Essa ligação é uma `ESPECIALIZA` aproximada (`resolve.rs:333`); sem ela, a fixture U contradizia o C5. Bases e subclasses continuam só por aresta exata.
- Decidido D14 (humano, recomendação aceita, na ENGINEERING): o "mesmo uso" da evidência `forte` é a linha do símbolo. O extrator divide `Alpha(1).run()` em duas `raw_ref` na mesma linha, `?.run` (sem receptor) e `Alpha`, e por isso a classe nunca aparecia no mesmo uso que o método. A alternativa de gravar o receptor no extrator foi recusada: o resolvedor lê o receptor e passaria a criar arestas novas. Efeito colateral aceito: `x = Alpha(); y.run()` numa linha só vira `forte`.
- Decidido D15 (humano, recomendação aceita, na ENGINEERING): o `por palavra` só mostra usos de produção, como já fazia desde o PR #1 (`s.is_test = 0` no SQL), e o teste do C8 afirma isso. O extrator não grava chamadas no nível do módulo, e toda função em arquivo de teste é `is_test = 1`, então um uso de teste nunca chega à seção; o `--no-tests` não tem efeito nela.
- Decidido D16 (humano, recomendação aceita, na CODE_REVIEW [2026-11-08T15:37-0300]): a palavra de uma nota `word:` e do snapshot de eventos vem só do vocabulário (`name`, `path`, `import`, `test_doc`), como em estatísticas, pontes e `brief`; as fontes de uso (`ref`, `type`) não contam.
**Terceira rodada (2026-11-08, depois da medição no `bope`):**
- Decidido D17 (humano, recomendação aceita): a estrutura só adiciona conexões, e a inferência as considera sem esconder nada; o `por palavra` é aditivo, e o nível é rótulo e ordem. Revoga a D8 e o C7.
- Decidido D18 (humano, recomendação aceita): a evidência de classe exige o nome da classe como identificador inteiro no texto ou no receptor do uso; o subconjunto de palavras deixa de valer. A família da D9/D9a fica como está.
- Decidido D19 (humano, recomendação aceita): atributo cujo tipo anotado contém o nome do alvo volta como item próprio, com o rótulo `tipo`.
- Decidido D20 (humano, recomendação aceita): critério de desempenho de até 500 ms por alvo no `bope`, com carga em lote e índice `raw_ref(src, line)`.
- Decidido D21 (humano, recomendação aceita): R1 e Y2 da revisão entram nesta rodada.
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
#### [2026-11-08T15:37-0300] — [c144bd45](https://github.com/erickod/memo/commit/c144bd45c3b840a93b9ebc9482a53fc27c29add3)
WT: `/Users/erick.duartelend.tech/Documents/dev/memo` (checkout do usuário, branch `feat/task-6-completion`, árvore limpa) · base `origin/master` = merge-base `8835ee4` · escopo: segunda rodada da TASK-6 (22 commits)

**SUMMARY**
- Veredito: **Requer alterações**. STATE: BLOCKED → REQUEST CHANGES: eng.
- Positivos: fluxo red → green em todos os passos de código; a camada relacional ficou com ponto único de escrita (`insert_raw_ref`), testada por propriedade (C1, C2) e por equivalência incremental × do zero (C4); a inferência ficou numa função pura (`evidence::classify`) com 9 unitários; decisões D9a, D13, D14 e D15 registradas antes do código que as usa; 334 testes, `fmt` e `clippy` verdes.
- Preocupações: 🔴 R1, a palavra das notas `word:` e dos eventos aceita as fontes `ref` e `type`, contra a D16 (decidida nesta revisão); 🟡 Y1, consultas por uso sem índice em `raw_ref.src` (N+1 sem medição, C12 pulado); 🟡 Y2, `by_word` com 149 linhas.

**ANALYSIS**
CRITERIA
- [ ] design: `by_word` concentra busca de candidatos, montagem da âncora, classificação e deduplicação (crates/memo-core/src/query.rs:758-906) → Y2
- [ ] DDD: não identificado — verificado em: EXTRACT módulos `evidence.rs`, `query.rs`, `index.rs` (CLI sem modelo de domínio de negócio)
- [ ] reliability: não identificado — verificado em: `run_index` numa única transação (crates/memo-core/src/index.rs, `insert_raw_ref` e `delete_file_symbols` dentro de `tx`); idempotência coberta por `incremental_equals_fresh_index`
- [ ] data: não identificado — verificado em: EXTRACT sem valores monetários, PII ou trilha financeira
- [ ] contract: JSON do `impact` muda de forma (`by_word` vira objeto com `nivel`, entra `by_word_name_only`) e a `cobertura` perde `nao_resolvidas_por_palavra` (crates/memo-core/src/query.rs:94-104, :497-503); consumidores internos verificados (render, hide, delta, main, testes); consumidor externo: `memo serve`/MCP repassa o JSON (crates/memo-mcp/src/lib.rs:23) → informado no CHANGELOG? não há CHANGELOG; esquema JSON é `schema: 1` sem versão subida → 🟢 G1
- [ ] env_k8s: não identificado — verificado em: diff sem settings, env ou manifest
- [ ] concurrency: não identificado — verificado em: diff sem worker/consumer/entrypoint
- [ ] migrations: não identificado — verificado em: banco derivado; `schema_version` 3 → 4 descarta e recria (crates/memo-core/src/store.rs:8), testado em `old_schema_version_when_opened`
- [ ] perf: N+1 por uso candidato (3 a 4 consultas por uso) e `r.src = ?1` sem índice em `raw_ref(src)` (crates/memo-core/src/query.rs:833-860; índices em crates/memo-core/src/store.rs:48-55) → Y1
- [ ] security: não identificado — verificado em: SQL dinâmico só com ids inteiros (`word_id IN ({list})`, crates/memo-core/src/query.rs:777-790) e parâmetros bind no restante
- [ ] errors: não identificado — verificado em: todas as consultas novas propagam `CoreError` com `?`
- [ ] readability: nomes e comentários explicam o porquê (D14 em crates/memo-core/src/query.rs:836-838); `by_word` longo → Y2
- [ ] layers: não identificado — verificado em: `index.rs` (relacional) não importa `evidence.rs` (inferência); `evidence.rs` não importa banco (§5)
- [ ] tests: todos os nomes do §7 presentes com docstring GIVEN/WHEN/THEN (crates/memo-core/tests/evidence.rs, crates/memo-core/tests/use_words.rs, crates/memo-cli/tests/query.rs:1095-1205); D16 sem teste (ver R1)
- [ ] task: §4 C1–C11 com teste; C12 não medido (registrado, não bloqueia); decisão D16 com efeito observável sem enforcement nem teste (crates/memo-core/src/notes.rs:750, :829; crates/memo-core/src/events.rs:75) → R1
- [ ] format: linhas acima de 100 só em literais SQL (crates/memo-core/src/query.rs:705), mesmo padrão do arquivo; `fmt` e `clippy -D warnings` verdes
- [ ] lang: lang sem reference: rust (só critérios base aplicados)
- [ ] implicit: "mesmo uso" = mesma linha do símbolo (D14) explícito em comentário e na spec; `family` inclui Protocols das bases, além dos da dona (crates/memo-core/src/query.rs:727-745) → 🟢 G2
- [ ] refactor_estrutural: remoção de `rebuild_use_words` preserva semântica, travada por `incremental_equals_fresh_index` e `each_ref_owns_exactly_its_text_words`

SMELLS
- [ ] Long Method: `by_word` 149 linhas (crates/memo-core/src/query.rs:758) → Y2
- [ ] Long Class: não identificado — verificado em: `query.rs` 978 linhas, módulo de funções sem estado (preexistente)
- [ ] Long Params: não identificado — verificado em: `Anchor::Module` agrupa 4 campos em vez de parâmetros soltos (crates/memo-core/src/evidence.rs:24-33)
- [ ] Data Clumps: tupla `Use = (SymbolAddr, i64, i64, i64, i64)` (crates/memo-core/src/query.rs:798) → parte de Y2
- [ ] Primitive Obsession: não identificado — verificado em: nível como `enum Evidence`, não string
- [ ] Switch Stmts: não identificado — verificado em: dois `match` sobre `Evidence`/`Anchor`, um por responsabilidade
- [ ] Refused Bequest: não identificado — verificado em: sem herança/traits novos
- [ ] Divergent Change: não identificado — verificado em: `evidence.rs` muda só por regra de evidência
- [ ] Shotgun Surgery: tipo `ByWordItem` exigiu ajuste em `hide`, `delta`, `render` (3 locais, esperado por contrato)
- [ ] Speculative Generality: não identificado — verificado em: `Anchor` com dois casos, ambos usados
- [ ] Duplicate Code: fixture U duplicada em crates/memo-core/tests/use_words.rs e crates/memo-cli/tests/query.rs (binários de teste separados) → 🟢 G3
- [ ] Dead Code: drop de `by_word_name_only` em crates/memo-core/src/hide.rs:62 não é alcançável hoje (D15) → 🟢 G4
- [ ] Temp Field: não identificado — verificado em: structs novas sem campo opcional temporário
- [ ] Feature Envy: não identificado — verificado em: `classify` usa só seus argumentos
- [ ] Inappropriate Intimacy: não identificado — verificado em: `render` lê `ByWordItem` público, sem acessar internos
- [ ] Msg Chains: não identificado — verificado em: acessos `b.item.addr.qname` de profundidade 3, padrão preexistente
- [ ] Middle Man: não identificado — verificado em: sem delegação vazia
- [ ] Magic Numbers: não identificado — verificado em: sem literais numéricos novos
- [ ] Ignored Exceptions: não identificado — verificado em: sem `unwrap` em código de produção novo (só em testes)
- [ ] Opaque Indirection: não identificado — verificado em: closure `words_where` local e comentada (crates/memo-core/src/query.rs:839)
- [ ] Order Coupling: `insert_raw_ref` grava as palavras depois do INSERT e lê `last_insert_rowid` (crates/memo-core/src/index.rs:380-410); ordem garantida na própria função

N_PARTITE
- [ ] n-partite: ver `N_PARTITE (CODE_REVIEW)` abaixo — `[N_PARTITE] modo: code | alvo: delta | referência: TASK-6 §4/§9 | perfil: stacked + full`; `[N_PARTITE.GAP]` D16 sem guarda; `[N_PARTITE.FAIL]` não coberto: nenhum; DRIFT: não aplicável — sem estado empilhado de DEFINITION

VERIFY: reli o diff `8835ee4..c144bd45` do zero (crates/memo-core/src/{store,index,query,evidence,hide,delta}.rs, crates/memo-cli/src/render.rs e os testes). Achados novos da releitura: G2 (Protocols das bases na família) e G5 (com `--task`, `by_word_name_only` não passa pelo delta: `+N só pelo nome` se repete a cada chamada, crates/memo-core/src/delta.rs:86-95). Nenhum outro.

**ACTIONS**
- 🔴 R1 — palavra de nota e de evento aceita fontes de uso. Problema: `notes.rs:750` e `:829` e `events.rs:75` consultam `occurrence` sem filtrar `source`; com a TASK-6, uma palavra que só existe em `ref`/`type` mantém viva uma nota `word:x` e entra no snapshot de eventos. Impacto: contraria a D16; nota que deveria virar `orfa` não vira. Fix (eng, commit próprio, red → green):
  - before: `SELECT EXISTS(SELECT 1 FROM word w JOIN occurrence o ON o.word_id = w.id WHERE w.text = ?1)`
  - after: `... WHERE w.text = ?1 AND o.source IN ('name', 'path', 'import', 'test_doc'))`, nas três consultas, com teste: nota `word:x` vira `orfa` quando `x` só aparece numa chamada; snapshot de eventos não lista palavra só de uso.
- 🟡 Y1 — consultas por uso sem índice. `by_word` faz 3 a 4 consultas por uso candidato e filtra `raw_ref` por `src` sem índice (store.rs:48-55). Alvo de nome genérico com centenas de usos faz varreduras repetidas; C12 não medido. Correção: `CREATE INDEX raw_ref_src ON raw_ref(src, line)` (o esquema 4 ainda não foi publicado: dá para incluir sem subir de novo) e medir o `impact` de um nome genérico antes/depois no gerador.
- 🟡 Y2 — `by_word` longo. Justificativa: mistura quatro responsabilidades (query.rs:758-906). Correção: extrair `anchor_for(store, id)` e `use_evidence(store, use, anchor)`, trocando a tupla `Use` por struct.
- 🟢 G1 — o JSON do `impact` mudou de forma sob `schema: 1`; benefício de registrar no README/spec que `by_word` mudou (consumidores MCP).
- 🟢 G2 — `family` inclui Protocols das bases (query.rs:727-745); registrar na D9a para a regra e o código dizerem o mesmo.
- 🟢 G3 — fixture U duplicada em dois binários de teste; mover para um arquivo `tests/fixtures/u/` lido pelos dois.
- 🟢 G4 — drop de `by_word_name_only` em `hide.rs:62` é defensivo e hoje inalcançável (D15); manter com comentário ou remover.
- 🟢 G5 — com `--task`, `+N só pelo nome` não passa pelo delta (delta.rs:86-95); decidir se a contagem também é "nova desde a última chamada".
- ⚖️ T1 — forma de corrigir Y1: (a) índice `raw_ref(src, line)`: 1 linha, mantém o código, ainda N+1 em memória local · (b) pré-carregar as palavras de todos os usos dos símbolos candidatos numa consulta só e classificar em memória: elimina o N+1, mais código. Recomendação: (a) agora, (b) só se a medição mostrar `impact` acima de 500 ms num nome genérico.

**COMMITS**
- ✓ [d8d95f83](https://github.com/erickod/memo/commit/d8d95f83ef82cf696eb76f35e492b0a7f05df66e) docs: reabre a TASK em DEFINITION com D7–D12
- ✓ [36a323a8](https://github.com/erickod/memo/commit/36a323a84353675014fad84f6b4770d4b899aec2) docs: §7
- ✓ [8e0f7034](https://github.com/erickod/memo/commit/8e0f70347f509bbde17daa3e7dbad4747bd8a926) test: schema 4 (red por asserção)
- ✓ [e01febd3](https://github.com/erickod/memo/commit/e01febd33b5e0ea75d9b2f1b592af414c58cb439) feat: `raw_ref.id` e schema 4
- ✓ [12148731](https://github.com/erickod/memo/commit/121487312a2d8706241397c6276299b7549b1e0a) test: dono `ref` (red por asserção; adapta os obsoletos aprovados)
- ✓ [f780582a](https://github.com/erickod/memo/commit/f780582abf6b584f5f7bcf037d95923e9095aee8) feat: uso como dono
- ✓ [e4915c77](https://github.com/erickod/memo/commit/e4915c77010fe43a205deaa79f8ae67585af106e) test: incremental (red por asserção; fixtures C2/C3 ajustadas e registradas)
- ✓ [45e2d6c1](https://github.com/erickod/memo/commit/45e2d6c11843144bbff725defb69de6bb91ea1cf) perf: gravação por arquivo
- ✓ [4d94f233](https://github.com/erickod/memo/commit/4d94f233380bc4afcf27792a0aa5ebd871a57ce7) docs: log (a)–(c) e D9a
- ✓ [e26062cf](https://github.com/erickod/memo/commit/e26062cf016e9c466debfe47749c818a16f447a9) test: níveis por uso (stubs de tipo no mesmo commit, explicados no corpo)
- ✓ [27fe910b](https://github.com/erickod/memo/commit/27fe910b95362f3a116c26e388e4869e395f3606) feat: classificação por evidência
- ✓ [bed42c7b](https://github.com/erickod/memo/commit/bed42c7b4566272b7e5d3c2ef467e3e9dee3c718) docs: log (d) e D14
- ✓ [80599d75](https://github.com/erickod/memo/commit/80599d758b6789dc37dd0f93024eb2be407616fe) test: CLI (red)
- ✓ [d0f4b599](https://github.com/erickod/memo/commit/d0f4b5990e8fc9b915654674c7dc7d303b5abe6f) docs: D15
- ✓ [2c403241](https://github.com/erickod/memo/commit/2c40324125a422b04ee96fbfc970f7b8b545ecf9) feat: render (golden aceito pelo humano)
- ✓ [a9bf1672](https://github.com/erickod/memo/commit/a9bf1672c1d0725ed268402e2b1ff9a48de14422) test: sem contagem na cobertura (goldens editados só no trecho)
- ✓ [e916d5d9](https://github.com/erickod/memo/commit/e916d5d94d66028a166f955fe2fa6b6b92b7a0fa) feat: remove a contagem
- ✓ [6f679714](https://github.com/erickod/memo/commit/6f679714deb2c59b819f6d23439d037a985199e6) docs: spec
- ✓ [ea77f5e7](https://github.com/erickod/memo/commit/ea77f5e75f6bc9e4ac900b7182dc6539493074f0) docs: funcionalidades
- ✓ [baf9aed7](https://github.com/erickod/memo/commit/baf9aed72a21d69b90b98f7588ad6a2af43f96d1) docs: limitações
- ✓ [b322f52f](https://github.com/erickod/memo/commit/b322f52f5495fbeab3fcc7f8cd1bcf09fb78ed24) docs: índice de tasks
- ✓ [c144bd45](https://github.com/erickod/memo/commit/c144bd45c3b840a93b9ebc9482a53fc27c29add3) docs: log (e)–(f) e critérios

**EVIDÊNCIAS**
- SHA revisado: `c144bd45c3b840a93b9ebc9482a53fc27c29add3` · base `origin/master` = `8835ee4dfa9be0012577963b7abd1cf70b7396dc` (fetch `git fetch --no-tags origin +refs/heads/master:refs/remotes/origin/master` ok) · merge-base único `8835ee4` · fingerprint `sha256(git diff --no-ext-diff --no-textconv --binary --find-renames 8835ee4 c144bd45)` = `8a21b5dc313b8487b27f5ab9272f508724a2a710c7c4284791ffa8067269571c` (calculado antes do commit desta entrada)
- Comandos: `cargo fmt --check` ok · `cargo clippy --all-targets -- -D warnings` ok · `cargo test --no-fail-fast` 334 passed, 0 failed
- Consumidores: memo não indexa Rust (`memo status` → `base: ausente`); grep mínimo `grep -rlw --include=*.rs <termo> crates`: `Impact` → main.rs, render.rs, delta.rs, query.rs, hide.rs, tests/use_words.rs · `by_word`/`by_word_name_only` → render.rs, hide.rs, delta.rs, query.rs e testes (`representativeness.rs` casa só uma variável local homônima) · `Coverage` → render.rs, main.rs, query.rs · `nao_resolvidas` → render.rs, query.rs, tests/calls.rs, tests/query.rs (nenhum uso do campo removido) · `insert_raw_ref`/`delete_file_symbols` → só index.rs · consumidor externo do JSON: `memo serve` (crates/memo-mcp/src/lib.rs:23). memo: 0 · grep extra: 7 termos · não coberto: clientes MCP fora do repo
- Não executado: medição C12 (pulada por decisão do humano); teste contra clientes MCP reais
- GRILL: 1 rodada, 1 decisão (D16), 0 pendências

### N_PARTITE (CODE_REVIEW)
[N_PARTITE] modo: code | alvo: delta | referência: TASK-6 §4 e §9 (decisões D7–D16) | perfil: stacked + full | budget: 10 | frame: base | fonte: 8835ee4..c144bd45
- metadado: repo `github.com/erickod/memo` · WT `/Users/erick.duartelend.tech/Documents/dev/memo` · base `origin/master` `8835ee4` · merge-base `8835ee4` · fingerprint sha256 `8a21b5dc…69571c` (método `git diff --no-ext-diff --no-textconv --binary --find-renames`) · fetch ok · memo: índice ausente (Rust não indexado), grep mínimo
[N_PARTITE.STACK] decisão: base | frames: 1 | fonte: 8835ee4..c144bd45 | recalculados: 0 | herdados: 0 | resolvidos: 0 | revalidar: 0
[N_PARTITE:code] par: occurrence.owner_kind='ref' (L1) ↔ insert_raw_ref (L3) — acoplamento: alto | evidência: co-mudança f780582a, 45e2d6c1 | abstração: ponto único de escrita | lifecycle: coerente (nasce e morre com a raw_ref: index.rs:380-410, :126-131) → manter
[N_PARTITE:code] par: raw_ref.id (L1) ↔ store.rs (L4) — acoplamento: baixo | evidência: e01febd3 | abstração: alias de rowid | lifecycle: coerente → manter
[N_PARTITE:code] par: ByWordItem.nivel (L1) ↔ Evidence (L2) — acoplamento: alto | evidência: e26062cf, 27fe910b | abstração: enum fechado | lifecycle: coerente → manter
[N_PARTITE:code] par: Evidence (L2) ↔ classify (L3) — acoplamento: alto | evidência: 27fe910b | abstração: função pura no módulo do tipo | lifecycle: coerente → manter
[N_PARTITE:code] par: Impact (L2) ↔ by_word (L3) — acoplamento: alto | evidência: e26062cf, 27fe910b | abstração: extrair montagem de âncora e classificação por uso | lifecycle: coerente → Y2
[N_PARTITE:code] par: by_word (L3) ↔ query.rs (L4) — acoplamento: alto | evidência: 27fe910b | abstração: candidatos/âncora são inferência; poderiam ir para `evidence.rs` com acesso a banco separado | lifecycle: coerente → Y2
[N_PARTITE:code] par: occurrence (L1) ↔ notes.rs/events.rs (L4) — acoplamento: alto | evidência: não definido (sem co-mudança no diff) | abstração: filtro de fonte "vocabulário" compartilhado | lifecycle: divergente (fontes de uso nasceram na TASK-6, consumidores não foram ajustados) → R1
[N_PARTITE:code] par: evidence.rs (L4) ↔ memo-core (L5) — acoplamento: baixo | evidência: e26062cf | abstração: módulo puro | lifecycle: coerente → manter
[N_PARTITE:code] par: render.rs::by_word_item (L3) ↔ memo-cli (L5) — acoplamento: baixo | evidência: 2c403241 | abstração: formatação local | lifecycle: coerente → manter
[N_PARTITE.STRUCT:code] alvo: query.rs::by_word — problema: orquestrador com regra (âncora + classificação + dedupe) | estrutura proposta: `anchor_for` + `use_evidence`, struct no lugar da tupla `Use` | base: crates/memo-core/src/query.rs:758-906
[N_PARTITE.FAIL] ação: index.rs::insert_raw_ref (crates/memo-core/src/index.rs:380) — eixo: interrupção | status: coberto por transação única de `run_index` → —
[N_PARTITE.FAIL] ação: index.rs::insert_raw_ref — eixo: duplicidade | status: coberto por `delete_file_symbols` antes da reinserção e teste `incremental_equals_fresh_index` → —
[N_PARTITE.FAIL] ação: store.rs schema 4 — eixo: falha | status: coberto por descarte e recriação do banco derivado, notas intactas (teste `old_schema_version_when_opened`) → —
[N_PARTITE.FAIL] ação: query.rs::by_word — eixo: tempo | status: não coberto (sem limite de usos candidatos; custo não medido, C12) → Y1/T1
[N_PARTITE.GAP] item: D16 — palavra de nota e de evento só do vocabulário (name, path, import, test_doc) — status: ausente-não-decidido no código | base: TASK-6 §9 D16 → R1
[N_PARTITE.GAP] item: C12 custo medido — status: ausente-intencional (registrado no §4 e §10) | base: TASK-6 §4 C12 → —
[N_PARTITE.CLUSTER] nome: camada relacional de uso — membros: 4 (raw_ref.id, occurrence ref/type, insert_raw_ref, delete_file_symbols) | razão de mudança: nova fonte de texto extraída | fronteira com: inferência por palavra (→) | contrato: exigido (occurrence com owner_kind/source; dono e ciclo diferentes)
[N_PARTITE.CLUSTER] nome: inferência por palavra do impact — membros: 6 (by_word, family, hierarchy, classify, Evidence, ByWordItem) | razão de mudança: nova regra de evidência | fronteira com: renderização (→) | contrato: exigido (`Impact` JSON, consumidor externo MCP)
[N_PARTITE.INV] id: INV-USE-1 (C1) | matriz: confirmado | c_código: 1.00 | c_humano: 1 (TASK-6 §4 C1) | estado: occurrence(owner_kind='ref') | guardas: insert_raw_ref | sem guarda: delete_file_symbols (coerente: só remove junto com a raw_ref) | cobertura: grep `owner_kind = 'ref'` e `INSERT INTO occurrence` · não cobre SQL dinâmico | proteção: atual 2 · recomendada 2 (dado derivado, |W|=2)
[N_PARTITE.INV] id: INV-USE-2 (§5 camada relacional não importa inferência) | matriz: confirmado | c_código: 0.45 | c_humano: 1 (TASK-6 §5) | estado: imports de index.rs | guardas: — | sem guarda: index.rs (coerente: importa só `sources::use_words`) | cobertura: leitura dos `use` de index.rs | proteção: atual 0 · recomendada 1 (|W|=1, sem dado sensível; teste de arquitetura opcional)
[N_PARTITE.INV] id: D16 | matriz: intenção sem proteção | c_código: 0.00 | c_humano: 1 (decisão [2026-11-08T15:37-0300]) | estado: consultas de palavra em notes.rs/events.rs | guardas: — | sem guarda: notes.rs:750, notes.rs:829, events.rs:75 (GAP: aceitam source ref/type) | cobertura: grep `FROM occurrence` sem `source` · não cobre consultas dinâmicas | proteção: atual 0 · recomendada 1 (|W|=3, dado derivado) → R1
[N_PARTITE.SIDE] ação: insert_raw_ref (crates/memo-core/src/index.rs:380) — eixo: visibilidade | efeito: palavras de uso visíveis para notas `word:` e eventos | quando: imediato | status: GAP → R1
[N_PARTITE.SIDE] ação: store.rs schema 4 — eixo: cascata | efeito: banco recriado na primeira abertura | quando: imediato | status: informativo → —
[N_PARTITE.RANK] 1 · GAP D16 · impacto: alto · alcance: não definido · custo: baixo · 80/20
[N_PARTITE.RANK] 2 · FAIL tempo by_word · impacto: médio · alcance: não definido · custo: baixo · —
[N_PARTITE.RANK] 3 · STRUCT by_word · impacto: baixo · alcance: não definido · custo: médio · —
- cohesion.evidence: co-mudança medida só nos 22 commits do diff (janela: 8835ee4..c144bd45) · drift: não aplicável — sem estado empilhado de DEFINITION

## 10. Log de Decisões e Alterações

### [2026-11-08] — Medição no `bope` e reabertura em DEFINITION (terceira rodada)
- Tipo: medição e decisão
- Medição (clones isolados em `/tmp/memo-cmp`, binários `--release` de `8835ee4` e da branch, `--no-index`, uma execução): índice completo 54 s → 52 s; `impact` por alvo, antes → agora: `UpdateOrderGroupForRenegotiationRepository.run` 160 listados, 0,07 s → forte 2, média 18, só nome 197, 4,33 s; `GroupRenegotiate.execute` 248, 0,07 s → 1/0/384, 7,63 s; `ImhotepReceiveNfeAccessKeysPort.call` 27 → 0/0/35, 0,75 s; `ManuallyLockedOrder.is_renegotiation` 7 → 0/2/10, 0,30 s; `ProcessReceivableLockRequestMessage.build` 37 → 0/0/51, 1,08 s; `OrderOut.receipt` 119 → 4/3/160, 3,49 s.
- Achados conferidos no código do `bope`: os dois `forte` de `run` são falsos (`UpdateOrdersStatusByDebtCodeRepository(session).run`, `ClearOrderDebtCodeRepository(session).run`), pela família com o Protocol estrutural `OrderDebtCodeRepository` somada ao casamento por subconjunto de palavras; o `forte` de `execute` é verdadeiro (`operation.execute(input)`); cinco símbolos de `OrderOut.receipt` sumiram porque a busca de candidatos não lê mais `type`; o `impact` ficou de 10 a 100 vezes mais lento.
- Decisão tomada: D17 a D21 (§9); critérios C13 a C21 (§4).
- Impacto: `Estado: DEFINITION`; C7 revogado.

### [2026-11-08T15:37-0300] — code review Requer alterações
- Tipo: revisão
- Revisado: `8835ee4..c144bd45` (22 commits), entrada completa no §9/Code Review e `N_PARTITE (CODE_REVIEW)`.
- Achados: 🔴 R1 (D16 sem guarda em notes.rs/events.rs); 🟡 Y1 (consultas por uso sem índice em `raw_ref.src`), Y2 (`by_word` com 149 linhas); 🟢 G1–G5; ⚖️ T1.
- GRILL: grill-me · diferida · — · CODE_REVIEW · 1 rodadas · 1 decisões · 0 pendências · STATUS:READY
- Impacto: STATE: BLOCKED → REQUEST CHANGES: eng (R1 obrigatório; Y1 e Y2 a corrigir ou justificar).

### [2026-11-08] — ENGINEERING, passos (e) e (f), medição pulada e docs
- Tipo: implementação e documentação
- Feito: (e) o texto mostra o nível no fim da linha e `+N só pelo nome`; o JSON lista `by_word_name_only` (`80599d7` red, `2c40324` green; golden aceito pelo humano depois de lido). (f) `nao_resolvidas_por_palavra` saiu da `cobertura` e do JSON (`a9bf167` red, com os dois goldens editados só nesse trecho; `e916d5d` green). Docs: spec, `docs/funcionalidades.md`, `docs/limitacoes.md` e `tasks/README.md`. 334 testes, `fmt` e `clippy` verdes.
- Decisão tomada: D15 (§9). O teste do C8 com `--no-tests` não tinha como passar e foi trocado, com aval, por `use_in_test_file_when_impact_then_never_listed`.
- C12 não medido: a medição antes/depois (worktree do `8835ee4` e gerador de `docs/medicoes.md`) foi interrompida e pulada por decisão do humano. Os temporários foram removidos. Não bloqueia o aceite, e fica registrado em `docs/limitacoes.md`.
- Não verificado: o custo da gravação por arquivo e da consulta por uso (uma consulta por uso candidato, o que pode pesar em alvos com muitos usos); medição em repositório real.
- Próximo: CODE_REVIEW da segunda rodada.

### [2026-11-08] — ENGINEERING, passo (d)
- Tipo: implementação
- Feito: `evidence.rs` (`Evidence`, `Anchor` e a função pura `classify`); `query::by_word` passa a ser por uso, com a família (D9, D9a), o módulo (D13), a linha do uso e o nível. `Impact.by_word` traz `forte` e `media`, e `Impact.by_word_name_only` traz `so_nome`. Quando há dois usos na mesma linha do mesmo símbolo, vale o mais forte (`e26062c` red, com os stubs; green no commit seguinte). 329 testes, `fmt` e `clippy` verdes.
- Achado no green: a chamada encadeada vira duas `raw_ref`, o que levou à D14 (§9).
- Conferido num repositório de rascunho com a fixture U: `forte` = `strong_owner` :L4 e `strong_sub` :L7; `media` = `medium_owner` :L11 e `medium_protocol` :L15; `so_nome` = `only_beta` :L18 e `only_obj` :L21. Nenhum desses usos tem aresta.

### [2026-11-08] — ENGINEERING, passos (a) a (c)
- Tipo: implementação
- Feito: (a) `raw_ref.id` explícito e `schema_version` 4 (`8e0f703` red, `e01febd` green); (b) cada referência é dona das próprias palavras de uso (`1214873`, `f780582`); (c) palavras de uso gravadas por arquivo, sem reconstrução global (`e4915c7`, `45e2d6c`). 316 testes, `fmt` e `clippy` verdes.
- Ajuste de fixture no red do passo (c): `type_text` só é gravado em atributos anotados (`extract.rs:426-473`). Por isso, a fixture U não tinha nenhum símbolo com tipo, e o C2 ganhou uma classe `Holder` com atributos anotados.
- Achado (anterior à TASK, fora do escopo): quando outro arquivo ganha um homônimo (`def helper()`), quem já chamava `helper` não é resolvido de novo; a chamada segue `resolved = 1`, apontando para a definição antiga. O C3 passou a usar a remoção de `helper` em `h.py`, que faz o resolvedor refazer a resolução de quem a chamava (`resolved` 1 → 2).
- Achado (anterior, do PR #1): `events.rs:75` e `notes.rs:750/829` não filtram por `source`. Desde o PR #1, uma palavra que só aparece em texto de uso conta como existente para a âncora `word:` de nota e para o snapshot de eventos. Ainda não decidido se é desejado.
- Decisão tomada: D9a (§9).

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
