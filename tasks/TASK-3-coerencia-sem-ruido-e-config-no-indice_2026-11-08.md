# TASK-3 — Âncoras por interconexão: esqueleto só pela lista, índice que acompanha a config e representatividade na ficha
branch: master
workdir: /var/home/erickod/Documentos/dev/memmo
created_at: 2026-11-08

## Status
- Estado: done
- Owner: Erick Duarte
- ROLE requerida: eng

---

## 1. Objetivo
Fazer `memo brief order` devolver resultado a partir das interconexões da palavra (símbolos, testes, imports, chamadas e outras palavras), sem o `memo` decidir sozinho se uma palavra é de domínio. (REF: DOR#CRITERIA)

---

## 2. Contexto
- Origem: `memo brief order` no repositório real `easy_lease_integrated` voltou vazio (`âncoras: nenhuma   ignoradas: order`). `memo query word order` mostra `espec=0.06`, `coer=0.22`, `esqueleto: sim`: a regra automática da TASK-2 (especificidade abaixo de 0,35 E coerência abaixo de 0,30) marcou `order` como esqueleto, e esqueleto nunca é âncora.
- Medições numa cópia do repositório (`/tmp/eli`, 6.012 símbolos, 796 arquivos), registradas no log:
  - tirar 32 palavras de função da coerência não ajuda (`order` 0,21 → 0,20);
  - a coerência não separa conceito de ruído (`order` 0,21, `reservation` 0,27, `payment` 0,29, contra `id` 0,48, `is` 0,35);
  - `context_root = "backend/easy_lease"` tira `easy`, `lease` e `backend` das pontes de import, mas não muda o esqueleto;
  - a representatividade entre pares é assimétrica e informativa em arquivos (`P(order|cancel)` 0,77 contra `P(cancel|order)` 0,19; `P(order|id)` 0,33);
  - a concentração entre contextos separa as palavras de domínio das de infraestrutura nessa amostra, mas o humano decidiu não classificar domínio agora (D1).
- Segundo problema: trocar `context_root` no `memo.toml` não reindexa. `run_index` só olha mtime, tamanho e hash dos arquivos; sem arquivo mudado ele sai antes de `stats::recompute`. Só `rm base.db` aplicou a config. O mesmo vale para `skeleton`, `verbs`, `roles`, camadas e pesos de ponte.
- Estado atual do código: `esqueleto` em `stats.rs` = palavra em `skeleton` OU (especificidade < `min_specificity` E coerência < `min_coherence`); âncora de `brief` e de nota recusa `esqueleto`; `store.rs` tem `meta` com `get_meta`/`set_meta`; `schema_version` 2.
- Esta TASK substitui o critério 7 da TASK-2 (esqueleto automático por IDF e coerência).
- Stack: Rust, SQLite, as mesmas dependências da TASK-1.

---

## 3. Escopo

### Dentro do escopo
- `esqueleto` passa a significar só "está na lista `skeleton`"; a regra automática por especificidade e coerência deixa de decidir.
- Especificidade e coerência continuam calculadas e mostradas (`stats`, `query word`), só informativas.
- `min_specificity` e `min_coherence` continuam aceitas em `memo.toml`, sem efeito (compatibilidade).
- Hash das configurações que afetam o índice, guardado em `meta`; quando muda, o próximo `index` refaz tudo (recria o banco derivado, notas intactas).
- Ficha `query word` mostra a representatividade da palavra em arquivos: frente ao repositório, frente a cada contexto e frente às vizinhas mais fortes (nos dois sentidos).
- Medição antes e depois no `easy_lease_integrated` (o `brief order`); spec (§5.6, A7, A12) e README.

### Fora do escopo
- Decidir se uma palavra é de domínio (explícito do humano, D1).
- Usar a representatividade para ranquear o `brief` (TASK seguinte).
- Lista de palavras de função, filtro de ruído na coerência, concentração entre contextos como critério de esqueleto.
- Calibração de pesos e da lista `skeleton` no repositório real (do humano).
- Materializar pontes em tabela (D19 da TASK-2).

---

## 4. Critérios de Aceitação
- [x] palavra fora da lista `skeleton` nunca é `esqueleto`, mesmo com especificidade e coerência abaixo dos limiares antigos (fixture com palavra frequente e de coerência baixa, tipo `order` com cauda longa)
- [x] palavra na lista `skeleton` continua `esqueleto`: não é âncora do `brief`, `note add` nela falha com `ancora_generica` (exit 4) e aparece em `ignoradas`
- [x] `brief` com uma palavra frequente de coerência baixa (fora da lista) traz âncora e símbolos, em vez de `âncoras: nenhuma`
- [x] especificidade e coerência continuam em `stats` e em `query word`; `min_specificity` e `min_coherence` no `memo.toml` são aceitos e não alteram o esqueleto
- [x] a ficha `query word` mostra: arquivos da palavra sobre arquivos do repositório; por contexto, arquivos com a palavra sobre arquivos do contexto (com o tamanho do contexto); por vizinha forte, `P(palavra|vizinha)` e `P(vizinha|palavra)` em arquivos com o número de arquivos em comum; valores exatos conferidos na fixture
- [x] mudar `skeleton`, `context_root`, camadas, `verbs`, `roles` ou pesos de ponte faz o próximo `index` refazer o índice por completo; sem mudança de config e de arquivos, a reindexação segue sem escrever
- [x] a refeitura por mudança de config não toca nas notas nem gera evento `NOTE_*`
- [x] mesma árvore e mesma config geram a mesma saída byte a byte
- [x] medido no `easy_lease_integrated`: `brief order` deixa de vir vazio; tempos de índice e de `query word` registrados no log (não bloqueia o aceite)
- [x] indexação completa de 5.000 arquivos ≤ 60 s e reindexação sem mudança ≤ 300 ms (mesmo gerador sintético)
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas

---

## 5. Restrições e Regras
- técnicas: princípios 1 a 7 da spec §2 (determinístico, saída compacta, cobertura explícita); `memo-core` não depende de `clap` nem de MCP; dependência nova exige justificativa.
- negócio: o `memo` entrega fatos, não julga código (nem classifica domínio).
- dependências obrigatórias: TASK-2 concluída (`done`).
- regra de dados: o banco é derivado e descartável; sem migração, só recriação; notas (`.md`) não mudam.
- invariante de estado (promovido de D6): `word_stats` nunca fica calculado com uma config diferente da vigente; guarda: o hash da config em `meta` é comparado em todo `index` antes da saída antecipada.
- unidade de medida da representatividade: arquivos (a medição em símbolos separa pouco, porque palavras raramente dividem o mesmo nome).

---

## 6. Impacto em Dados
- [x] Nenhuma alteração de schema
- [ ] Uso de entidades existentes
- [ ] Leitura apenas
- [x] Escrita controlada

Observação: sem tabela ou coluna nova; usa `meta` (chave nova, por exemplo `config_hash`). Hash ausente ou diferente descarta e recria o banco derivado, como já faz com `schema_version`. `word_stats.esqueleto` passa a refletir só a lista; `coerencia` e `especificidade` seguem gravadas. A representatividade é calculada na consulta (D19: derivada).

---

## 7. Estratégia de Testes
escritor: test-analyst

**Cenários** — Happy Path: esqueleto só pela lista, `brief` com âncora de palavra frequente, hash de config, representatividade na ficha · Error Scenarios: `note add` em palavra da lista (`ancora_generica`, exit 4), banco sem hash de config · Edge Cases: `min_*` sem efeito, mudança de `min_*` não reindexa, arquivo sem contexto, vizinha com 2 arquivos em comum (abaixo do piso 3), mais de 10 vizinhas, empate alfabético, palavra sem vizinhas, contagens iguais com e sem `--no-tests`.

**Fixture da representatividade** (11 arquivos; cada arquivo define uma função com o nome exato de cada palavra que ele contém, como `def alpha`, sem sufixo comum, para nenhuma palavra ligar todos os arquivos; contexto = segmento depois de `app/`): `app/a/` 4 arquivos (`a1` a `a4`), `app/b/` 6 arquivos (`b1` a `b6`) e `app/raiz.py` sem contexto. `alpha` aparece em `a1`, `a2`, `a3`, `b1` e `raiz` (5 arquivos). `beta` aparece em `a1`, `a2`, `a3`, `b2`, `b3`, `b4` (6 arquivos, 3 em comum com `alpha`). `gamma` aparece em `a1`, `a2` e `b5` (2 em comum, abaixo do piso). Valores esperados: repositório 5 de 11; contexto `a` 3 de 4; contexto `b` 1 de 6; `beta`: `P(alpha|beta)` = 3/6 = 0,5 e `P(beta|alpha)` = 3/5 = 0,6.

**Rastreabilidade**
- critério 1 (fora da lista nunca é esqueleto) → `frequent_low_coherence_word_outside_list`, `min_thresholds_at_one`
- critério 2 (lista continua esqueleto) → `word_in_skeleton_list_when_indexed`, `..._when_brief_then_no_anchor`, `..._when_note_with_word_anchor_then_ancora_generica_error`
- critério 3 (`brief` com âncora) → `low_coherence_word_outside_list_when_brief`, `word_outside_list_when_note_with_word_anchor`
- critério 4 (informativo, `min_*` sem efeito) → `min_coherence_and_min_specificity_in_toml`, `coerencia_and_especificidade_stored`, `min_thresholds_at_one`
- critério 5 (ficha com representatividade) → os 8 testes de `word_card` e de `query word`
- critério 6 (config refaz o índice) → 9 testes de hash (unidade) e `context_root_changes`, `skeleton_changes`, `min_coherence_changed`
- critério 7 (notas intactas) → `config_change_when_rebuilt_then_notes_and_note_events_are_untouched`, `note_on_word_when_word_enters_skeleton_list`
- critério 8 (determinismo) → `same_tree_twice_when_query_word`
- critério 9 (medida no repositório real) → não coberto por teste: medição registrada no log (não bloqueia)
- critério 10 (desempenho) → não coberto por teste: medição registrada no log (meta, não bloqueia)
- critério 11 (qualidade) → comandos `cargo fmt`, `clippy` e `test`
- invariante §5 (estatísticas refletem a config vigente) → `skeleton_changes_then_esqueleto_follows`, `unchanged_files_and_config`, `database_without_config_hash`
- unidade de medida (arquivos) → fixture da representatividade e `file_without_context`
- não coberto: calibração da lista `skeleton` e do piso de 3 no repositório real; ruído de `is`/`id` no `brief` sem ranking (D8); formatos de `serve`/MCP com a ficha nova

**Lifecycle Coverage**
- [ ] Esqueleto: nasce quando a palavra entra na lista e morre quando sai → `word_in_skeleton_list_when_indexed`, `note_on_word_when_word_enters_skeleton_list`, `skeleton_changes_then_esqueleto_follows`
- [ ] Hash de config: nasce na primeira indexação, muda com a config, banco sem hash é refeito → `unchanged_files_and_config`, 9 testes de hash, `database_without_config_hash`
- [ ] Representatividade: derivada na consulta (0 arquivos, 1 contexto, N contextos, mais de 10 vizinhas) → `word_without_neighbors`, `two_contexts`, `more_than_ten_neighbors`
- [ ] Nota `word:` × esqueleto: sem transição de estado, só a marca → `note_on_word_when_word_enters_skeleton_list`
- [ ] Escritor sem guarda: nenhum além de `stats::recompute`, coberto pela invariante do §5

### Testes Unitários
- [ ] test_given_min_coherence_and_min_specificity_in_toml_when_parsed_then_values_accepted  (85 chars)
    - GIVEN a memo.toml with min_specificity = 0.9 and min_coherence = 0.9; WHEN the config is parsed; THEN it succeeds and both values are kept as given
- [ ] test_given_same_config_when_hashed_twice_then_hash_is_equal  (59 chars)
    - GIVEN one parsed config; WHEN the index config hash is computed twice; THEN both hashes are equal
- [ ] test_given_changed_skeleton_when_hashed_then_hash_differs  (57 chars)
    - GIVEN two configs that differ only in the skeleton list; WHEN the index config hash is computed for each; THEN the hashes differ
- [ ] test_given_changed_context_root_when_hashed_then_hash_differs  (61 chars)
    - GIVEN two configs that differ only in context_root; WHEN the index config hash is computed for each; THEN the hashes differ
- [ ] test_given_changed_layers_when_hashed_then_hash_differs  (55 chars)
    - GIVEN two configs that differ only in layers; WHEN the index config hash is computed for each; THEN the hashes differ
- [ ] test_given_changed_verbs_when_hashed_then_hash_differs  (54 chars)
    - GIVEN two configs that differ only in verbs; WHEN the index config hash is computed for each; THEN the hashes differ
- [ ] test_given_changed_roles_when_hashed_then_hash_differs  (54 chars)
    - GIVEN two configs that differ only in roles; WHEN the index config hash is computed for each; THEN the hashes differ
- [ ] test_given_changed_bridge_weight_when_hashed_then_hash_differs  (62 chars)
    - GIVEN two configs that differ only in one [bridges] weight; WHEN the index config hash is computed for each; THEN the hashes differ
- [ ] test_given_changed_min_coherence_when_hashed_then_hash_is_equal  (63 chars)
    - GIVEN two configs that differ only in min_coherence (and min_specificity); WHEN the index config hash is computed for each; THEN the hashes are equal, because those keys no longer affect the index

### Testes de Integração
- [ ] test_given_frequent_low_coherence_word_outside_list_when_indexed_then_not_esqueleto  (83 chars)
    - GIVEN the get_like fixture (50 modules, get_<distinct word>) and skeleton = []; WHEN the repository is indexed; THEN word_stats for get has esqueleto false
- [ ] test_given_word_in_skeleton_list_when_indexed_then_esqueleto_true  (65 chars)
    - GIVEN the get_like fixture and skeleton = ["get"]; WHEN the repository is indexed; THEN word_stats for get has esqueleto true
- [ ] test_given_min_thresholds_at_one_when_indexed_then_word_outside_list_not_esqueleto  (82 chars)
    - GIVEN the get_like fixture, skeleton = [] and min_specificity = min_coherence = 1.0; WHEN the repository is indexed; THEN word_stats for get has esqueleto false
- [ ] test_given_low_coherence_word_when_indexed_then_coerencia_and_especificidade_stored  (83 chars)
    - GIVEN the get_like fixture and skeleton = []; WHEN the repository is indexed; THEN word_stats for get keeps coerencia below 0.30 and especificidade below 0.35
- [ ] test_given_low_coherence_word_outside_list_when_brief_then_word_is_anchor  (73 chars)
    - GIVEN the get_like fixture, skeleton = [] and the request "get"; WHEN brief is computed; THEN get is the anchor, the anchor list is not empty and symbols are listed
- [ ] test_given_word_in_skeleton_list_when_brief_then_no_anchor_and_word_ignored  (75 chars)
    - GIVEN the get_like fixture, skeleton = ["get"] and the request "get"; WHEN brief is computed; THEN anchors are empty, get appears as ignored and no symbol is listed
- [ ] test_given_word_in_skeleton_list_when_note_with_word_anchor_then_ancora_generica_error  (86 chars)
    - GIVEN the get_like fixture and skeleton = ["get"]; WHEN a note with anchor word:get is added; THEN it fails with ancora_generica (exit 4 in the CLI)
- [ ] test_given_word_outside_list_when_note_with_word_anchor_then_note_is_accepted  (77 chars)
    - GIVEN the get_like fixture and skeleton = []; WHEN a note with anchor word:get is added; THEN the note is stored as proposta
- [ ] test_given_note_on_word_when_word_enters_skeleton_list_then_marked_generic_without_event  (88 chars)
    - GIVEN an active note anchored on word:get, indexed with skeleton = []; WHEN get is added to skeleton and the repository is indexed again; THEN the index is rebuilt, the note stays active with the generic anchor mark and no NOTE_* event is recorded
- [ ] test_given_indexed_repo_when_context_root_changes_then_next_index_rebuilds_contexts  (83 chars)
    - GIVEN a repository indexed with context_root = "app" and no file changed; WHEN context_root changes to "app/orders" and index runs again; THEN file contexts follow the new root and the report says the index was rebuilt
- [ ] test_given_indexed_repo_when_skeleton_changes_then_esqueleto_follows_without_file_change  (88 chars)
    - GIVEN a repository indexed with skeleton = []; WHEN skeleton becomes ["get"] with no file changed and index runs again; THEN word_stats for get has esqueleto true
- [ ] test_given_unchanged_files_and_config_when_indexed_then_nothing_is_written  (74 chars)
    - GIVEN a repository indexed once; WHEN index runs again with the same files and the same config; THEN the report says nothing was written and the config hash is unchanged
- [ ] test_given_min_coherence_changed_when_indexed_then_index_is_not_rebuilt  (71 chars)
    - GIVEN a repository indexed once; WHEN only min_coherence changes and index runs again; THEN nothing is written and the index is not rebuilt
- [ ] test_given_config_change_when_rebuilt_then_notes_and_note_events_are_untouched  (78 chars)
    - GIVEN two notes in the notes directory and an indexed repository; WHEN the skeleton changes and the index is rebuilt; THEN the note files are byte-identical and no NOTE_* event was added
- [ ] test_given_database_without_config_hash_when_indexed_then_it_is_rebuilt_and_hash_stored  (87 chars)
    - GIVEN a derived database with schema_version 2 and no config hash in meta; WHEN index runs; THEN the database is rebuilt and meta now holds the config hash
- [ ] test_given_fixture_when_word_card_then_repository_share_is_files_with_word_over_files  (85 chars)
    - GIVEN the representativeness fixture (see below); WHEN the word card for alpha is computed; THEN the repository share is 5 files over 11 files
- [ ] test_given_files_in_two_contexts_when_word_card_then_share_per_context_with_size  (80 chars)
    - GIVEN the representativeness fixture; WHEN the word card for alpha is computed; THEN context a shows 3 of 4 files and context b shows 1 of 6 files
- [ ] test_given_file_without_context_when_word_card_then_counted_in_repository_only  (78 chars)
    - GIVEN the representativeness fixture, with alpha also in one file directly under app/; WHEN the word card for alpha is computed; THEN that file counts in the repository share and in no context
- [ ] test_given_neighbor_with_three_shared_files_when_word_card_then_both_directions_listed  (86 chars)
    - GIVEN the representativeness fixture where beta shares 3 files with alpha, alpha is in 5 files and beta in 6; WHEN the word card for alpha is computed; THEN beta is listed with P(alpha|beta) = 3/6 = 0.5, P(beta|alpha) = 3/5 = 0.6 and 3 shared files
- [ ] test_given_neighbor_with_two_shared_files_when_word_card_then_neighbor_not_listed  (81 chars)
    - GIVEN the representativeness fixture where gamma shares only 2 files with alpha; WHEN the word card for alpha is computed; THEN gamma is not listed
- [ ] test_given_more_than_ten_neighbors_when_word_card_then_ten_strongest_listed_in_order  (84 chars)
    - GIVEN a word that shares 3 or more files with 12 neighbors, with distinct shared counts; WHEN the word card is computed; THEN 10 neighbors are listed, by shared files descending, and the 2 weakest are absent
- [ ] test_given_equal_shared_files_when_word_card_then_ties_ordered_alphabetically  (77 chars)
    - GIVEN a word with three neighbors that each share 3 files with it; WHEN the word card is computed; THEN the neighbors are listed in alphabetical order
- [ ] test_given_word_without_neighbors_when_word_card_then_neighbor_section_is_empty  (79 chars)
    - GIVEN a word that appears in one file and shares no file count of 3 or more with any other word; WHEN the word card is computed; THEN the neighbor section is empty and the repository share is still shown
- [ ] test_given_fixture_when_query_word_then_representativeness_section_matches_golden  (81 chars)
    - GIVEN the representativeness fixture; WHEN memo query word alpha runs; THEN the output matches the insta golden, with the repository, context and neighbor lines
- [ ] test_given_fixture_when_query_word_json_then_representativeness_fields_present  (78 chars)
    - GIVEN the representativeness fixture; WHEN memo query word alpha --json runs; THEN the JSON has repository, contexts and neighbors with the counts above
- [ ] test_given_same_tree_twice_when_query_word_then_output_is_byte_identical  (72 chars)
    - GIVEN the representativeness fixture indexed twice from scratch; WHEN memo query word alpha runs on each; THEN both outputs are byte-identical
- [ ] test_given_no_tests_flag_when_query_word_then_representativeness_counts_unchanged  (81 chars)
    - GIVEN the representativeness fixture with one test file containing alpha; WHEN memo query word alpha runs with and without --no-tests; THEN the representativeness counts are equal in both outputs

**FIRST**
- [x] Fast: indexação em memória (`SqliteStore::open_in_memory`) sobre árvore temporária pequena, sem rede nem relógio
- [x] Isolated: cada teste monta o próprio repositório temporário; o `insta` guarda só o golden do CLI
- [x] Repeatable: datas e `gpgsign` fixados pelo helper de repositório; contagens por construção
- [x] Self-validating: cada teste afirma valores exatos (contagens e razões da fixture)
- [x] Timely: escritos antes do código; todos os testes de esqueleto e de representatividade falham hoje (regra automática ainda ativa e ficha sem a seção)

---

## 8. Dependências
- tasks predecessoras: TASK-2
- módulos | serviços externos: nenhum
- flags | configurações: `memo.toml` (`min_specificity` e `min_coherence` sem efeito)

---

## 9. Observações
Esta TASK nasce de uso real e de três rodadas de medição em `/tmp/eli`. D1 e D2 vieram do humano; D3 a D8 são recomendações a aprovar.

### N_PARTITE (DEFINITION)
[N_PARTITE] modo: concept | alvo: snapshot | referência: — | perfil: stacked + core | budget: 10 | frame: base | fonte: reescrita da DEFINITION em 2026-11-08 (frame manual; a skill n-partite não foi executada como CALL)

Elementos: L4 grafo de palavras · índice derivado | L2 Palavra · Âncora · Esqueleto (lista) · Config do índice (hash) · Representatividade | L3 decisão de esqueleto · detecção de config alterada · recriação do índice · cálculo da representatividade | L1 lista `skeleton` · hash de config · especificidade · coerência.
lifecycle.onto: Esqueleto (decisão do humano, vive na config; deixa de ser estado derivado do corpus) · Hash de config (nasce na primeira indexação, muda com a config, morre com o banco) · Representatividade (derivada na consulta, sem identidade) — `não definido`=0.

[N_PARTITE:concept] par: Esqueleto (L2) ↔ Âncora (L2) — acoplamento: alto | evidência: medido (`brief order` vazio por esqueleto automático) | abstração: esqueleto passa a ser decisão explícita, não inferência | lifecycle: divergente (o corpus muda, a lista não) → coerente depois da mudança
[N_PARTITE:concept] par: Config do índice (L2) ↔ Esqueleto (L2) — acoplamento: alto | evidência: medido em `/tmp/eli` (config muda, índice não acompanha) | abstração: o índice precisa de identidade da config | lifecycle: divergente (o banco vive mais que a config) → hash em `meta` com recriação
[N_PARTITE:concept] par: Representatividade (L2) ↔ Palavra (L2) — acoplamento: baixo | evidência: medido em arquivos | abstração: informação mostrada, não filtro | lifecycle: coerente (derivada)
[N_PARTITE.INV?] invariante: `word_stats` reflete a config vigente | escritor: `stats::recompute` (só roda quando há arquivo mudado) | guarda: ausente hoje | nível: confirmado pela medição, promovido a §5 pela D6

### Decisões (grill-me)
Origem: humano, 2026-11-08, respostas à análise de impacto, às medições e à correção de rumo.
- D1: o objetivo é `brief order` trazer resultado por interconexões; o `memo` não decide se uma palavra é de domínio. (humano)
- D2: a lista `skeleton` continua bloqueando âncora; a regra automática por especificidade e coerência sai da decisão de esqueleto. (humano)
- D3: especificidade e coerência continuam calculadas e mostradas, só informativas. (recomendação)
- D4: `min_specificity` e `min_coherence` continuam aceitas em `memo.toml`, sem efeito, por compatibilidade. (recomendação)
- D5: o hash de config cobre só o que afeta o índice (`skeleton`, `context_root`, camadas, `verbs`, `roles`, pesos de ponte), não o arquivo inteiro. (recomendação; `min_*` ficam de fora porque deixam de afetar)
- D6: config alterada refaz o índice por completo; a invariante vai para §5. (recomendação)
- D7: a ficha `query word` mostra a representatividade em arquivos: frente ao repositório, frente a cada contexto e frente às vizinhas mais fortes nos dois sentidos, com piso de 3 arquivos em comum para não inflar palavras raras. (recomendação; unidade e piso vêm das medições)
- D8: o ranking do `brief` por representatividade fica para uma TASK seguinte. (recomendação)

### Premissas
- [DEFINITION] A numeração D1 a D8 é local a esta TASK.
- [DEFINITION] Verificado em `/tmp/eli`: sem a regra automática, `order` não está na lista `skeleton` padrão, então vira âncora.
- [DEFINITION] Os testes da TASK-2 que dependem do esqueleto automático (fixtures `order_like`/`get_like`, `note_on_word_turned_esqueleto`, `low_idf_and_*`) precisam ser reescritos nesta TASK; isso é consequência de D2.
- [DEFINITION] O piso de 3 arquivos em comum é inicial; a calibração é do humano.
- [TEST_ANALYSIS] A ficha lista as 10 vizinhas mais fortes: o mesmo `TOP_NEIGHBORS = 10` já usado em `stats.rs`; ordem: arquivos em comum decrescente, depois nome crescente. Número e ordem verificados no código, não perguntados.
- [TEST_ANALYSIS] Um arquivo sem contexto (diretamente sob `app/`) conta no repositório e em nenhum contexto; a fixture confere o comportamento do `context_root` atual e precisa ser validada na ENGINEERING antes do red.
- [TEST_ANALYSIS] `--no-tests` só filtra a saída (D12 da TASK-2): as contagens da representatividade incluem arquivos de teste nos dois casos.

### Plano de Execução
#### Etapas
1. Esqueleto só pela lista: `stats.rs` deixa de aplicar a regra automática; `coerencia` e `especificidade` seguem gravadas; `min_*` aceitos sem efeito; reescrever os testes e goldens que dependiam da regra — depende de: NONE
2. Hash de config em `meta`; `index` compara antes da saída antecipada e refaz tudo quando muda; notas intactas — depende de: NONE
3. Ficha `query word` com representatividade (repositório, contextos, vizinhas), em texto e JSON — depende de: 1
4. Medições no repositório sintético e no `easy_lease_integrated`; spec (§5.6, A7, A12) e README — depende de: 1, 2, 3
#### Gates
- após cada etapa: testes da etapa e `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` limpos
- após a etapa 2: reindexação sem mudança segue sem escrever e dentro de 300 ms
- após a etapa 4: medidas registradas no log (não bloqueiam o aceite)
#### Riscos
- sem a regra automática, palavras de infraestrutura fora da lista (`is`, `id`, `usecase`) viram âncora e o `brief` traz ruído → a lista `skeleton` é editável; o ranking por representatividade (D8) é a resposta e fica para a TASK seguinte
- reescrever testes da TASK-2 em lote esconde regressão → revisar cada golden lendo, um a um
- o hash de config subestima algo que afeta o índice → teste que muda cada chave coberta e confere a reindexação
- recriar o banco por mudança de config é caro em repositório grande → só quando o hash muda; medir o índice completo
- a ficha fica longa → piso de co-ocorrência e limite de vizinhas
#### Assunções
- o banco derivado é descartável, então recriar é aceitável
- a lista `skeleton` e os pesos são ajustados pelo humano no repositório real
- precisa existir uma fixture de palavra frequente com coerência baixa, fora da lista, e contextos com tamanhos distintos para a representatividade

### N_PARTITE (CODE_REVIEW)
[N_PARTITE] modo: multi (concept×code) | alvo: delta | referência: — | perfil: stacked + full | budget: 10 | frame: base | fonte: dfc4077..7a21ca5
- REUSE: miss (sem frame de CODE_REVIEW anterior para esta TASK); frame base. Frame escrito à mão a partir da leitura do diff, sem executar a skill n-partite como CALL (limitação registrada).
[N_PARTITE:code] par: Config (L2) ↔ index_hash (L3) — acoplamento: alto | evidência: não definido (sem co-mudança medida) | abstração: o hash vive em `Config`, junto das chaves que cobre | lifecycle: coerente (muda quando a config muda) → nenhuma
[N_PARTITE:code] par: run_index (L3) ↔ meta.config_hash (L1) — acoplamento: alto | evidência: não definido | abstração: comparar antes da saída antecipada e gravar depois do commit | lifecycle: divergente (o banco vive mais que a config) → coerente com o hash; falha entre o commit e `set_meta` apenas refaz na próxima vez
[N_PARTITE:code] par: representativeness (L4) ↔ word_core (L3) — acoplamento: médio | evidência: não definido | abstração: módulo próprio; `word_core` só decide chamar ou não | lifecycle: coerente (derivada)
[N_PARTITE:code] par: word_core (L3) ↔ weights: Option (L1) — acoplamento: alto | evidência: o flag `Some/None` agora decide três coisas (símbolos etiquetados, pontes, representatividade) | abstração: dois tipos de saída (padrão e ficha) | lifecycle: divergente → tipo próprio `WordPattern` (A2 da TASK-2, ainda aberta)
[N_PARTITE.STRUCT:code] alvo: word_core (query.rs) — problema: `Option<&BridgeWeights>` funciona como flag de modo e já controla 3 efeitos | estrutura proposta: `word_pattern` e `word_card` como funções separadas | base: `match weights { Some(_) ... None ... }` ×3
[N_PARTITE.STRUCT:code] alvo: `Config::to_toml` e `init` — problema: continuam gravando `min_specificity` e `min_coherence`, que agora não têm efeito | estrutura proposta: parar de gravar ou gravar comentadas | base: `crates/memo-cli/tests/init.rs:73` espera `min_specificity = 0.35`
[N_PARTITE.DRIFT] par: D2 (esqueleto só pela lista) ↔ stats.rs — situação: sem drift | par: D7 (representatividade) ↔ representativeness.rs — situação: sem drift (10 vizinhas, piso 3, arquivos) | par: D5 (hash) ↔ index_hash — situação: desvio declarado e registrado: `write_methods` entra no hash, fora do texto da D5 → registrado no log, não é drift de intenção
[N_PARTITE.FAIL] elemento: run_index + invalidate_files — falha: interrupção entre `invalidate_files` e o fim | proteção: os arquivos ficam invalidados (`hash=''`) e `config_hash` não é gravado, então a próxima execução refaz tudo | lacuna: nenhum teste cobre a interrupção
[N_PARTITE.GAP] completeness: não definido (sem referência)
[N_PARTITE.CLUSTER] bridges.rs, query.rs, stats.rs e representativeness.rs mudam juntos nas etapas 1, 3 e 10 da TASK-2/3; a vizinhança é calculada de dois modos independentes (pontes por palavra, `core_neighborhoods` em lote) e agora de um terceiro (representatividade por arquivos) → três cálculos de "vizinha" sem teste que os cruze
[N_PARTITE.INV?] invariante: `word_stats` e o índice refletem a config vigente | escritor: `run_index` | guarda: o hash em `meta` comparado antes da saída antecipada | nível: guardado e testado (`config_rebuild.rs`, 6 testes); invariante inferido: `memo.toml` com uma chave fora do hash não muda o índice — vale para `entrypoint_decorators` (sem consumidor no código), mas `main_branch` e `stale_on` afetam notas e ref, não o grafo
[N_PARTITE.SIDE] efeito colateral: o `index` passa a poder reextrair todos os arquivos sozinho quando a config muda (custo de tempo no próximo comando de consulta: 9 s em 801 arquivos, ~42 s em 5.000) | já declarado na spec e no README
[N_PARTITE.RANK] 1) `.query.rs.pending-snap` rastreado · 2) `word_core` como flag (A2 aberta) · 3) `init` grava chaves inertes · 4) sem teste de `write_methods` no hash e sem teste de interrupção · 5) três cálculos de vizinhança

### Code Review
#### [2026-11-08T23:40-03:00] — 7a21ca5b
- WT: /var/home/erickod/Documentos/dev/memmo (branch master; sem PR, revisão local; `.agents`, `.mcp.json` e `CLAUDE.md` não rastreados, gerados pelo `memo init`)
- lang sem reference: rust (só CRITERIA base)
- flags: ddd, perf_deep e refactor_only não ativadas na TASK (§9/Plano/Assunções); não inferidas.

**SUMMARY — Aprovado c/ ressalvas**
- Positivos: os 8 commits (`dfc4077..7a21ca5`) seguem red e green, com testes escritos antes; cada decisão do §9 (D2 a D7) tem código e teste; invariante do §5 guardada por hash e coberta por 6 testes; 292 testes passam, `fmt` e `clippy` limpos; metas medidas (42 s, 0,04 s, 0,36 s); spec e README acompanham o código.
- Preocupações: (1) um artefato do `insta` (`.query.rs.pending-snap`) foi commitado em [cf5de65](https://github.com/erickod/memo/commit/cf5de659b0935da9327ed817fd35be7b9651e950) e muda a cada execução de teste; (2) `word_core` usa `Option<&BridgeWeights>` como flag e já controla 3 efeitos; (3) `init` ainda grava `min_specificity` e `min_coherence`, agora sem efeito; (4) sem teste de `write_methods` no hash nem da interrupção do rebuild; (5) a revisão não inclui o `serve`/MCP.

**ANALYSIS**
- [ ] design: `Config::index_hash`, `representativeness` e `Corpus` (da etapa anterior) coesos; `word_core` mistura modos (achado A2).
- [ ] DDD: não identificado — verificado em: TASK §9/Plano (flag ddd não ativada; não inferida).
- [ ] reliability: `run_index` (index.rs:430-445, 558-562): `invalidate_files` fora da transação, `set_meta` depois do commit; interrupção em qualquer ponto refaz tudo na próxima, sem corrupção (arquivos com `hash=''` e `config_hash` antigo); idempotência: reindexar duas vezes dá o mesmo estado; dual write: não há (um banco só). Lacuna: sem teste de interrupção (A4).
- [ ] data: não identificado — verificado em: diff 7a21ca5 (sem valor monetário nem PII).
- [ ] contract: `WordInfo` ganhou o campo `representativeness` (JSON aditivo); `IndexReport` ganhou `rebuilt`; `Config` ganhou `index_hash`; consumidores por `grep -w` em `crates/`: `word_info` só em `memo-cli/src/main.rs`, `IndexReport` em `index.rs`/`workspace.rs`; `min_*` seguem no `Config` (compatibilidade); `serve`/MCP não verificado.
- [ ] env_k8s: não identificado — verificado em: diff sem settings nem manifest.
- [ ] concurrency: `run_index` roda sob o lock por banco (30 s) em `workspace.rs`; `rebuilt` não altera isso; não identificado.
- [ ] migrations: o banco derivado sem `config_hash` é refeito uma vez (sem migração; notas e eventos intactos); não identificado problema.
- [ ] perf: `representativeness` carrega todos os pares arquivo×palavra por consulta (O(ocorrências)) e intersecta cada palavra com os arquivos da consulta; medido 2,3 s no pior caso sintético, 0,21 s no repositório real (801 arquivos); `index_hash` desprezível; rebuild custa a indexação completa (declarado). Sem N+1.
- [ ] security: não identificado — verificado em: SQL do diff é texto fixo; sem entrada do usuário concatenada.
- [ ] errors: não identificado — verificado em: `?` com `CoreError` nas consultas novas; sem `unwrap` em código de produção; `by_word[&id]` e `texts[id]` indexam ids da própria consulta.
- [ ] readability: `representativeness_lines` e `representativeness` legíveis; `word_core` cresce com mais um `match weights`.
- [ ] layers: não identificado — verificado em: imports do diff (`memo-core` sem `clap` nem MCP; render só em `memo-cli`).
- [ ] tests: §7 presente com nomes exatos (9 unidade, 27 integração); extras só os 2 de apoio à fixture; golden do CLI embutido (desvio declarado); falta teste de `write_methods` no hash e de interrupção (A4).
- [ ] task: §4 critérios 1–11 com teste ou medição; §5 invariante com guarda e testes; D2 por `stats.rs:154`, D3/D4 por `skeleton_list.rs`, D5 por `config_hash.rs`, D6 por `config_rebuild.rs`, D7 por `representativeness.rs`; D8 fora do escopo, registrada.
- [ ] format: não identificado — verificado em: `cargo fmt --check` OK; `clippy -D warnings` sem avisos.
- [ ] implicit: `min_*` aceitos mas sem efeito é decisão D4, documentada na spec e no README; `to_toml`/`init` ainda os escrevem (achado A3).
- [ ] refactor_estrutural: não identificado — verificado em: a mudança de `stats.rs` remove só a condição automática; nenhuma duplicação colateral.
- [ ] smells¹: Long Params — `representativeness` com 2; Long Method — `representativeness` ~85 linhas (três cálculos no mesmo corpo), no limite; Feature Envy/Msg Chains/Magic Numbers (`MIN_SHARED_FILES` e `MAX_NEIGHBORS` nomeados)/Ignored Exceptions/Dead Code — não identificado; Dead Code observado fora do diff: `entrypoint_decorators` não tem consumidor (A5).
- [ ] N_PARTITE: §9/N_PARTITE (CODE_REVIEW); perfil `stacked + full`, REUSE miss, frame base manual.
- [ ] drift (DEFINITION × código): base e deltas do §9/N_PARTITE (DEFINITION) comparados no mesmo frame — DRIFT=0; desvio de `write_methods` no hash registrado no log da etapa 2.

**VERIFY** — reli o diff `dfc4077..7a21ca5` sem olhar a ANALYSIS: sem omissão além de A1–A5; conferi que `representativeness` é determinística (ordens totais por contagem e nome) e que não divide por zero (a lista de vizinhas só existe com `files.len() >= 3`).

**ACTIONS**
- 🟡 A1 — `crates/memo-cli/tests/.query.rs.pending-snap` rastreado, commitado em [cf5de65](https://github.com/erickod/memo/commit/cf5de659b0935da9327ed817fd35be7b9651e950) e reescrito a cada `cargo test`. Correção: `git rm --cached` e acrescentar `*.pending-snap` ao `.gitignore`. before: arquivo no índice do git; after: ignorado.
- 🟡 A2 — `word_core` (query.rs): `Option<&BridgeWeights>` virou flag de modo para 3 efeitos. Correção: `word_pattern` e `word_card` como funções separadas, com `WordPattern` para o `brief`, mais um teste de paridade de verbos, papéis e camadas. Dívida da TASK-2 (A2) que cresceu.
- 🟢 A3 — `Config::to_toml` e o `init` ainda gravam `min_specificity` e `min_coherence`, sem efeito; novos usuários recebem chaves inertes. Sugestão: parar de gravar, ou gravar comentadas com aviso (ajustar `init.rs:73`).
- 🟢 A4 — teste de `write_methods` no hash e teste de interrupção do rebuild (`invalidate_files` seguido de falha) que travem a recuperação.
- 🟢 A5 — `entrypoint_decorators` não tem consumidor no código: chave morta, anterior a esta TASK.
- ⚖️ A6 — três cálculos de "vizinha" (pontes por palavra, `core_neighborhoods`, representatividade): (a) manter, cada um responde a uma pergunta diferente; (b) extrair a carga arquivo×palavra e compartilhar com `bridges::Corpus`; (c) cachear a carga no processo (`serve`). Recomendação: (a) agora; (b) se aparecer um quarto uso.
- ❓ nenhuma pergunta ao autor com 🔴 potencial.

**COMMITS**
- ✓ [e7ff342](https://github.com/erickod/memo/commit/e7ff3420be28cd21aa91434db682d2d551db058e) test: add skeleton list only esqueleto tests — red genuíno (6 falhando); a mensagem confere. ⚠ saiu com um aviso do clippy (variável sem uso), corrigido em [9f6e7ff](https://github.com/erickod/memo/commit/9f6e7fff59cbe6e4779f9789960e82e2db32e27f): o histórico tem um commit que não passa `clippy -D warnings`.
- ✓ [7ee8abd](https://github.com/erickod/memo/commit/7ee8abdc31ff7a917334a248b56d0f7e5fa0b816) feat: decide esqueleto only by the skeleton list — spec §5.6/A7 e README no mesmo commit.
- ✓ [9f6e7ff](https://github.com/erickod/memo/commit/9f6e7fff59cbe6e4779f9789960e82e2db32e27f) test: drop unused config binding in coherence test.
- ✓ [a9923c1](https://github.com/erickod/memo/commit/a9923c1e9e446b64a5c6a4ff75c064f271b5a31c) test: add config hash and index rebuild tests — red por asserção, com stubs.
- ✓ [31e550f](https://github.com/erickod/memo/commit/31e550ff7d037b4bfe871e926985bd80520aa917) feat: rebuild the index when the config that shapes it changes.
- ⚠ [cf5de65](https://github.com/erickod/memo/commit/cf5de659b0935da9327ed817fd35be7b9651e950) test: add word representativeness card tests — red por asserção, mas inclui `.query.rs.pending-snap` por engano (A1).
- ✓ [bda849f](https://github.com/erickod/memo/commit/bda849fa1345071f939bcc7ddc730a1a0c79f435) feat: show word representativeness by repository, context and neighbor — 3 goldens antigos atualizados após leitura dos diffs.
- ✓ [7a21ca5](https://github.com/erickod/memo/commit/7a21ca5b7ff951d01ad676ee5e792290344d2b52) docs: document config hash, skeleton list and word representativeness.
- Mensagens seguem a regra de commit sem atribuição da TASK.

**EVIDÊNCIAS**
- SHA revisado: 7a21ca5b7ff951d01ad676ee5e792290344d2b52 (HEAD); sem PR, então `headRefOid` não se aplica; `repo_url` via `gh repo view --json url -q .url`: https://github.com/erickod/memo.
- Comandos: `cargo fmt --check` OK; `cargo clippy --all-targets -- -D warnings` 0 avisos; `cargo test --no-fail-fast` 292 passam, 0 falham (a execução alterou `.query.rs.pending-snap`, que restaurei).
- Medidas (repositório sintético de 5.000 arquivos): índice 42 s, sem mudança 0,04 s, `brief` 0,36 s, `query word total` 1,1 s, `limit` 2,3 s; cópia do repositório real (801 arquivos): índice 9 s, `query word order` 0,21 s.
- Consumidores: `grep -w word_info|IndexReport|index_hash|representativeness` em `crates/`; Semble não foi consultado nesta review; vazio não prova ausência.
- Não executado: `serve`/MCP com o campo novo; repositório sem contexto; teste do binário instalado no repositório do humano; teste de interrupção do rebuild; revisão em PR (não há).


## 10. Log de Decisões e Alterações

### [2026-11-08T16:00-03:00] — Criação da TASK-3
- Tipo: criação
- Contexto: pedido do humano (`1`) depois da análise de impacto e da medição em `/tmp/eli`; escopo e recomendações D3 a D6 aceitos.
- Decisão tomada: abrir a TASK-3 em DEFINITION, `Estado: draft`. N_PARTITE em perfil `core` sobre o conceito (a TASK tem poucos elementos; `full` fica para o code review). Grill-me: as decisões já vieram respondidas na conversa; nenhuma pergunta nova de design pendente. §7 fica para TEST_ANALYSIS.
- Impacto: STATE=DEFINITION, aguardando a aprovação única para seguir a TEST_ANALYSIS.
- Não verificado: a lista padrão de palavras de função; o efeito real nas outras palavras do repositório do humano.

### [2026-11-08T16:40-03:00] — Hipótese de `function_words` refutada; DEFINITION reaberta
- Tipo: alteração (bloqueio antes da TEST_ANALYSIS)
- Contexto: ao fixar valores de teste, medi o efeito da mudança antes de escrever o red. Protótipo descartável (filtro por variável de ambiente em `stats.rs`, revertido; `git status` limpo) sobre a cópia `/tmp/eli` do repositório real: filtrar 32 palavras de função (`is`, `and`, `the`, `with`, `a`, `by`, `to`, `of`, `id`, `get`...) leva a `coer` de `order` de 0,21 para 0,20. Não sai do esqueleto.
- Medida de outras palavras (mesmo índice, sem filtro): `order` 0,21 · `reservation` 0,27 · `payment` 0,29 · `account` 0,25 · `event` 0,21 · `usecase` 0,20 · `item` 0,25 · `hold` 0,22 (todas esqueleto), contra `id` 0,48 · `get` 0,36 · `is` 0,35 · `cancel` 0,40 · `status` 0,31 (nenhuma esqueleto, salvo `get` que está na lista). Ou seja: a coerência (top-10 sobre o total) não separa conceito de ruído neste repositório; ela cresce quando a palavra tem poucas vizinhas, e favorece palavras genéricas de cauda curta. Palavras de domínio com muitas vizinhas (grau 180 a 377) saem com coerência baixa.
- Consequência: D1, D3 e D5 (e os critérios 2 a 4 do §4) partem de uma premissa errada e ficam suspensos. Continua válido e independente: o índice não acompanha mudanças de config (D4, D6, critérios 5 e 6, invariante do §5).
- Estado: STATE volta para DEFINITION, `Estado: draft`; sem TEST_ANALYSIS até o humano decidir o novo caminho.

### [2026-11-08T17:30-03:00] — DEFINITION reescrita
- Tipo: alteração
- Contexto: depois das medições de representatividade (pares, repositório e contextos) o humano corrigiu o objetivo (não classificar domínio; foco em interconexões) e confirmou manter a lista `skeleton` removendo só a regra automática.
- Decisão tomada: reescrever §1 a §9. Saem `function_words` e a concentração entre contextos como critério (a segunda classificaria domínio); ficam o hash de config, o esqueleto só pela lista e a representatividade na ficha. D1 e D2 do humano; D3 a D8 são recomendações a aprovar.
- Registro das medições (cópia `/tmp/eli`): filtrar 32 palavras de função não muda `order` (0,21 → 0,20); coerência: `order` 0,21, `reservation` 0,27, `payment` 0,29, `account` 0,25, `event` 0,21, `usecase` 0,20, `item` 0,25, `hold` 0,22, contra `id` 0,48, `get` 0,36, `is` 0,35, `cancel` 0,40, `status` 0,31; pares em arquivos: `P(order|payment)` 0,59 e `P(payment|order)` 0,33, `P(order|cancel)` 0,77 e `P(cancel|order)` 0,19, `P(order|id)` 0,33, `P(order|is)` 0,24; concentração entre contextos (maior sobre a média, contextos com ao menos 30 arquivos): conceitos 3,3 a 5,5, demais 1,6 a 2,4.
- Limites das medições: um repositório só, os mesmos 14 termos, `api` (5 arquivos) distorcia e foi excluído, sem validação em palavras novas.
- Impacto: STATE=DEFINITION, `Estado: draft`, aguardando a aprovação única; o N_PARTITE desta versão é um frame manual, a skill não foi executada como CALL.
- Não verificado: o efeito do `brief order` sem a regra automática (só previsto pela premissa); os testes da TASK-2 que quebram (contados só na TEST_ANALYSIS).

### [2026-11-08T18:10-03:00] — TEST_ANALYSIS
- Tipo: alteração
- Contexto: aprovação da DEFINITION (`1`); `test-analyst` escreveu o §7 com 9 testes de unidade e 27 de integração, todos com nome `given_..._when_..._then_...` de até 89 caracteres e GIVEN/WHEN/THEN em inglês.
- Decisão tomada: valores esperados vêm da fixture da representatividade (contagens por construção, D7) e do teste atual de `get_like` da TASK-2 (coerência < 0,30 e especificidade < 0,35 com a regra antiga). Três premissas registradas no §9: 10 vizinhas, ordem por arquivos em comum e alfabética, arquivo sem contexto fora dos contextos.
- Lacunas verificadas no ambiente antes de perguntar: nenhuma pergunta aberta; o número de vizinhas veio de `TOP_NEIGHBORS` em `stats.rs`. GRILL não chamado (sem lacuna sem resposta).
- Não coberto por teste: calibração, ruído de `is`/`id` sem ranking, `serve`/MCP com a ficha nova, medições no repositório real.
- Impacto: STATE=TEST_ANALYSIS concluída; aguardando o `1` para a ENGINEERING. N_PARTITE reaproveitado (frame da DEFINITION), sem nova CALL.

### [2026-11-08T19:20-03:00] — Etapa 1: esqueleto só pela lista
- Tipo: alteração
- Contexto: alteração atômica 1 aprovada (`1`, incluindo a troca dos testes obsoletos da TASK-2); commits `e7ff342` (test, red: 6 testes falhando), `7ee8abd` (feat, green) e `9f6e7ff` (test: remove um binding sem uso que o clippy apontou no red).
- Decisão tomada (D2, D3, D4): em `stats.rs`, `is_skeleton = skeleton.contains(word)`; especificidade e coerência continuam gravadas; `min_specificity` e `min_coherence` seguem aceitos e validados, sem efeito (comentário em `config.rs`); spec §5.6 e A7 e README ajustados no mesmo commit para não contradizer o código.
- Testes da TASK-2 trocados: `coherence.rs` perdeu `low_idf_and_low_coherence_..._esqueleto` (coberto pelo novo teste 1), o de edição de docstring agora espera `coerencia < 0,30` e `!esqueleto`, o de nota passa a pôr `hub` na lista antes de reindexar; no CLI, `get_like` agora espera que `get` não seja esqueleto.
- Novos testes (`crates/memo-core/tests/skeleton_list.rs`): 8 do §7 de esqueleto e brief e nota; ficam para as etapas 2 e 3 os de hash de config e de representatividade.
- Impacto: 264 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos.
- Falha de processo: o primeiro commit red saiu com um aviso do clippy (variável sem uso) que eu não conferi antes de commitar, porque o comando usava `;`; corrigido no commit seguinte, sem reescrever histórico.
- Medição na cópia `/tmp/eli` (índice refeito com `rm base.db`, porque o bug da config ainda existe): `brief order` deixa de vir vazio (`âncoras: order(1.00)`), 0,04 s. A qualidade do ranking é ruim: os primeiros símbolos são `Entity.get_events` e `PaymentGatewayRegistry.get` por vizinhança (etiqueta `vizinho`), antes dos métodos de `OrderEntity`, e o contorno traz listas longas de chamadores aproximados. É o risco previsto (D8: ranking por representatividade fica para a TASK seguinte).
- Não verificado: o `brief order` no repositório do humano (só na cópia); efeito em outras palavras.
- Critérios afetados: 1, 2, 3, 4

### [2026-11-08T20:40-03:00] — Etapa 2: hash de config e índice refeito
- Tipo: alteração
- Contexto: alteração atômica 2 aprovada (`1`); commits `a9923c1` (test, red: 12 falhando por asserção, com stubs `index_hash` e `IndexReport.rebuilt`) e `feat: rebuild the index when the config that shapes it changes` (green).
- Decisão tomada (D5, D6): `Config::index_hash()` = blake3 de `skeleton`, `context_root`, `layers`, `verbs`, `roles`, `write_methods` e os 8 pesos de `[bridges]`, com listas ordenadas; `run_index` compara com `meta.config_hash` antes da saída antecipada e, se diferir e já houver arquivos, chama `invalidate_files` (reextrai tudo; sem apagar o banco, então eventos e notas ficam intactos); o hash é gravado depois do commit e na saída antecipada; `IndexReport.rebuilt` informa a refeitura.
- Duas decisões minhas, fora do texto da D5: (1) `write_methods` entra no hash, porque define as arestas `ESCREVE` gravadas no índice (`resolve.rs`); (2) `entrypoint_decorators`, `stale_on`, `main_branch` e `min_*` ficam de fora (não alteram o que o índice grava; `entrypoint_decorators` só aparece em `config.rs`). Sem teste dedicado para `write_methods`: o §7 só cobre as chaves da D5.
- Erro meu no teste: a nota do teste de notas ancorava numa palavra inexistente (`wcb`); corrigido antes do green.
- Impacto: 280 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. Medição: reindexação sem mudança no repositório sintético de 5.000 arquivos 0,04 s (meta 300 ms); na cópia do repositório real (801 arquivos), trocar `context_root` agora refaz o índice sozinho em 9,6 s, sem eventos novos (`+0 símbolos`), e as pontes de import perdem `easy`, `lease` e `backend`; sem mudança, 0,01 s.
- Não verificado: o tempo da indexação completa de 5.000 arquivos depois desta mudança (etapa 4); o CLI não imprime ainda que o índice foi refeito (`rebuilt` só existe no relatório).
- Critérios afetados: 6, 7 (hash e notas), invariante do §5

### [2026-11-08T22:10-03:00] — Etapa 3: representatividade na ficha
- Tipo: alteração
- Contexto: alteração atômica 3 aprovada (`1`); commits `cf5de65` (test, red: 12 falhando por asserção, com módulo `representativeness` em stub) e `feat: show word representativeness by repository, context and neighbor` (green).
- Decisão tomada (D7): novo módulo `memo-core/src/representativeness.rs` calcula, em arquivos e na consulta, o repositório (arquivos com a palavra sobre arquivos totais), cada contexto (só contextos onde a palavra aparece, por fração decrescente e nome) e as 10 vizinhas mais fortes com piso de 3 arquivos em comum (ordem: arquivos em comum decrescente, depois nome), com `P(palavra|vizinha)` e `P(vizinha|palavra)`; `WordInfo.representativeness` (opcional, vazio no padrão do `brief`); `query word` imprime a seção `representatividade (arquivos):` e o JSON traz `data.representativeness`. Só nomes de símbolos contam; arquivo sem contexto entra no repositório e em nenhum contexto.
- Desvio do §7: o golden do CLI é um snapshot `insta` embutido só da seção da representatividade (e não o arquivo de saída inteira), porque a saída completa de `query word` já tem golden próprio.
- Erros meus: a fixture usava módulos `a1`…`b6`, e `a1` vira a palavra `a`, que entrou como vizinha de `alpha`; troquei por nomes sem palavra em comum; faltava `Repo::empty()` nos helpers do CLI (adicionado); o `.query.rs.pending-snap` rastreado foi alterado pelo insta e restaurado antes do commit.
- Goldens antigos: 3 snapshots de `query word` mudaram; conferi cada diff lendo, e só ganharam as 3 linhas da seção nova (`limit` em 4 de 10 arquivos, contexto `billing` 4 de 8, sem vizinhas).
- Impacto: 292 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos. Desempenho: `query word limit` 2,3 s e `total` 1,1 s no repositório sintético de 5.000 arquivos (sem regressão), `brief` 0,35 s; `query word order` na cópia do repositório real 0,21 s.
- No repositório real (cópia `/tmp/eli`): `order` está em 118 de 801 arquivos (0,15); contexto `orders` 70/188, `booking` 32/128, `tests` 9/39, `shared` 2/77, `inventory` 1/78, `account` 1/113; vizinhas por arquivos em comum: `id` (`order|id`=0,33), `test`, `item`, `reservation` (`order|reservation`=0,65, `reservation|order`=0,39), `status` (0,56), `repository`. Bate com as medições antes do código.
- Limite honesto: ordenar as vizinhas por arquivos em comum põe palavras muito frequentes (`id`, `test`, `item`) no topo; a representatividade aparece, mas o ranking continua favorecendo frequência. Usar a representatividade para ordenar é a D8 (TASK seguinte).
- Não verificado: `serve`/MCP com o campo novo (o JSON do `query word` mudou, aditivo); o efeito em repositórios sem contexto (contextos vazios).
- Critérios afetados: 5, 8

### [2026-11-08T23:00-03:00] — Etapa 4: medições, spec e README
- Tipo: alteração
- Contexto: etapa 4 aprovada (`1`); commit `docs: document config hash, skeleton list and word representativeness`.
- Medições (repositório sintético de 5.000 arquivos, mesmo gerador da TASK-1): índice completo 42 s (meta 60 s), reindexação sem mudança 0,04 s (meta 300 ms), `brief "calculate total limit"` 0,36 s (meta 500 ms), `query word total` 1,1 s, `query word limit` 2,3 s. Cópia do repositório real (801 arquivos): índice completo 9 s; `brief order` traz `âncoras: order(1.00)`.
- Spec: §6 (hash de config em `meta.config_hash` e quais chaves entram), §5.6 (representatividade) e a linha de `query word` na tabela de comandos; README: ficha com representatividade, aviso de reindexação por mudança de config, desempenho remedido.
- Critérios do §4 marcados `[x]`: todos os 11, com as evidências nas etapas 1 a 4 (os de medição no repositório real e desempenho são medidas, não testes).
- Limite honesto do critério 9: `brief order` deixou de vir vazio, mas o ranking de símbolos continua ruim (`Entity.get_events` e `PaymentGatewayRegistry.get` antes dos métodos de `OrderEntity`); colocar `id` e `is` na lista `skeleton` não mudou os primeiros resultados. O ranking por representatividade (D8) é a TASK seguinte.
- Não verificado: `brief order` no repositório do humano com o binário novo instalado (só na cópia); `serve`/MCP com o campo novo; repositório sem contexto.
- Impacto: STATE=ENGINEERING concluída; falta o gate para CODE_REVIEW.

### [2026-11-08T23:40-03:00] — code review Aprovado c/ ressalvas
- Revisado 7a21ca5b (`dfc4077..7a21ca5`, 8 commits lidos); 🔴=0, 🟡=2 (A1 artefato do insta rastreado, A2 `word_core` como flag), 🟢=3, ⚖️=1; N_PARTITE gravado (frame manual, sem CALL da skill); GRILL não chamado (sem DRIFT, sem achado dependente de intenção).
- Estado: segue `in-progress`; DONE depende do `1` do humano. A1 e A2 entram antes (ENGINEERING) ou ficam como dívida declarada.

### [2026-11-09T00:05-03:00] — DONE
- Aprovação do humano (`1`) ao veredito "Aprovado c/ ressalvas" de 7a21ca5b; Estado: done. Escolhida a opção de declarar a dívida, não a de corrigir A1 e A2 antes.
- Dívida declarada, sem prazo: A1 (`.query.rs.pending-snap` rastreado e reescrito a cada `cargo test`; recomendado ignorar `*.pending-snap`), A2 (`word_pattern`/`word_card` separados, com teste de paridade), A3 (`init` grava `min_*` inertes), A4 (testes de `write_methods` no hash e de interrupção do rebuild), A5 (`entrypoint_decorators` sem consumidor), A6 (três cálculos de vizinhança; manter).
- Seguinte (fora desta TASK): D8, ranquear o `brief` por representatividade; é o que falta para `brief order` trazer os métodos de `OrderEntity` antes de `Entity.get_events`.
- Em aberto, não verificado: `brief order` no repositório do humano com o binário novo instalado; `serve`/MCP com o campo novo; repositório sem contexto.

### [2026-11-09T00:20-03:00] — A1 resolvida depois do DONE
- Tipo: alteração (dívida declarada)
- Contexto: autorização do humano (`1`) para corrigir só a A1; commit `chore: stop tracking the insta pending snapshot file`.
- Decisão tomada: `git rm --cached` em `crates/memo-cli/tests/.query.rs.pending-snap` e `*.pending-snap` no `.gitignore`; 292 testes passam e o `git status` fica limpo depois do `cargo test`.
- Dívida restante: A2 a A6 (sem prazo). Estado segue `done`.

