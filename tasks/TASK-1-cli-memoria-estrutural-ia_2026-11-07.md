# TASK-1 — Implementar o MVP do CLI `memo` (memória estrutural para IA)
branch: master
workdir: /var/home/erickod/Documentos/dev/memmo
created_at: 2026-11-07

## Status
- Estado: done
- Owner: Erick Duarte
- ROLE requerida: eng

---

## 1. Objetivo
Entregar o CLI `memo` em Rust, que indexa um repositório Python como grafo de palavras, símbolos e chamadas e guarda notas ancoradas, conforme a spec `Spec CLI de memória estrutural para IA.md` (REF: spec §1).

---

## 2. Contexto
- Fonte de verdade: `/var/home/erickod/Documentos/dev/memmo/Spec CLI de memória estrutural para IA.md` (referida abaixo como "spec §N").
- Linguagem/stack: Rust estável, workspace Cargo com `memo-core`, `memo-cli`, `memo-mcp`, `memo-harness`. SQLite (`rusqlite` bundled), `tree-sitter-python`, `clap`, `serde`, `toml`, `blake3`, `rayon`, `thiserror`, `insta` (spec §2, §3).
- Repo `memmo` criado com `git init` na DEFINITION (não era repositório git).

---

## 3. Escopo

### Dentro do escopo
- Marcos 1 a 5 da spec §11.1: extração e armazenamento; chamadas, tipos e efeitos; pesos e `brief`; notas e eventos (derive, promote, drop, status); harness (`init`) e MCP (`serve`).
- Fixture Python em `tests/fixtures/` e testes de aceitação A1 a A14 (spec §11.2, §11.3).
- Qualidade: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` limpos, golden files `insta` (spec §11.4).

### Fora do escopo
- Marco 6 (`extract` para revisão), outras linguagens, hooks externos, runtime, sincronização de notas, harness `agy` (spec §11.1, §12.1, §12.2).
- Validação manual em repositório real (spec §11.5): pós-MVP.
- Chamar APIs de provedores de IA, executar código do repo, versionar o grafo (spec §1).

---

## 4. Critérios de Aceitação
verificáveis (teste | comando | valor) · regras de transição

- [x] A1–A3 e A14 passam (marco 1) — `indexing.rs`, `extraction.rs`, `calls.rs`
- [x] A4–A6 passam (marco 2) — `calls.rs`, `query.rs`, `edges.rs`
- [x] A7–A8 passam (marco 3) — `brief.rs`, `query.rs`
- [x] A9–A12 passam (marco 4) — `note_lifecycle.rs`, `features.rs`, `notes.rs`
- [x] A13 passa (marco 5) — `harness.rs`, `init.rs`
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas (194 testes)
- [x] Toda saída da spec §8 coberta por golden file `insta` — aceito como parcial pelo humano (D17): golden em symbol, word, writers, readers, type, callers, callees, impact, tests, brief, note add, index, promote, status e init --print; `note list/show/confirm/recheck`, `derive` e `drop` são cobertos por asserções, sem golden
- [x] P1: duas reindexações simultâneas da mesma db serializam por lock, sem corromper nem perder eventos
- [x] P2: nota gravada por `.md` temporário + rename; interrupção não deixa `.md` truncado; índice reconstruível a partir dos `.md`
- [x] P3: nota `stale` guarda `estado_anterior`; `note recheck` restaura esse estado
- [x] P4: com `--no-index`, nenhuma nota muda de estado e nenhum evento é gravado
- [x] D13: `file.context` e `file.layer` vêm de `context_root` e `layers` em `memo.toml` (sem glob); spec §2, §4 e §6.1 atualizadas
- [x] D14: o cabeçalho do `brief` pode repetir o pedido; nenhum outro item ecoa a entrada
- [x] D12: sem feature derivada, comandos leem a base indexada por ref; a árvore de trabalho só alimenta a feature
- [x] Id de task fora de `^[A-Za-z0-9][A-Za-z0-9._-]*$` ou com `..`: exit 1, nada muda em disco
- [x] Campos de uma linha das notas rejeitam `\n`/`\r` (exit 4); chave duplicada no frontmatter é erro
- [x] Nota corrompida é pulada e contada em `cobertura.notas_com_erro`; o comando segue (exit 0)
- [x] `serve` insere `--` antes dos posicionais
- [x] Metas da spec §11.4 medidas e registradas (5.000 arquivos ≤ 60 s; sem mudança ≤ 300 ms; `brief` ≤ 500 ms) — ver log de 2026-11-08T12:00-03:00

---

## 5. Restrições e Regras
- técnicas: contexto e camada por `context_root` e `layers` (D13); eco do pedido só no cabeçalho do `brief` (D14); princípios 1–7 da spec §2; dependência fora da lista exige justificativa; `memo-core` não imprime nem depende de `clap`/MCP (spec §3).
- negócio: o CLI entrega fatos; não julga código.
- dependências obrigatórias: tree-sitter-python, rusqlite bundled.
- segurança · performance: não escreve na árvore versionada, salvo `init` sob marcadores; determinismo byte a byte.

---

## 6. Impacto em Dados
O esquema SQLite da spec §4 é o contrato (projeto novo, sem schema legado).

- [x] Nenhuma alteração de schema (esquema novo conforme spec §4)
- [ ] Uso de entidades existentes
- [ ] Leitura apenas
- [x] Escrita controlada

---

## 7. Estratégia de Testes
escritor: test-analyst

**Cenários** — Happy Path: A1, A2, A4–A6, A9–A11, A13 · Error Scenarios: A12, A14, nota > 1200, lock estourado (D6) · Edge Cases: sufixo ambíguo, `--max-chars`, `--no-index`, interrupção na escrita de nota.

**Rastreabilidade**
- A1 → integration `index_again_no_writes` · A2 → unit `split_action_object_role` · A3 → unit `comment_only`/`docstring` · A4 → `protocol_edge_exact` · A5 → `protocol_consumer` · A6 → `writers_listed`
- A7 → `same_brief_twice` · A8 → `output_over_max_chars` · A9 → `renamed_function` · A10 → `changed_signature` · A11 → `derived_feature` · A12 → `skeleton_word`/`missing_symbol` · A13 → `init_twice`/`init_print` · A14 → `syntax_error`
- P1/D6 (30 s, exit 3) → `two_concurrent_indexes`, `lock_held_over_30s` · P2/D7 → `interrupt_during_note_write` · P3/D8 → `estado_anterior_restored` · P4/D9 → `no_index_flag` (2 testes)
- Determinismo (princípio 2) → `two_runs_same_tree` · `promote`/`drop` (spec §6.4) → testes dedicados
- não coberto: metas de desempenho §11.4 — medição, não teste de pass/fail; golden files `insta` cobrem a saída da §8 nos testes acima.

**Lifecycle Coverage**
- [x] Símbolo: nascimento/mutação/morte/renomeação → `renamed_function`, `changed_signature`, `index_again_no_writes`
- [x] Nota: proposta→confirmada→stale→restaurada → `confirm_flag`, `changed_signature`, `estado_anterior_restored`; orfa → coberta em unit de transição (a detalhar na ENGINEERING com a fixture)
- [x] feature.db: derive→promote | drop → `derived_feature`, `task_promoted`, `dropped_task`

### Testes Unitários
- [x] test_given_calculate_limit_usecase_when_split_then_action_object_role  (69 chars)
- [x] test_given_comment_only_change_when_hashed_then_body_hash_unchanged  (67 chars)
- [x] test_given_docstring_change_when_hashed_then_body_hash_unchanged  (64 chars)
- [x] test_given_http_server_name_when_split_then_http_and_server_words  (65 chars)
- [x] test_given_dunder_init_when_split_then_no_words  (47 chars)
- [x] test_given_test_prefixed_name_when_split_then_segments_given_when_then  (70 chars)
- [x] test_given_note_over_1200_chars_when_added_then_rejected  (56 chars)
- [x] test_given_skeleton_word_anchor_when_note_added_then_exit_4  (59 chars)
- [x] test_given_missing_symbol_anchor_when_note_added_then_exit_2  (60 chars)
- [x] test_given_note_with_confirm_flag_when_added_then_state_confirmada  (66 chars)
- [x] test_given_stale_note_when_recheck_then_estado_anterior_restored  (64 chars)
- [x] test_given_ambiguous_suffix_when_queried_then_candidates_and_exit_2  (67 chars)
- [x] test_given_output_over_max_chars_when_rendered_then_last_line_omitidos  (70 chars)
- [x] test_given_same_brief_twice_when_run_then_bytes_identical  (57 chars)
- [x] test_given_unannotated_param_when_extracted_then_recebe_with_null_dst  (69 chars)
- [x] test_given_no_index_flag_when_note_anchor_changed_then_state_unchanged  (70 chars)

### Testes de Integração
- [x] test_given_indexed_repo_when_indexed_again_then_no_writes_no_events  (67 chars)
- [x] test_given_file_with_syntax_error_when_indexed_then_listed_in_arquivos_com_erro  (79 chars)
- [x] test_given_protocol_dep_when_save_called_then_protocol_edge_exact_impl_edge_approx  (82 chars)
- [x] test_given_protocol_consumer_when_callers_then_included_with_cobertura  (70 chars)
- [x] test_given_two_usecases_writing_status_when_writers_then_both_listed  (68 chars)
- [x] test_given_renamed_function_same_body_when_indexed_then_symbol_renamed_note_reanchored  (86 chars)
- [x] test_given_changed_signature_when_indexed_then_note_stale_and_recheck_restores  (78 chars)
- [x] test_given_derived_feature_when_file_edited_then_base_db_byte_identical  (71 chars)
- [x] test_given_init_twice_when_run_then_instruction_block_not_duplicated  (68 chars)
- [x] test_given_init_print_when_run_then_no_file_written  (51 chars)
- [x] test_given_two_concurrent_indexes_when_run_then_serialized_no_lost_events  (73 chars)
- [x] test_given_lock_held_over_30s_when_queried_then_exit_3  (54 chars)
- [x] test_given_interrupt_during_note_write_when_restarted_then_md_intact_and_index_rebuilt  (86 chars)
- [x] test_given_no_index_flag_when_query_run_then_no_note_transition_no_event  (72 chars)
- [x] test_given_task_promoted_when_base_reindexed_then_notes_scope_repo  (66 chars)
- [x] test_given_dropped_task_when_run_then_feature_db_and_task_notes_removed  (71 chars)
- [x] test_given_two_runs_same_tree_when_index_then_graph_bytes_identical  (67 chars)

- [ENGINEERING] test_given_changed_expression_when_hashed_then_body_hash_differs  (73 chars) — sanidade do A3 (aprovado pelo humano na alteração 4)

- [ENGINEERING] 6 testes de CLI em `crates/memo-cli/tests/query.rs` (query symbol/word: sufixo único, ambíguo, desconhecido, JSON, `--max-chars`; golden `insta`) — aprovados na alteração 5

- [ENGINEERING] 12 testes da alteração 6: 9 em `crates/memo-core/tests/edges.rs` (extração de ESPECIALIZA, RETORNA, LEVANTA, ESCREVE, LE, imports e IMPORTA resolvida, reindexação incremental de `writers`) e 3 de CLI (`writers`, `readers`, `type`, golden `insta`) — aprovados na alteração 6

- [ENGINEERING] 16 testes da alteração 7: 11 em `crates/memo-core/tests/calls.rs` (regras 1, 2, 3, 5, 6, 7 e externo; `ESCREVE` externo; `ESPECIALIZA` estrutural; A4) e 5 de CLI (`callers`, `callees`, `impact`, `tests`, `--depth 4` → exit 1; golden `insta`; A5) — aprovados na alteração 7

- [ENGINEERING] 14 testes da alteração 8: 11 em `crates/memo-core/tests/stats.rs` (`memo.toml`, vocabulário e camadas configuráveis, `word_stats`, esqueleto por lista e por limiar, PageRank, determinismo) e 3 de CLI (`query word` com esqueleto, `memo.toml` inválido → exit 1; golden `insta`) — aprovados na alteração 8

- [ENGINEERING] 16 testes da alteração 9: 10 em `crates/memo-core/tests/brief.rs` (tokenização, âncoras exatas, por prefixo e esqueleto, limites de 5 âncoras e 15 símbolos, escritores, determinismo, deltas) e 6 de CLI (`brief` golden, A7, A8, `--task`, `--since-last`, delta de `impact`) — aprovados na alteração 9

- [ENGINEERING] 27 testes da alteração 10: 20 em `crates/memo-core/tests/notes.rs` (roundtrip, validação do §7.2, conflitos, `substitui`, âncoras `edge:`/`file:`, escopo por task, pesos, escrita atômica, `confirm`, alias `glossario`, notas no `brief`, carimbo UTC) e 7 de CLI (`note add/list/show/confirm`, A12 exit 2 e 4, notas em `query symbol` e `brief`, escopo de task) — aprovados na alteração 10

- [ENGINEERING] 28 testes da alteração 11: 10 em `crates/memo-core/tests/events.rs` (lotes `b-N`, criado, assinatura, corpo, só comentário, só linha, renomeação, renomeação com corpo alterado, pareamento ambíguo, palavras, resumo), 12 em `crates/memo-core/tests/note_lifecycle.rs` (reancoragem, `stale` com `estado_anterior`, `recheck`, `stale_on`, `orfa` por símbolo, palavra, aresta, aresta com assinatura alterada, arquivo, nota nunca apagada, eventos `NOTE_*`, A1 com notas, `NOTE_ADDED/CONFIRMED/SUPERSEDED`) e 6 de CLI (`index`, estado `stale`, P4 com `--no-index`, `recheck`, A9) — aprovados na alteração 11

- [ENGINEERING] 20 testes da alteração 12: 8 em `crates/memo-core/tests/features.rs` (lock, ref git, backup, meta, promote, drop) e 12 de CLI em `crates/memo-cli/tests/features.rs` (A11, `--task` sem feature, derive repetido, `index --ref`, `index --full`, promote golden, drop, status golden, branch mergeado, lock exit 3, índices concorrentes) — aprovados na alteração 12

- [ENGINEERING] 23 testes da alteração 13: 14 em `crates/memo-harness/tests/harness.rs` (bloco e marcadores, conteúdo preservado, reinstalação só entre marcadores, `--print`, `.mcp.json` com merge e JSON inválido, codex com `AGENTS.md` e `.codex/config.toml`, conflito, uninstall, doctor, `agy` e desconhecido, texto fixo do bloco, detect), 1 de `Config::to_toml` e 8 de CLI em `crates/memo-cli/tests/init.rs` (A13 duas vezes e `--print`, `memo.toml` nunca sobrescrito, base indexada pela ref, codex com MCP, `agy`/desconhecido exit 1, repo posicional, repo sem commits) — aprovados na alteração 13

- [ENGINEERING] 22 testes da alteração 14: 18 em `crates/memo-mcp/tests/protocol.rs` (initialize, notificação sem resposta, ping, `-32601`, `-32700` com continuidade, `-32600` para lote, `tools/list` com 4 ferramentas, mapeamento de argumentos de `brief`, `query`, `note_add` e `status`, `isError`, ferramenta ou argumento inválido `-32602`, `--repo`, ids, `serve` em duas linhas) e 4 de CLI em `crates/memo-cli/tests/serve.rs` (saída idêntica à do CLI, erro, `note_add` com origem `agente`, handshake e exit 0) — aprovados na alteração 14

- [TEST_ANALYSIS] testes do review (itens 🔴 2 a 4, 🟡 5 e 6; D12):
- [x] test_given_task_id_with_dotdot_when_drop_then_usage_error_and_base_db_kept  (74 chars)
- [x] test_given_task_id_with_dotdot_when_derive_then_usage_error_and_nothing_created  (79 chars)
- [x] test_given_task_id_with_slash_when_note_add_then_usage_error_exit_1  (67 chars)
- [x] test_given_task_with_newline_when_note_added_then_validation_exit_4_and_no_file  (79 chars)
- [x] test_given_alias_with_newline_when_note_added_then_validation_error  (67 chars)
- [x] test_given_duplicate_frontmatter_key_when_parsed_then_note_error  (64 chars)
- [x] test_given_corrupted_note_file_when_queried_then_skipped_and_counted_in_cobertura  (81 chars)
- [x] test_given_dash_prefixed_brief_text_when_mcp_called_then_argv_has_double_dash_separator  (87 chars)
- [x] test_given_dash_prefixed_query_target_when_mcp_called_then_target_stays_positional  (82 chars)
- [x] test_given_ref_base_when_worktree_changed_without_feature_then_query_reads_ref_graph  (84 chars)
- [x] test_given_derived_feature_when_worktree_changed_then_only_feature_sees_change  (78 chars)
- [x] test_given_repo_notes_when_worktree_changed_without_feature_then_notes_not_stale  (80 chars)

**F.I.R.S.T Checklist**
- [x] Fast · Isolated · Repeatable · Self-validating · Timely (fixture local, SQLite temporário, sem rede)
- [x] nenhum nome > 89 chars
- [x] docstrings GIVEN/WHEN/THEN em inglês: presentes em todos os testes (conferido por varredura dos arquivos de teste, 0 faltando)
- [x] todo critério §4 tem ≥1 teste
- [x] nenhum valor concreto inventado (D6 vem da resposta Q1)

---

## 8. Dependências
- tasks predecessoras: nenhuma
- módulos | serviços externos: documentação vigente dos harnesses `claude` e `codex` e do SDK MCP Rust, consultada antes do marco 5 (spec §9.2, §9.4)
- flags | configurações: `memo.toml` (spec §6.1)

---

## 9. Observações
Spec já define o modelo (entidades, arestas, estados de nota) e os critérios; esta TASK a referencia em vez de duplicá-la.

### N_PARTITE (DEFINITION)
[N_PARTITE] modo: concept | alvo: snapshot | referência: — | perfil: stacked + full | budget: 10 | frame: base | fonte: 5d6a9e724f06..5d6a9e724f06 (sha256 da spec)

Elementos (EXTRACT.CORE, resumo): L5 repo Python · L4 grafo (base.db · features/<task>.db) · L4 notas (notes/*.md) · L4 integração (harness · MCP) · L2 Símbolo · Nota · Palavra · Aresta · Evento · Task · L3 reindexação · resolução de chamadas · transições de nota · derive/promote/drop · brief · L1 sig_hash · body_hash · estado da nota · ancora_sig/body.
lifecycle.onto: Símbolo (nasce na indexação · muta sig/body · morre ou renomeia) · Nota (proposta→confirmada→stale→orfa|substituida) · feature.db (derive→promote|drop) · Evento (imutável) — definidos; `não definido`=0.
lifecycle.hist: não definido (snapshot de spec sem histórico) · evidence: não definido.

[N_PARTITE:concept] par: Nota (L2) ↔ Símbolo (L2) — acoplamento: alto | evidência: não definido | abstração: ancora por `qname`+hashes, sem hash de commit | lifecycle: divergente (nota vive além do símbolo; resolvido por orfa/stale) → coerente, sem reorganização
[N_PARTITE:concept] par: feature.db (L4) ↔ base.db (L4) — acoplamento: baixo | evidência: não definido | abstração: cópia completa, sem overlay | lifecycle: divergente (feature morre em drop/promote) → coerente
[N_PARTITE:concept] par: Reindexação (L3) ↔ Nota (L2) — acoplamento: alto | evidência: não definido | abstração: transições de nota calculadas na reindexação | lifecycle: divergente (reindexação é efêmera, nota é permanente) → ver STRUCT 1 e SIDE 1
[N_PARTITE.STRUCT:concept] alvo: Nota.estado `stale` — problema: `note recheck` "volta ao estado anterior ao stale", mas o estado anterior não é campo de nenhuma nota | estrutura proposta: campo `estado_anterior` no frontmatter, gravado na transição para `stale` | base: spec §7.3 (linha `note recheck`) e §7.1
[N_PARTITE.STRUCT:concept] alvo: trait `GraphStore` (L3) e `LanguageExtractor` — problema: 1 implementação cada no MVP; CONTRACT_COST só justifica contrato com ≥2 implementações ou consumidor externo | estrutura proposta: manter (aceito: spec §3 declara troca de motor e linguagem como requisito de fronteira) | base: spec §3 "Traits de fronteira"
[N_PARTITE.CLUSTER] nome: leitura estática de Python e resolução de chamadas — membros: 5 | razão de mudança: sintaxe Python e regras de resolução | fronteira com: grafo (saída: fatos) | contrato: exigido (LanguageExtractor, 2º consumidor de linguagem previsto)
[N_PARTITE.CLUSTER] nome: política de conhecimento humano (notas) — membros: 4 | razão de mudança: ciclo de vida e regras de nota | fronteira com: grafo (entrada: hashes, eventos) | contrato: exigido (estado de nota + eventos)
[N_PARTITE.CLUSTER] nome: adaptação a harness e MCP — membros: 3 | razão de mudança: formatos externos de harness | fronteira com: núcleo (consome funções) | contrato: exigido (HarnessAdapter, 2 implementações)
[N_PARTITE.FAIL] ação: reindexação sob query (L3) (spec §6.3, princípio 5) — eixo: concorrência | status: não coberto (`$COMMON` é compartilhado entre worktrees e todo comando de consulta reindexa; a spec não define lock) → lock exclusivo por db (ex.: `flock` em `<db>.lock`) com espera limitada
[N_PARTITE.FAIL] ação: índice + notas `.md` + eventos (L3) (spec §6.3 passo 5, §7.3) — eixo: dual write | status: não coberto (evento e índice entram numa transação SQLite; reescrita do `.md` da nota fica fora dela) → gravar `.md` por arquivo temporário + rename e tratar as notas como reconstruíveis a partir do `.md` (fonte de verdade, spec §7)
[N_PARTITE.FAIL] ação: indexação (L3) — eixo: interrupção | status: coberto por transação única de escrita (spec §3, regras de implementação) 
[N_PARTITE.FAIL] ação: `init` (L3) — eixo: duplicidade | status: coberto por marcadores `memo:begin/end` e idempotência por passo (spec §9.1, §9.2)
[N_PARTITE.SIDE] ação: consulta com reindexação prévia (L3) — eixo: reação | efeito: consulta pode mudar estado de nota (stale/orfa) e gravar eventos | quando: imediato | status: pergunta → decidir se `--no-index` deve também suspender as transições de nota (padrão sugerido: sim, só reindexação altera notas)
[N_PARTITE.SIDE] ação: `promote` (L3) — eixo: visibilidade | efeito: nota de `task:<id>` passa a `repo` e aparece em consultas da base | quando: imediato | status: informativo (previsto em spec §7.5)
[N_PARTITE.GAP] item: completeness — status: não definido (sem referência) | base: perfil full sem referência → —
invariantes: nenhum declarado em §5; sem código no ALVO; c_código não calculável → `[N_PARTITE.INV?]` não emitido (0 candidatos); 0 invariantes confirmados.
[N_PARTITE.RANK] 1 · FAIL concorrência de reindexação · impacto: alto · alcance: não definido · custo: baixo · 80/20
[N_PARTITE.RANK] 2 · FAIL dual write notas × índice · impacto: alto · alcance: não definido · custo: médio · 80/20
[N_PARTITE.RANK] 3 · STRUCT estado anterior de `stale` · impacto: médio · alcance: não definido · custo: baixo · —
[N_PARTITE.RANK] 4 · SIDE consulta altera notas · impacto: médio · alcance: não definido · custo: baixo · —
[N_PARTITE.STACK] decisão: base | frames: 1 | fonte: 5d6a9e724f06..5d6a9e724f06 | recalculados: 0 | herdados: 0 | resolvidos: 0 | revalidar: 0

#### Pendências geradas pela modelagem (assumidas na aprovação do §D)
- P1: lock exclusivo por db com espera limitada (FAIL 1).
- P2: escrita de nota por `.md` temporário + rename; `.md` é a fonte (FAIL 2).
- P3: campo `estado_anterior` no frontmatter da nota para o `recheck` (STRUCT 1).
- P4: com `--no-index` nenhuma transição de nota é aplicada (SIDE 1).

### Decisões (grill-me)
Rodada única, aceita em bloco pelo humano ("aprovação para tudo, é um mvp") em 2026-11-07. Origem: recomendação aceita.
- D1: escopo do MVP = marcos 1–5; marco 6 fora. (recomendação aceita)
- D2: repo `memmo` recebe `git init`; código Rust na raiz de `memmo`. (recomendação aceita)
- D3: pontos em aberto da spec §12.2 seguem o padrão adotado. (recomendação aceita)
- D4: SDK MCP: verificar `rmcp` antes do marco 5; sem estabilidade, JSON-RPC mínimo à mão (spec §9.4). (recomendação aceita)
- D5: validação manual §11.5 fica pós-MVP. (recomendação aceita)

- D6: P1 lock exclusivo por db com espera limitada. (recomendação aceita, 2026-11-07T17:25-03:00)
- D7: P2 escrita de nota por `.md` temporário + rename. (recomendação aceita)
- D8: P3 campo `estado_anterior` no frontmatter. (recomendação aceita)
- D9: P4 `--no-index` não aplica transições de nota. (recomendação aceita)
- D10: [TEST_ANALYSIS] lock com espera máxima de 30 s e exit 3 ao estourar. (humano)
- D11: [ENGINEERING] `memo serve` por JSON-RPC mínimo à mão sobre stdio, só com `serde_json`; sem `rmcp` (stack assíncrona e dependência fora da §2; estado do SDK não confirmado). (humano, recomendação escolhida)
- D12: [CODE_REVIEW] DRIFT-1 classificado como código errado: a base deve vir da ref principal; a árvore de trabalho só na feature. (humano, recomendação escolhida)
- D13: [CODE_REVIEW] DRIFT-2 classificado como modelo desatualizado: contexto e camada por `context_root` e `layers`, não globs. (humano, recomendação escolhida)
- D14: [CODE_REVIEW] DRIFT-3 classificado como modelo desatualizado: o cabeçalho do `brief` pode ecoar o pedido. (humano, recomendação escolhida)
- D15: [CODE_REVIEW] 🟡 7 (perf da indexação completa, 45,9 s para 5.000 arquivos) aceito como risco: meta de 60 s atendida; reavaliar na validação em repositório real (spec §11.5). (humano, recomendação aceita)
- D16: [CODE_REVIEW] ⚖️ traits `GraphStore` e `LanguageExtractor`: opção B, adiar e concentrar o SQL em `store.rs` depois do MVP; A só com segunda linguagem ou motor. (humano, recomendação aceita)
- D17: [CODE_REVIEW] critério §4 "toda saída da §8 coberta por golden file" aceito como parcial: `note list/show/confirm/recheck`, `derive` e `drop` ficam com asserções; golden como melhoria futura. (humano, recomendação aceita)

### Premissas
- [DEFINITION] A aprovação em bloco dispensa rodadas individuais do grill-me; decisões D1–D5 são as recomendações padrão da spec.
- [DEFINITION] `n-partite@DEFINITION` executada após retorno da TEST_ANALYSIS (ver Log).
- [ENGINEERING] lang sem reference: rust (só `guidelines/references/python.md` existe); vale `guidelines#CRITERIA` base.
- [ENGINEERING] `WORKFLOWS.md` ausente no projeto; comandos de qualidade = spec §11.4 (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`).

### Plano de Execução
#### Etapas
1. Marco 1: workspace Cargo, esquema SQLite, `PythonExtractor`, indexação incremental, palavras/slots, `query symbol`/`word`, fixture, A1–A3, A14 — depende de: NONE
2. Marco 2: resolução de chamadas (regras 1–7), tipos, efeitos, `callers`/`callees`/`impact`/`writers`/`readers`/`type`/`tests`, `cobertura`, A4–A6 — depende de: 1
3. Marco 3: `word_stats`, `brief`, `--max-chars`, deltas por task, A7–A8 — depende de: 2
4. Marco 4: notas, eventos, `derive`/`promote`/`drop`/`status`, A9–A12 — depende de: 3
5. Marco 5: `init`, adaptadores `claude` e `codex`, `serve`, A13 — depende de: 4
#### Gates
- após cada etapa: testes de aceitação do marco passando + fmt/clippy/test limpos
#### Riscos
- Formatos de config dos harnesses mudam → ler a documentação vigente e registrar link e data no PR; formato desconhecido gera exit 1.
- Regra 7 de resolução liga símbolo errado → sempre `~`; medir na fixture.
- Metas de desempenho não atingidas → registrar e ajustar, não bloqueia aceite.
#### Assunções
- `rmcp` pode não estar estável (D4).
- Parse tree-sitter cobre a sintaxe Python da fixture.

### N_PARTITE (CODE_REVIEW)
[N_PARTITE] modo: multi concept×code | alvo: delta | referência: — | perfil: stacked + full | budget: 10 | frame: base | fonte: 01f4792..a2990bf4 (código) + 5d6a9e724f06 (modelo, frame de DEFINITION)
[N_PARTITE:multi] par: id de task (L1) ↔ caminho do grafo de feature (L4) — acoplamento: alto | evidência: não definido | abstração: tipo `TaskId` validado na borda | lifecycle: divergente (o id vive no CLI, o arquivo vive em disco) → validar o id; crates/memo-cli/src/workspace.rs:111
[N_PARTITE:multi] par: id de task (L1) ↔ frontmatter da nota (L1) — acoplamento: alto | evidência: não definido | abstração: serialização que rejeita quebra de linha | lifecycle: divergente → rejeitar `\n` e `\r`; crates/memo-core/src/notes.rs:57, :470
[N_PARTITE:multi] par: ferramenta MCP (L3) ↔ argv do CLI (L3) — acoplamento: alto | evidência: não definido | abstração: separador `--` antes dos posicionais | lifecycle: coerente → inserir `--`; crates/memo-mcp/src/lib.rs:116
[N_PARTITE:multi] par: nota (L2) ↔ símbolo (L2) — acoplamento: alto | evidência: não definido | abstração: âncora por `qname` e hashes | lifecycle: coerente (stale, orfa e reancoragem cobertos por teste) → sem reorganização
[N_PARTITE:multi] par: grafo base (L4) ↔ árvore de trabalho (L5) — acoplamento: alto | evidência: não definido | abstração: base por ref, árvore só na feature | lifecycle: divergente → DRIFT-1; crates/memo-cli/src/workspace.rs, `open_indexed`
[N_PARTITE.STRUCT:multi] alvo: `run` (L3) — problema: orquestrador de ~180 linhas que mistura despacho, índice, notas, delta e renderização | estrutura proposta: um módulo por grupo de comandos, como `lifecycle.rs`, e `run` só despacha | base: crates/memo-cli/src/main.rs:216
[N_PARTITE.STRUCT:multi] alvo: `run_index` (L3) — problema: seleção, parse, transação, resolução, estatísticas e eventos na mesma função de ~120 linhas | estrutura proposta: extrair `select_changes` e `apply_changes` | base: crates/memo-core/src/index.rs:315
[N_PARTITE.STRUCT:multi] alvo: `NoteDir::list` (L3) — problema: uma nota corrompida derruba todo comando | estrutura proposta: pular o arquivo inválido e contá-lo na cobertura | base: crates/memo-core/src/notes.rs:506; spec §2, regra 7
[N_PARTITE.STRUCT:multi] alvo: traits de fronteira (L2) — problema: `GraphStore` e `LanguageExtractor` da spec §3 não existem; o SQL está em `store.conn` fora do store | estrutura proposta: ver ⚖️ em ACTIONS | base: spec §3 "Traits de fronteira"; crates/memo-core/src/query.rs, notes.rs, status.rs
[N_PARTITE.STRUCT:multi] alvo: helpers de teste (L4) — problema: `tests/common/mod.rs` em 3 cópias | estrutura proposta: crate de teste compartilhado | base: crates/memo-core/tests/common/mod.rs, crates/memo-cli/tests/common/mod.rs
[N_PARTITE.DRIFT] par: spec §6.2 e §9.1 (base por ref) ↔ crates/memo-cli/src/workspace.rs, `open_indexed` (base reindexa a árvore de trabalho) — situação: drift → código errado (GRILL Q1, D12)
[N_PARTITE.DRIFT] par: spec §4 e §6.1 (globs de context e layer) ↔ crates/memo-core/src/config.rs (`context_root`, `layers`) — situação: drift → modelo desatualizado (GRILL Q2, D13)
[N_PARTITE.DRIFT] par: spec §2 princípio 3 (nenhum eco da entrada) ↔ crates/memo-cli/src/render.rs, `brief_text` (eco do pedido no cabeçalho, como o exemplo da §8.3) — situação: drift → modelo desatualizado (GRILL Q3, D14)
[N_PARTITE.DRIFT] par: TASK §9/N_PARTITE (DEFINITION), P1 a P4 ↔ código — situação: sem drift (P1 `lock.rs`, P2 `NoteDir::write`, P3 `estado_anterior`, P4 `--no-index`, todos com teste de §7)
[N_PARTITE.FAIL] ação: `drop <task>` (crates/memo-cli/src/lifecycle.rs:100) — eixo: falha | status: não coberto (`drop ../base` apagou `base.db`) → validar o id de task na borda
[N_PARTITE.FAIL] ação: `note add` (crates/memo-core/src/notes.rs, `add`) — eixo: dual write | status: aceito (o `.md` é a fonte de verdade; o evento perdido não afeta estado; registrado no log de 2026-11-08T05:00-03:00)
[N_PARTITE.FAIL] ação: `run_index` (crates/memo-core/src/index.rs:315) — eixo: concorrência | status: coberto por lock exclusivo por banco, espera de 30 s (crates/memo-core/src/lock.rs; teste de dois `index` simultâneos)
[N_PARTITE.FAIL] ação: `note add` repetido — eixo: duplicidade | status: aceito (a mesma entrada vira nota nova e a anterior aparece em `conflitos`, decisão humana, spec §7.2 item 4)
[N_PARTITE.GAP] item: completeness — status: não definido (sem referência) | base: perfil full sem referência → —
[N_PARTITE.CLUSTER] nome: leitura estática de Python e resolução de chamadas — membros: 4 (extract.rs, resolve.rs, words.rs, source.rs) | razão de mudança: sintaxe Python e regras da §5.4 | fronteira com: armazenamento (fatos) | contrato: exigido (`FileFacts`)
[N_PARTITE.CLUSTER] nome: política de conhecimento humano — membros: 4 (notes.rs, events.rs, delta.rs, status.rs) | razão de mudança: ciclo de vida e regras das notas | fronteira com: grafo (hashes e eventos) | contrato: exigido (`Note` e eventos)
[N_PARTITE.CLUSTER] nome: adaptação a harness e protocolo MCP — membros: 4 (memo-harness, memo-mcp, init.rs, serve.rs) | razão de mudança: formatos externos de harness e do MCP | fronteira com: núcleo (comandos do CLI) | contrato: exigido (`HarnessAdapter`, `Backend`)
[N_PARTITE.CLUSTER] nome: orquestração do CLI — membros: 4 (main.rs, workspace.rs, lifecycle.rs, render.rs) | razão de mudança: UX dos comandos | fronteira com: núcleo | contrato: colocalizar (CONTRACT_COST: sem segundo consumidor)
[N_PARTITE.INV?] candidato: `assert ∀ nota: nota.estado = confirmada => confirmada por --confirm, note confirm, recheck ou promote` | estado: Note.estado | c_código: 0.61 | sinais: consistência 4/5 · guarda 0 · teste 1 · restrição 0 | evidência: crates/memo-core/src/notes.rs (add, confirm, recheck, apply_transitions, promote) | contrária: o parser aceita `estado` duplicado vindo de `--task` com quebra de linha (reproduzido)
[N_PARTITE.SIDE] ação: consulta com reindexação (crates/memo-cli/src/workspace.rs, `open_indexed`) — eixo: reação | efeito: toda consulta pode gravar eventos e `session_seen` e reescrever notas | quando: imediato | status: informativo (princípio 5 da spec; testes de P4)
[N_PARTITE.SIDE] ação: `derive` (crates/memo-cli/src/lifecycle.rs:14) — eixo: cascata | efeito: a feature herda os eventos da base | quando: imediato | status: informativo
[N_PARTITE.RANK] 1 · FAIL `drop` com id fora de `features/` · impacto: alto · alcance: não definido · custo: baixo · 80/20
[N_PARTITE.RANK] 2 · INV? e STRUCT de frontmatter (injeção via task) · impacto: alto · alcance: não definido · custo: baixo · 80/20
[N_PARTITE.RANK] 3 · DRIFT-1 base por ref · impacto: alto · alcance: não definido · custo: alto · 80/20
[N_PARTITE.RANK] 4 · DRIFT-2 e DRIFT-3 (TASK desatualizada) · impacto: médio · alcance: não definido · custo: baixo · —
[N_PARTITE.RANK] 5 · STRUCT nota corrompida derruba comando · impacto: médio · alcance: não definido · custo: baixo · —
[N_PARTITE.RANK] 6 · STRUCT argv do MCP sem `--` · impacto: médio · alcance: não definido · custo: baixo · —
[N_PARTITE.STACK] decisão: base | frames: 1 | fonte: 01f4792..a2990bf4 | recalculados: 0 | herdados: 0 | resolvidos: 0 | revalidar: 0

#### Delta re-review 1
[N_PARTITE] modo: multi concept×code | alvo: delta | referência: — | perfil: stacked + full | budget: 10 | frame: delta | fonte: a2990bf4..bfc0c71
[N_PARTITE.STACK] decisão: push delta | frames: 2 | fonte: a2990bf4..bfc0c71 | recalculados: 6 | herdados: 20 | resolvidos: 6 | revalidar: 0
[N_PARTITE.DRIFT] par: spec §6.2 e §9.1 ↔ workspace.rs `open_indexed` — status: resolvido (base por ref; árvore de trabalho só na feature; teste `test_given_ref_base_when_worktree_changed_without_feature_then_query_reads_ref_graph`)
[N_PARTITE.DRIFT] par: spec §4 e §6.1 (globs) ↔ config.rs — status: resolvido (spec e TASK atualizadas, D13)
[N_PARTITE.DRIFT] par: spec §2 princípio 3 ↔ `brief_text` — status: resolvido (spec e TASK atualizadas, D14)
[N_PARTITE.FAIL] ação: `drop <task>` — eixo: falha | status: resolvido (id validado em `memo_core::task::is_valid_id` e no CLI antes de qualquer E/S; `drop ../base` exit 1, `base.db` mantido)
[N_PARTITE.STRUCT:multi] alvo: `NoteDir::list` — status: resolvido (`scan` pula e conta arquivos inválidos)
[N_PARTITE.STRUCT:multi] alvo: frontmatter da nota — status: resolvido (campos de uma linha, chave duplicada rejeitada)
[N_PARTITE.STRUCT:multi] alvo: argv do MCP — status: resolvido (`--` antes dos posicionais)
[N_PARTITE.INV?] candidato: nota `confirmada` só por `--confirm`, `note confirm`, `recheck` ou `promote` — c_código: 0.78 | sinais: consistência 5/5 · guarda 1 (parser e validação) · teste 1 · restrição 0 | contrária: nenhuma após a correção; segue candidato, sem promoção humana
[N_PARTITE.FAIL] ação: `derive`/`index` sem branch principal — eixo: falha | status: aceito (base cai na árvore de trabalho com aviso no stderr; decisão de borda registrada no log de 2026-11-08T16:30-03:00)
[N_PARTITE.STRUCT:multi] alvo: traits de fronteira — status: aceito (D16, opção B)
[N_PARTITE.RANK] 1 · INV? confirmação de nota · impacto: alto · alcance: não definido · custo: baixo · 80/20
[N_PARTITE.RANK] 2 · FAIL base sem branch principal · impacto: baixo · alcance: não definido · custo: baixo · —

### Code Review
#### [2026-11-08T13:30-03:00] — a2990bf4

WT (PR): não se aplica (revisão local dos commits da branch `master`, `01f4792..a2990bf4`, 26 commits; sem remoto GitHub, links com sha8 puro).

**SUMMARY**
- verdict: **Requer alterações**
- positivos:
  - 194 testes passam, `fmt` e `clippy --all-targets -D warnings` limpos; os 33 testes de §7 existem com o nome exato e todo teste tem docstring GIVEN/WHEN/THEN em inglês.
  - fronteiras limpas: `memo-core` não depende de `clap` nem de MCP; protocolo MCP isolado atrás de `Backend`; adaptadores atrás de `HarnessAdapter`; todo SQL parametrizado.
  - os quatro pontos da n-partite de DEFINITION (P1 a P4) têm enforcement e teste: lock (`lock.rs`), escrita atômica de nota (`notes.rs`, `NoteDir::write`), `estado_anterior`, `--no-index` sem transições.
- preocupações críticas (🔴):
  1. `TASK desatualizada`: DRIFT-2 e DRIFT-3 classificados pelo humano como modelo desatualizado (GRILL) → retorno à DEFINITION.
  2. DRIFT-1 (código errado, decisão do humano): a base reindexa a árvore de trabalho em vez de vir da ref principal.
  3. O id de task entra em caminhos sem validação: `drop ../base` apagou `base.db` (reproduzido).
  4. `--task` com quebra de linha injeta chave no frontmatter da nota: uma nota `proposta` virou `confirmada` sem `--confirm` (reproduzido).

**ANALYSIS**

CRITERIA
- [ ] design: achado: `run` concentra despacho, índice, notas, delta e renderização em ~180 linhas (crates/memo-cli/src/main.rs:216); as demais fronteiras (`Backend`, `HarnessAdapter`, `SourceTree`) estão boas.
- [ ] DDD: não identificado — verificado em: flag `ddd?` ausente no GATE.IN e no Plano de Execução da TASK.
- [ ] reliability: achado: `drop` com id fora de `features/` destrói arquivos (crates/memo-cli/src/lifecycle.rs:100, crates/memo-cli/src/workspace.rs:111); dual write nota `.md` × SQLite aceito e registrado (crates/memo-core/src/notes.rs, `NoteDir::add`); idempotência de `init` e de `index` coberta por teste.
- [ ] data: não identificado — verificado em: diff sem campo monetário, PII, nem trilha de auditoria de dado financeiro.
- [ ] contract: achado: argumentos posicionais do `serve` viram flags do CLI (crates/memo-mcp/src/lib.rs:116); sem consumidor externo de assinatura pública alterada (branch inicial); envelope JSON com `schema: 1`.
- [ ] env_k8s: não identificado — verificado em: diff sem `settings.<VAR>` nem manifest; só lê `MEMO_NOW`, `MEMO_LOCK_TIMEOUT_MS` e `MEMO_TASK`.
- [ ] concurrency: não identificado — verificado em: diff sem worker, consumer ou manifest com `terminationGracePeriodSeconds`; concorrência entre processos coberta por lock exclusivo por banco e pelo teste `test_given_two_concurrent_indexes_when_run_then_serialized_no_lost_events`.
- [ ] migrations: não identificado — verificado em: diff sem migrations de banco (esquema novo `schema_version=1`, sem caminho de migração: observação 🟢).
- [ ] perf: achado: a resolução faz uma consulta SQL por referência (crates/memo-core/src/resolve.rs:402, :553); indexação completa de 5.000 arquivos em 45,9 s (meta 60 s); `brief` busca endereço símbolo a símbolo (`addr_by_id`); tabela `event` sem poda.
- [ ] security: achado: id de task sem validação (path traversal e injeção de frontmatter, crates/memo-cli/src/workspace.rs:111, crates/memo-core/src/notes.rs:57 e :470); argv do MCP sem `--` (crates/memo-mcp/src/lib.rs:116); SQL injection não identificado — verificado em: consultas com parâmetros `?n`, `format!` só com constantes.
- [ ] errors: achado: uma nota corrompida faz todo comando sair com exit 3 (crates/memo-core/src/notes.rs:506), contra a regra 7 da spec (falha parcial não é falha).
- [ ] readability: achado: funções longas e com ramificação densa: `run_index` (crates/memo-core/src/index.rs:315, ~120 linhas), `insert_symbols`, `recompute` (stats.rs, ~100), `brief_with` (crates/memo-core/src/brief.rs:297, ~94), `scan_effects` (extract.rs, ~91).
- [ ] layers: não identificado — verificado em: `crates/memo-core/Cargo.toml` (sem `clap`, sem MCP) e direção CLI → núcleo.
- [ ] tests: achado: sem teste de id de task malicioso nem de frontmatter com quebra de linha; helper `Repo` copiado em 3 `tests/common/mod.rs`; sem over-mocking (só o `Backend` falso do protocolo MCP); 33/33 nomes de §7 presentes.
- [ ] task: achado: critérios §4 com teste; decisões D1 a D11 com enforcement ou teste; invariantes §5 inexistentes; DRIFT-1, DRIFT-2 e DRIFT-3 (ver N_PARTITE).
- [ ] format: não identificado — verificado em: `cargo fmt --check` e `cargo clippy --all-targets -- -D warnings` sem avisos.
- [ ] lang: `lang sem reference: rust` (guidelines/references só tem `python.md`); aplicado só o CRITERIA base.
- [ ] implicit: achado: o parser de nota deixa "o último `estado` vence" implícito (crates/memo-core/src/notes.rs, `Note::parse`); `MEMO_NOW` e `MEMO_LOCK_TIMEOUT_MS` alteram comportamento por ambiente.
- [ ] refactor_estrutural: não identificado — verificado em: flag `refactor_only?` ausente; o refactor de `main` para `execute` manteve os golden files.

Smells¹
- [ ] Long Method: achado: `run`, `run_index`, `insert_symbols`, `recompute`, `brief_with`, `scan_effects` (acima).
- [ ] Long Class: achado: `crates/memo-core/src/notes.rs` reúne modelo, validação, E/S, transições, promoção e resumo.
- [ ] Long Params: não identificado — verificado em: `NewEdge`, `NewRawRef`, `Transitions` e `InitArgs` agrupam parâmetros.
- [ ] Data Clumps: não identificado — verificado em: as mesmas structs; sem grupos repetidos de 3+ parâmetros.
- [ ] Primitive Obsession: achado: `estado`, `tipo`, `origem`, `escopo` e `kind` como `String` (crates/memo-core/src/notes.rs, `Note`), com validação espalhada.
- [ ] Switch Stmts: achado: `match` por prefixo de âncora repetido em `resolve_anchor`, `judge`, `anchor_present`, `notes_for` e `rename_anchor` (notes.rs).
- [ ] Refused Bequest: não identificado — verificado em: sem herança (Rust).
- [ ] Divergent Change: achado: `notes.rs` muda por regra de nota, formato de arquivo, transições e promoção.
- [ ] Shotgun Surgery: achado: um novo tipo de âncora exige alterar as cinco funções acima.
- [ ] Speculative Generality: não identificado — verificado em: `Backend` tem duas implementações (CLI e fake de teste); nenhum parâmetro sem uso.
- [ ] Duplicate/Dead Code: achado: `tests/common/mod.rs` em 3 cópias; `HarnessAdapter::detect` só é usado em teste.
- [ ] Temp Field: achado leve: `Brief.unchanged` e `Impact.unchanged` só são preenchidos com delta.
- [ ] Feature Envy: não identificado — verificado em: `render.rs` só formata campos de structs do núcleo.
- [ ] Inappropriate Intimacy: achado: módulos do núcleo (`query`, `notes`, `status`, `delta`, `brief`) fazem SQL direto em `store.conn`, sem a fronteira `GraphStore` da spec §3.
- [ ] Msg Chains: não identificado — verificado em: sem cadeias longas de chamadas além de `Option`.
- [ ] Middle Man: não identificado — verificado em: `Server::handle` e `Backend` acrescentam protocolo e conversão de argumentos.
- [ ] Magic Numbers: achado leve: `sha[..15]` (crates/memo-core/src/source.rs:152) e os bônus de slot (brief.rs, `slot_bonus`); demais valores são constantes nomeadas.
- [ ] Ignored Exceptions: achado: `let _ = std::fs::remove_file(...)` (crates/memo-cli/src/lifecycle.rs:108) e `.ok()` na leitura do branch atual (workspace.rs, `feature_key`).
- [ ] Opaque Indirection: não identificado — verificado em: sem camadas que só repassam chamadas.
- [ ] Order Coupling: achado: `apply_transitions` depende de `run_index` ter rodado antes e do `Delta` (crates/memo-core/src/index.rs); lock → abrir → indexar é ordem obrigatória em `open_indexed`.

N_PARTITE (CODE_REVIEW), gravado em TASK §9/N_PARTITE (CODE_REVIEW); 1ª linha:
`[N_PARTITE] modo: multi concept×code | alvo: delta | referência: — | perfil: stacked + full | budget: 10 | frame: base | fonte: 01f4792..a2990bf4 (código) + 5d6a9e724f06 (modelo, frame de DEFINITION)`
REUSE: modelo = hit (frame de DEFINITION consumido como base); código = miss (primeiro frame). Cobertura da busca de consumidores: não aplicável (branch inicial, sem símbolo preexistente alterado). DRIFT: 3 itens classificados via GRILL (DRIFT-1 código errado; DRIFT-2 e DRIFT-3 modelo desatualizado).

**ACTIONS**

🔴 bloqueia merge
1. `TASK desatualizada` (DRIFT-2, DRIFT-3) → STATE:BLOCKED, retorno à DEFINITION (REF: agentsmd#TRIAGEM).
   - problema: spec §4 e §6.1 pedem globs para `context` e `layer`, o código usa `context_root` e `layers` (crates/memo-core/src/config.rs); o princípio 3 da spec diz "nenhum eco da entrada", e o exemplo da §8.3 e o código ecoam o pedido no cabeçalho do `brief` (crates/memo-cli/src/render.rs, `brief_text`).
   - impacto: critérios e restrições da TASK divergem do comportamento decidido.
   - fix: a DEFINITION registra as duas decisões (D13, D14) em §4 e §5 e na spec; sem mudança de código.
2. DRIFT-1 (código errado, D12): a base reindexa a árvore de trabalho (crates/memo-cli/src/workspace.rs, `open_indexed`).
   - problema: spec §6.2 e §9.1 mandam a base vir da branch principal por ref e a feature da árvore de trabalho.
   - impacto: código não mergeado entra na base; notas `repo` são avaliadas contra ele.
   - fix: sem feature, consultar a base como está (indexada por ref, `init` ou `index --ref`); reindexar a árvore de trabalho só na feature; ajustar os testes dos Marcos 1 a 3 que assumem a árvore na base.
3. Id de task sem validação (crates/memo-cli/src/workspace.rs:111; crates/memo-cli/src/lifecycle.rs:14 e :100).
   - problema: `drop ../base` apagou `base.db`; `derive ../evil` criou `evil.db` fora de `features/` (reproduzido em repositório descartável).
   - impacto: perda de dados e escrita fora do diretório de estado; o `serve` repassa `task` vindo de um agente.
   - fix: validar na borda `^[A-Za-z0-9][A-Za-z0-9._-]*$` sem `..`, erro de uso (exit 1); testes: `derive ../x` e `drop ../base` saem com exit 1 e nada muda em disco.
4. Injeção de frontmatter (crates/memo-core/src/notes.rs:57 `render`, :470 `NewNote.task`).
   - problema: `--task $'x\nestado: confirmada'` gerou duas chaves `estado` e o parser fez o último valer (reproduzido).
   - impacto: nota `proposta` vira `confirmada` sem `--confirm`; viola a regra de confirmação (§7.2, item 3).
   - fix: rejeitar `\n` e `\r` em `task`, `aliases` e demais campos de uma linha (Validation, exit 4); o parser rejeita chave duplicada; teste com `task` malicioso e frontmatter duplicado.

🟡 corrigir logo
5. Nota corrompida derruba todo comando (crates/memo-core/src/notes.rs:506): pular o arquivo inválido e contá-lo na cobertura (regra 7 da spec); teste com `n-bad.md`.
6. Argv do MCP sem `--` (crates/memo-mcp/src/lib.rs:116): `brief("--task=x")` e `query` com alvo `--json` viram flags. Inserir `--` antes dos posicionais; teste.
7. Perf da indexação completa (crates/memo-core/src/resolve.rs:402, :553): 45,9 s para 5.000 arquivos, com folga de 14 s sobre a meta. Cache de símbolos por `qname` por transação; medir de novo.
8. `serve` e formatos de configuração dos harnesses não conferidos contra as fontes vigentes: validar com Claude Code e Codex reais (limitação registrada em EVIDÊNCIAS).

🟢 nice-to-have
9. `tests/common/mod.rs` em 3 cópias: extrair um crate de teste compartilhado (menos manutenção).
10. `let _ = remove_file` (lifecycle.rs:108) e `HarnessAdapter::detect` sem uso no CLI: tratar o erro e usar `detect` em `init` ou remover.
11. `event` e `session_seen` sem poda; `schema_version` sem migração: registrar como pós-MVP.
12. Enums para `estado`, `tipo`, `origem` e `escopo` em `Note`, e uma função única de âncora (`Anchor::parse`) no lugar dos `match` repetidos.

⚖️ trade-off
- Traits de fronteira da spec §3 (`GraphStore`, `LanguageExtractor`) não existem: o núcleo usa `SqliteStore` e `extract_python_file` direto, e o SQL está espalhado por `store.conn`.
  - A: introduzir as traits agora. Prós: cumpre a spec §3, isola o SQL, abre a segunda linguagem. Contras: custo alto, sem segundo consumidor, risco de regressão nos 194 testes.
  - B: registrar na spec como adiado e concentrar o SQL em `store.rs`. Prós: barato, mantém o MVP. Contras: a troca de motor deixa de ser possível sem refatorar.
  - recomendação condicionada: B no MVP, e A só quando houver segunda linguagem ou motor; a decisão é da eng e do humano.

❓ perguntas ao autor
- nenhuma pendente: as três perguntas do GRILL foram respondidas pelo humano (D12 a D14).

**COMMITS** (sem remoto GitHub: sha8 puro)
- ✓ 01f4792 chore: add cargo workspace with four crates
- ✓ 78ae854 test: add python fixture and extraction unit tests
- ✓ 0a3d1c6 feat: extract python symbols and split names into words
- ✓ e6c712d test: add indexing integration tests and body hash sensitivity test
- ✓ cc24906 feat: add sqlite store and incremental indexer
- ✓ b63a9d7 test: add query symbol and query word cli tests
- ⚠ 40c764e feat: add memo cli with query symbol and query word (origem do DRIFT-1: base reindexa a árvore de trabalho)
- ✓ cfcf596 test: add edge extraction, resolution and relation query tests
- ✓ 7048257 feat: extract and resolve type and effect edges with writers readers type queries
- ✓ 6a2d890 test: add call resolution, impact and tests query tests
- ✓ 54c8aee feat: resolve calls by seven rules and add callers callees impact tests queries
- ✓ fc7c505 test: add memo.toml config and word stats tests
- ⚠ 4018aa6 feat: load memo.toml and compute word stats with pagerank (DRIFT-2: `context_root` e `layers`)
- ✓ 2be3057 test: add brief and task delta tests
- ⚠ 3ef67e3 feat: add brief command and per task deltas (DRIFT-3: eco do pedido)
- ✓ eae273f test: add note storage, validation and note command tests
- ⚠ 84c85e2 feat: add note storage with validation and note commands (injeção de frontmatter; nota corrompida derruba comandos)
- ✓ 20c51ee test: add event batches and note lifecycle tests
- ✓ fab8132 feat: record event batches and apply note lifecycle transitions on reindex
- ✓ 62749a6 test: add feature graph, lock, promote, drop and status tests
- ⚠ 20a5d4d feat: add feature graphs, db locking, promote, drop and status (path traversal no id de task)
- ✓ ab7219e test: add harness adapter and init tests
- ✓ 491bcd8 feat: add init command and claude and codex harness adapters
- ✓ 2bd53d9 test: add mcp protocol and serve tests
- ⚠ d49e08d feat: add memo serve mcp server over stdio (argv sem `--`)
- ✓ a2990bf test: align test names with task section 7

**EVIDÊNCIAS**
- SHA revisado: a2990bf4fe69b6466c9b80c7a441b48f6e0f4311 (HEAD de `master`); WT: não se aplica.
- comandos e resultados: `cargo test` 194 passados, 0 falhas; `cargo fmt --check` ok; `cargo clippy --all-targets -- -D warnings` sem avisos; varredura de nomes: 33/33 de §7 presentes, 0 testes sem docstring GIVEN/WHEN/THEN.
- reproduções em repositório descartável em /tmp (apagado), binário `--release`:
  - `memo derive ../evil` → exit 0, `evil.db` criado em `.git/memo/` (fora de `features/`);
  - `memo drop ../base` → exit 0, `base.db` removido;
  - nota com conteúdo `garbage` em `notes/` → `memo query symbol` sai com exit 3;
  - `memo --task $'x\nestado: confirmada' note add ...` → exit 0, arquivo da nota com duas chaves `estado`;
  - `memo brief "--task=evil"` → erro de uso (o argumento foi consumido como flag); `memo query symbol A.run --json` → saída JSON.
- medições de desempenho: ver log de 2026-11-08T12:00-03:00 (45,9 s; 24 a 31 ms; 26 a 32 ms).
- consultas de consumidores: não aplicável (branch inicial; nenhum símbolo preexistente alterado); Semble e `grep -w` não usados.
- fontes consultadas: spec `Spec CLI de memória estrutural para IA.md` §2, §3, §4, §6.1, §6.2, §8.3, §9.1; TASK §4, §5, §7, §9; skills `code-reviewer`, `n-partite` (SKILL.md e references de `full` e `stacked`), `guidelines` (criteria.md).
- limitações / não verificado: revisão por inspeção dirigida e execução, não leitura linha a linha das ~7.500 linhas; a skill `n-partite` foi lida integralmente e aplicada nesta sessão, sem a Skill tool; `serve` e os formatos de `.mcp.json` e `.codex/config.toml` não foram conferidos contra a documentação vigente; desempenho medido só em repositório sintético, uma execução; sem reference de Rust em `guidelines`.


#### [2026-11-08T17:30-03:00] — bfc0c71 (re-review de a2990bf4..bfc0c71)

WT (PR): não se aplica (revisão local, branch `master`, sem remoto GitHub; links com sha8 puro). Commits novos: 3605ec3, 718780a, ddd60ce, bfc0c71. Pilha da n-partite reaproveitada: frame delta.

**SUMMARY**
- verdict: **Aprovado c/ ressalvas**
- positivos:
  - os 4 🔴 e o 🟡 5 e 6 da entrada anterior estão corrigidos e verificados por execução; 206 testes passam duas vezes seguidas, `fmt` e `clippy --all-targets -D warnings` limpos;
  - os 33 nomes de §7 mantidos e os 12 testes novos de §7 presentes com docstring GIVEN/WHEN/THEN;
  - TASK, spec e código coerentes (D12 a D16); DRIFT=0.
- preocupações: nenhum 🔴. Ressalvas: 🟡 8 (clientes reais não testados) e 🟡 7 (risco aceito, D15).

**ANALYSIS** (commits novos; itens não tocados herdam o resultado anterior)

CRITERIA
- [ ] design: achado herdado: `run` longo (crates/memo-cli/src/main.rs); ganhou `check_task_ids` como função própria.
- [ ] DDD: não identificado — verificado em: flag `ddd?` ausente.
- [ ] reliability: ✓ resolvido: `drop` e `derive` validam o id (main.rs `check_task_ids`; crates/memo-core/src/task.rs); base sem branch principal cai na árvore de trabalho com aviso (workspace.rs `open_indexed`).
- [ ] data: não identificado — verificado em: diff dos 4 commits sem campo monetário nem PII.
- [ ] contract: ✓ resolvido: argv do MCP com `--` (crates/memo-mcp/src/lib.rs); `cobertura.notas_com_erro` novo campo aditivo do JSON `schema: 1`.
- [ ] env_k8s: não identificado — verificado em: diff sem `settings.<VAR>` nem manifest.
- [ ] concurrency: não identificado — verificado em: lock por banco inalterado; o `eprintln!` de aviso não toca estado compartilhado.
- [ ] migrations: não identificado — verificado em: diff sem migrations.
- [ ] perf: ⚠ aceito (D15): indexação completa 45,9 s para 5.000 arquivos; a base por ref usa `git ls-tree` e `cat-file --batch` incremental por blob, sem medição nova (limitação em EVIDÊNCIAS).
- [ ] security: ✓ resolvido: path traversal (id validado), injeção de frontmatter (`campo_multilinha`, chave duplicada), flag injection no MCP; reproduzidos de novo, todos recusados. Não identificado — verificado em: SQL só com parâmetros.
- [ ] errors: ✓ resolvido: nota corrompida pulada e contada (crates/memo-core/src/notes.rs `scan`); `.ok()` em `feature_key` agora é intencional e comentado.
- [ ] readability: achado herdado: `run_index`, `insert_symbols`, `recompute`, `brief_with`, `scan_effects` longos (🟢).
- [ ] layers: não identificado — verificado em: `memo-core` sem `clap` nem MCP; `task.rs` sem dependências.
- [ ] tests: ✓ resolvido: testes de id malicioso, frontmatter, argv, base por ref, feature e nota corrompida; helper de commit sem GPG e com datas fixas (determinismo).
- [ ] task: ✓ resolvido: §4 e §5 com D13 e D14; D12 com teste; D1 a D16 com enforcement ou registro.
- [ ] format: não identificado — verificado em: `cargo fmt --check` e `clippy` sem avisos.
- [ ] lang: `lang sem reference: rust`; só o CRITERIA base.
- [ ] implicit: ✓ resolvido: "último `estado` vence" eliminado pela rejeição de chave duplicada.
- [ ] refactor_estrutural: não identificado — verificado em: flag `refactor_only?` ausente.

Smells¹
- [ ] Long Method: achado herdado (🟢).
- [ ] Long Class: achado herdado: `notes.rs` (🟢).
- [ ] Long Params: não identificado — verificado em: structs de parâmetros mantidas.
- [ ] Data Clumps: não identificado — verificado em: diff dos 4 commits.
- [ ] Primitive Obsession: achado herdado: `estado`/`tipo`/`origem` como `String` (🟢).
- [ ] Switch Stmts: achado herdado (🟢).
- [ ] Refused Bequest: não identificado — verificado em: sem herança.
- [ ] Divergent Change: achado herdado (🟢).
- [ ] Shotgun Surgery: achado herdado (🟢).
- [ ] Speculative Generality: não identificado — verificado em: `scan` é usado por `list` e pelo CLI.
- [ ] Duplicate/Dead Code: achado herdado: `tests/common/mod.rs` em 3 cópias, agora com mais um helper copiado nas 2 versões (🟢).
- [ ] Temp Field: achado herdado leve.
- [ ] Feature Envy: não identificado — verificado em: render.rs.
- [ ] Inappropriate Intimacy: aceito (D16, B).
- [ ] Msg Chains: não identificado — verificado em: diff dos 4 commits.
- [ ] Middle Man: não identificado — verificado em: diff dos 4 commits.
- [ ] Magic Numbers: achado: limite de 64 caracteres do id em task.rs sem constante nomeada (🟢).
- [ ] Ignored Exceptions: achado herdado: `let _ = remove_file` em lifecycle.rs (🟢).
- [ ] Opaque Indirection: não identificado — verificado em: diff dos 4 commits.
- [ ] Order Coupling: achado herdado.

N_PARTITE: ver §9/N_PARTITE (CODE_REVIEW), "Delta re-review 1"; 1ª linha `[N_PARTITE] modo: multi concept×code | alvo: delta | ... | frame: delta | fonte: a2990bf4..bfc0c71`. REUSE: modelo hit, código push delta. DRIFT=0. Consumidores: não aplicável (símbolos alterados sem consumidor externo; os usos internos de `NoteDir::list` e `feature_key` foram lidos).

**VERIFY** (releitura do diff dos 4 commits ignorando a ANALYSIS): `task` aceita até 64 caracteres sem unicode; `note show` e `get` já validavam o id da nota; `promote` valida o id e `promote` por ref continua exigindo branch principal; `NoteDir::scan` ignora `.tmp`. Omissão nova: nenhuma.

**ACTIONS**

🔴 nenhum.

🟡 corrigir logo
8. (anterior, ⚠ pendente) `serve` e os formatos de `.mcp.json` e `.codex/config.toml` não foram conferidos contra Claude Code e Codex reais; o MCP foi escrito pelo que conheço do protocolo. Ação do humano ou de um cliente no ambiente antes de depender do `serve`.
7. (anterior, ⚠ aceito, D15) perf da indexação completa: reavaliar no repositório real.

🟢 nice-to-have (herdados, ⚠ pendentes): 9 `tests/common` em 3 cópias; 10 `let _ = remove_file` e `detect` sem uso no CLI; 11 poda de `event`/`session_seen` e migração de `schema_version`; 12 enums para `estado`, `tipo`, `origem`, `escopo` e `Anchor::parse`; novo: constante nomeada para o limite do id.

⚖️ trade-off: ✓ resolvido (D16, opção B).

❓ nenhuma.

**Itens da entrada anterior:** 🔴 1 `TASK desatualizada` ✓ resolvido (DEFINITION, D13 e D14); 🔴 2 DRIFT-1 ✓ resolvido (bfc0c71); 🔴 3 id de task ✓ resolvido (718780a); 🔴 4 injeção de frontmatter ✓ resolvido (718780a); 🟡 5 nota corrompida ✓ resolvido (bfc0c71); 🟡 6 argv MCP ✓ resolvido (718780a).

**COMMITS**
- ✓ 3605ec3 test: add task id, frontmatter and mcp argv hardening tests
- ✓ 718780a fix: validate task ids, reject multiline note fields and guard mcp argv
- ✓ ddd60ce test: add ref base, feature isolation and corrupted note tests
- ✓ bfc0c71 fix: index the base from the main ref and skip corrupted notes
- commits anteriores: ⚠ 40c764e, 4018aa6, 3ef67e3, 84c85e2, 20a5d4d, d49e08d agora ✓ (achados corrigidos nos commits acima); demais ✓.

**EVIDÊNCIAS**
- SHA revisado: bfc0c71 (HEAD de `master`); WT: não se aplica.
- `cargo test`: 206 passados, 0 falhas, em duas execuções seguidas; `cargo fmt --check` ok; `cargo clippy --all-targets -- -D warnings` sem avisos; `.snap.new`: 0.
- reproduções com o binário `--release` em repositório descartável (apagado): `derive ../evil` e `drop ../base` → exit 1, `base.db` mantido; `--task` com quebra de linha → exit 1, sem arquivo de nota; nota `garbage` → exit 0, `cobertura` com "1 notas com erro"; edição na árvore de trabalho sem feature → `query symbol only_here` exit 2; `serve` com `brief` de texto `--task=evil run` → resposta normal, sem erro de flag.
- consultas de consumidores: não aplicável (sem símbolo preexistente com consumidor fora do próprio repositório); leitura direta dos usos de `NoteDir::list`, `feature_key` e `open_indexed`.
- fontes: TASK §4, §5, §7, §9; spec §2, §4, §6; skills `code-reviewer` e `n-partite` (frame anterior reaproveitado).
- limitações / não verificado: `serve` e formatos de configuração dos harnesses contra clientes reais; desempenho não remedido após a mudança da base para ref; revisão do diff dos 4 commits por leitura dirigida e execução, não linha a linha; sem reference de Rust em `guidelines`.

## 10. Log de Decisões e Alterações
append-only · entrada ✗ alterada | removida (REF: agentsmd#LOG)

### [2026-11-07T16:59-03:00] — Criação da TASK e bloqueios resolvidos
- Tipo: decisão
- Contexto: `memmo` não era repo git; skills fora do diretório de trabalho; `issue_key=-`.
- Decisão tomada: `git init`; leitura das skills por `cat` com autorização humana; `/tasks` em `.git/info/exclude`.
- Impacto: DEST criado como TASK-1.
- Critérios afetados: nenhum

### [2026-11-07T16:59-03:00] — n-partite não executada
- Tipo: decisão
- Contexto: A.3 exige CALL automática com ≥2 conceitos; o modelo já está formalizado na spec §4 e o humano pediu aprovação em bloco para o MVP.
- Decisão tomada: `n-partite · não executada · modelo = spec §4, aprovação em bloco · DEFINITION`; `grill-me · diferida · — · DEFINITION · 1 rodada em bloco · 5 decisões · 0 pendências · STATUS:READY`.
- Impacto: sem frame N_PARTITE (DEFINITION); `code-reviewer` não terá estado empilhado de DEFINITION.
- Critérios afetados: nenhum

### [2026-11-07T17:05-03:00] — Aprovação humana da DEFINITION
- Tipo: validação
- Contexto: §D apresentado; humano respondeu `1`.
- Decisão tomada: DEFINITION aprovada; transição para TEST_ANALYSIS.
- Impacto: ROLE test-analyst assumiu a TASK.
- Critérios afetados: nenhum

### [2026-11-07T17:06-03:00] — TEST_ANALYSIS bloqueada, retorno a DEFINITION
- Tipo: bloqueio
- Contexto: test-analyst#Derivação por lifecycle: TASK com ≥2 conceitos e sem `### N_PARTITE (DEFINITION)` em §9 → STATE:BLOCKED. A n-partite foi dispensada na DEFINITION (log 16:59).
- Decisão tomada: retornar a DEFINITION para executar `n-partite@DEFINITION` (snapshot, full, stacked).
- Impacto: TEST_ANALYSIS só segue após o frame N_PARTITE e nova aprovação.
- Critérios afetados: nenhum

### [2026-11-07T17:20-03:00] — n-partite executada na DEFINITION
- Tipo: validação
- Contexto: retorno da TEST_ANALYSIS; `n-partite · snapshot · full · DEFINITION` (stacked, frame base).
- Decisão tomada: frame registrado em §9; 4 pendências (P1–P4) aguardam aprovação no §D; STATE segue DEFINITION (draft).
- Impacto: desbloqueia TEST_ANALYSIS após aprovação.
- Critérios afetados: nenhum até a aprovação (P1–P4 viram critérios em A.5)

### [2026-11-07T17:25-03:00] — Aprovação única do §D (2ª)
- Tipo: decisão
- Contexto: humano respondeu `1` às pendências P1–P4 da n-partite.
- Decisão tomada: P1–P4 → D6–D9 e critérios em §4; DEFINITION aprovada; STATE → TEST_ANALYSIS.
- Impacto: spec ganha 4 requisitos (lock, escrita atômica de nota, `estado_anterior`, `--no-index` sem transições).
- Critérios afetados: §4 (4 novos)

### [2026-11-07T17:40-03:00] — TEST_ANALYSIS concluída
- Tipo: validação
- Contexto: cenários derivados de A1–A14 e P1–P4; lacuna de valor do lock fechada via grill-me.
- Decisão tomada: `grill-me · diferida · — · TEST_ANALYSIS · 1 rodada · 1 decisão · STATUS:READY`; D10 = 30 s, exit 3 (refina D6); testes definidos em §7; STATE → ENGINEERING.
- Impacto: §7 preenchido; nenhum critério alterado.
- Critérios afetados: P1 (valor concreto)

### [2026-11-07T17:55-03:00] — ENGINEERING bloqueada: toolchain Rust ausente
- Tipo: bloqueio
- Contexto: GATE.IN da eng lido (eng, DOD, guidelines, commiter); `cargo` e `rustc` não estão no PATH, então testes e DOD não são verificáveis.
- Decisão tomada: STATE:BLOCKED até haver toolchain; nenhuma alteração de código feita.
- Impacto: Marco 1 não iniciado.
- Critérios afetados: nenhum

### [2026-11-07T18:10-03:00] — Toolchain Rust disponível
- Tipo: validação
- Contexto: humano instalou a toolchain; `cargo 1.99.1`, `rustc 1.99.1`, `clippy` e `rustfmt` respondem com `~/.cargo/bin` no PATH.
- Decisão tomada: ENGINEERING retomada (blocked → in-progress).
- Impacto: DO.1 do Marco 1 liberado.
- Critérios afetados: nenhum

### [2026-11-07T18:20-03:00] — Esqueleto do workspace Cargo
- Tipo: alteração
- Contexto: alteração atômica 1 do Marco 1 aprovada (`1`).
- Decisão tomada: workspace com `memo-core`, `memo-cli` (bin `memo`), `memo-mcp`, `memo-harness`, `rust-toolchain.toml` e `.gitignore`; sem licença (não decidida); commit `chore: add cargo workspace with four crates` sem trailer (commiter#DENY).
- Impacto: `cargo build` e `clippy -D warnings` limpos; sem comportamento ainda.
- Critérios afetados: nenhum

### [2026-11-07T18:45-03:00] — Fixture e testes unitários de extração (red)
- Tipo: alteração
- Contexto: alteração atômica 2 do Marco 1 aprovada (`1`).
- Decisão tomada: fixture `tests/fixtures/app/{billing,collections}`; 7 testes de §7 em `crates/memo-core/tests/extraction.rs` com docstring GIVEN/WHEN/THEN em `///` antes da função (dentro do corpo o clippy rejeita); stubs `todo!()` em `words.rs` e `extract.rs` para compilar; dependências adiadas para a alteração 3 (nada as usa ainda).
- Impacto: `cargo test` 7 falhas esperadas (TDD red); `clippy --all-targets -D warnings` e `fmt` limpos.
- Critérios afetados: A2, A3 (testes definidos)

### [2026-11-07T19:10-03:00] — Extração Python e palavras (green)
- Tipo: alteração
- Contexto: alteração atômica 3 do Marco 1 aprovada (`1`).
- Decisão tomada: `words.rs` (split, junção de papéis, slots, segmentos de teste) e `extract.rs` (tree-sitter: módulo, classe, função, método, atributo, `RECEBE`, `sig_hash`/`body_hash` com blake3); deps `tree-sitter 0.27.2`, `tree-sitter-python 0.25.0`, `blake3 1.8.7`. Arquivo com erro de sintaxe retorna `ExtractError` (tratado como `parse_error` na indexação).
- Impacto: 7 testes de §7 passam; `fmt` e `clippy --all-targets -D warnings` limpos. Não coberto ainda: teste que prove que mudança de código altera `body_hash` (sanidade do A3), a ser incluído com os testes de integração.
- Critérios afetados: A2, A3 (parcial: lado "não altera")

### [2026-11-07T20:10-03:00] — Armazenamento e indexação incremental
- Tipo: alteração
- Contexto: alteração atômica 4 do Marco 1 aprovada (`1`); dois commits: `e6c712d` (test, red) e `feat: add sqlite store and incremental indexer` (green).
- Decisão tomada: esquema da spec §4 em `store.rs`; `WorkTree` (git ls-files) em `source.rs`; `index()` com comparação por mtime/size, hash só nos diferentes, parse paralelo (rayon) e transação única; `thiserror` em `CoreError`; deps `rusqlite 0.41.1 (bundled)`, `rayon`, `thiserror 2`.
- Impacto: 11 testes passam (A1, A14, determinismo e 8 de extração); `fmt` e `clippy --all-targets -D warnings` limpos. O teste extra de sensibilidade do `body_hash` expôs um bug real: operadores (tokens anônimos) ficavam fora do hash (`a*b` = `a+b`); corrigido em `extract.rs`.
- Critérios afetados: A1, A14, A3 (completo); determinismo (princípio 2)
- Não feito ainda: símbolos de pacote (nível 5), `meta.indexed_ref`/`indexed_commit`, `raw_ref`/`import`, eventos; `query symbol`/`query word`.

### [2026-11-07T21:00-03:00] — memo CLI: query symbol e query word
- Tipo: alteração
- Contexto: alteração atômica 5 do Marco 1 aprovada (`1`); commits `b63a9d7` (test, red) e `feat: add memo cli with query symbol and query word` (green).
- Decisão tomada: `query.rs` no núcleo (resolve `qname` ou sufixo único; `Ambiguous` com candidatas); binário `memo` com `clap` (flags globais, exit 0/1/2/3/10, usage de clap remapeado de 2 para 1), render de texto da §8.3, envelope JSON `schema 1`; base em `$COMMON/memo/base.db`; `--task` aceito, mas ainda lê a base (derive é Marco 4). Dois ajustes de teste: `execute` é ambíguo com 4 candidatas (não 3) e o `--max-chars` do teste passou de 120 para 600, porque só os itens são cortados (cabeçalho e cobertura não) e o cabeçalho já passa de 120.
- Impacto: 17 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. Assinatura = `sig_hash` (o texto da assinatura não é armazenado); arestas e notas aparecem como "nenhuma" até os Marcos 2 e 4. Golden files revisados por leitura.
- Critérios afetados: Marco 1 fechado (A1, A2, A3, A14); falta A1–A3/A14 já cobertos; `memo.toml` e `index --ref` ficam para o `init` (Marco 5).

### [2026-11-07T22:30-03:00] — Arestas de tipo e efeito (Marco 2, parte 1)
- Tipo: alteração
- Contexto: alteração atômica 6 aprovada (`1`); commits `cfcf596` (test, red) e `feat: extract and resolve type and effect edges with writers readers type queries` (green).
- Decisão tomada: extrator gera `RECEBE`, `RETORNA`, `ESPECIALIZA`, `LEVANTA`, `ESCREVE`, `LE` e imports (relativos resolvidos, `TYPE_CHECKING`); `resolve.rs` recria as arestas de um arquivo a partir de `raw_ref`/`import`; reindexação re-resolve também quem aponta para símbolos alterados ou importa módulos alterados; `ESCREVE`/`LE` seguem `self.a.b` por tipos anotados de atributo (base da regra 4 da §5.4); comandos `query writers|readers|type`; `query symbol` agora lista arestas diretas; cobertura real (resolvidas · aprox · não resolvidas).
- Ajuste de esquema (a spec permite ajustar tipos e índices, não nomes nem semântica): colunas novas `symbol.type_text` e `raw_ref.attr`, `raw_ref.pos`, `raw_ref.resolved`; índices `raw_ref(file_id)` e `import(file_id)`.
- Ajuste de fixture: `Invoice.status` virou atributo de classe anotado (`status: str = "open"`); com `self.status = ...` no `__init__`, o próprio `Invoice.__init__` aparecia como terceiro escritor, e o A6 pede os dois casos de uso.
- Impacto: 29 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. Golden files revisados por leitura.
- Não feito: `CHAMA`/`CRIA` (regras 1–7), `callers`, `callees`, `impact`, `tests`, A4/A5; `ESCREVE` de variável de módulo/`global` e externo por `write_methods`; atributo herdado de base do repo na resolução de `self.x`.
- Incidente de processo: um slice meu no `query.rs` apagou trechos do arquivo; reconstruído a partir do commit `cfcf596` e validado pelos testes.

### [2026-11-07T23:40-03:00] — Resolução de chamadas e consultas de impacto (Marco 2 fechado)
- Tipo: alteração
- Contexto: alteração atômica 7 aprovada (`1`); commits `6a2d890` (test, red) e `feat: resolve calls by seven rules and add callers callees impact tests queries` (green).
- Decisão tomada: `CHAMA`/`CRIA` pelas regras 1–7 da §5.4 em duas fases (tipos e efeitos, depois chamadas); `ESPECIALIZA` estrutural (`~`) por Protocol recomputado a cada reindexação; `ESCREVE` externo (`~`) por `write_methods`; receptor local tipado vai em `raw_ref.receiver`; callers de métodos de Protocol são re-resolvidos quando as implementações mudam; `query callers|callees` (`--depth` 1–3), `impact` e `tests`.
- Impacto: 45 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. A4 e A5 cobertos. Golden files revisados por leitura. Fixture ganhou `checkout_usecase.py` (cadeia de profundidade 2) e um teste que chama `execute`.
- Limites: o teste da fixture chama `execute` via local construído (regra 6, `~`), então `tests` o lista como aproximado, não exato; `ESCREVE` de variável de módulo/`global` e atributo herdado de base do repo na resolução de `self.x` seguem pendentes.
- Critérios afetados: Marco 2 (A4, A5, A6); A7–A13 pendentes.

### [2026-11-08T00:30-03:00] — memo.toml e word_stats (Marco 3, parte 1)
- Tipo: alteração
- Contexto: alteração atômica 8 aprovada (`1`); commits `fc7c505` (test, red) e `feat: load memo.toml and compute word stats with pagerank` (green).
- Decisão tomada: `config.rs` (`toml` + `serde`, chaves desconhecidas rejeitadas, listas substituem a semente, `min_specificity` em 0..1, `stale_on` ∈ {sig, body}); vocabulário, `write_methods`, contexto e camada vêm da config; `stats.rs` recalcula `word_stats` dentro da transação de cada indexação que escreve (uso, alcance, grau, idf, especificidade normalizada, PageRank de 20 iterações com amortecimento 0,85 sem palavras esqueleto, `esqueleto` por lista ou limiar); `query word` mostra stats, marca `esqueleto: sim` e move palavras esqueleto vizinhas para `ignoradas`.
- Desvio da spec: contexto e camada usam `context_root` e `layers` (segmentos de caminho), e não globs, porque a lista de dependências da spec não inclui um motor de glob. A regra padrão é a da spec (segmento após `app/`; `domain`, `application`, `infra`).
- Decisão de cálculo: `N_símbolos` = total de símbolos do grafo; `especificidade` com idf constante vale 1,0.
- Observação: na fixture pequena, até `limit` (8 de ~50 símbolos) cai em `esqueleto` com o limiar 0,35 (espec 0,16). É efeito do tamanho da fixture; o limiar é para calibrar no repositório real (spec §11.5).
- Impacto: 59 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. Golden files revisados por leitura. Foi removido um `.snap.new` que entrou por engano no commit de teste.
- Limite: mudar `memo.toml` não reextrai arquivos inalterados (precisa de `index --full`, ainda não existe).
- Critérios afetados: Marco 3 parcial (`word_stats`); `brief`, deltas, A7 e A8 pendentes.

### [2026-11-08T01:30-03:00] — brief e deltas por task (Marco 3 fechado)
- Tipo: alteração
- Contexto: alteração atômica 9 aprovada (`1`); commits `2be3057` (test, red) e `feat: add brief command and per task deltas` (green).
- Decisão tomada: `brief.rs` segue a §8.5 (tokenização sem acento e sem stopwords pt/en; casamento exato ou por prefixo de 5+ caracteres, marcado `~`; âncoras = até 5 não esqueleto por especificidade × centralidade; pontuação por slot com bônus 1,0/0,7/0,2; expansão de um salto por `CHAMA`/`RECEBE`/`RETORNA` com fator 0,5; 15 símbolos; contorno dos 5 melhores; padrão da âncora principal; escritores dos atributos tocados); `delta.rs` guarda o que foi entregue em `session_seen` e devolve só o novo (`--task` ou `--since-last`; `--full` devolve tudo e só registra); o delta vale para `brief` e `query impact`.
- Decisões de detalhe: o score das âncoras é o peso normalizado pelo maior (a primeira vale 1,00); a chave de um item do delta inclui endereço e linhas, então mover um símbolo o faz voltar; o cabeçalho do `brief` ecoa o pedido, como no exemplo da §8.3 (o princípio 3 diz "nenhum eco da entrada": os dois trechos da spec divergem, e segui o exemplo); sem `base=<commit>` no cabeçalho, porque o índice é da árvore de trabalho e `meta.indexed_commit` ainda não existe.
- Impacto: 75 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. A7 e A8 cobertos. Golden revisado por leitura; a linha `padrão` mostra `-` quando a lista é vazia.
- Não feito: alias de nota `glossario` e a seção de notas (Marco 4; hoje `notas: nenhuma`); `--since-last` sem `--task` usa a chave de task `-`.
- Critérios afetados: Marco 3 fechado (A7, A8); A9–A13 pendentes.

### [2026-11-08T03:00-03:00] — Notas (Marco 4, parte 1)
- Tipo: alteração
- Contexto: alteração atômica 10 aprovada (`1`); commits `eae273f` (test, red) e `feat: add note storage with validation and note commands` (green).
- Decisão tomada: `notes.rs` guarda uma nota por arquivo em `$COMMON/memo/notes/<id>.md` (frontmatter + texto); `note add` valida na ordem da §7.2 (âncora existe → exit 2; `word:` esqueleto → `ancora_generica`, exit 4; texto > 1200 caracteres → `texto_longo`, exit 4), estado inicial `proposta` (`--confirm` → `confirmada`), lista até 5 `conflitos`, e `--substitui` marca a antiga `substituida`; âncoras `sym:` (qname completo ou sufixo único, normalizado), `word:`, `edge:` (exige aresta existente) e `file:`; escrita por `.md.tmp` + rename (P2); notas de task só aparecem na task (`--task` grava `escopo: task:<id>`); `notes_for` aplica os pesos 1,0/0,5/0,3 e exclui `orfa` e `substituida`; `brief` ganhou a seção de notas e o alias `glossario`; `query symbol` e `query impact` mostram as notas do símbolo.
- Decisões de detalhe: sem tabela de notas no SQLite (o índice reconstruível da spec é a própria leitura dos `.md`, a custo de listar o diretório a cada comando); `ancora_sig`/`ancora_body` guardam os 12 primeiros caracteres do hash (o exemplo da spec mostra 6); o `id` é `n-` + 8 caracteres base36 derivados de blake3(âncora, texto, carimbo, tentativa); `criada` é UTC (`...Z`), porque não há dependência de fuso horário na lista da spec, e `MEMO_NOW` fixa o carimbo nos testes; a seção `notas` do `brief` corta o texto em 120 caracteres.
- Impacto: 90 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. Golden revisado por leitura. O commit vermelho foi emendado, porque nasceu sem compilar o CLI (faltava o mapeamento dos novos erros de domínio para exit 3 e 4).
- Limites: `stale`, `órfã`, renomeação, `recheck` e `estado_anterior` ficam para a alteração 11; `derive`, `promote`, `drop` e `status` para a 12; eventos `NOTE_*` ainda não existem.
- Critérios afetados: A12 coberto; A9, A10, A11 e A13 pendentes.

### [2026-11-08T03:05-03:00] — Correção do log anterior
- Tipo: alteração
- Contexto: a entrada "Notas (Marco 4, parte 1)" diz 90 testes; a contagem real do `cargo test` é 101 (0 falhas).
- Decisão tomada: registrar a correção sem alterar a entrada anterior (log append-only).
- Impacto: nenhum no código.
- Critérios afetados: nenhum

### [2026-11-08T05:00-03:00] — Eventos e ciclo de vida das notas (Marco 4, parte 2)
- Tipo: alteração
- Contexto: alteração atômica 11 aprovada (`1`, após `on1` não aceito); commits `20c51ee` (test, red) e `feat: record event batches and apply note lifecycle transitions on reindex` (green).
- Decisão tomada: `events.rs` compara instantâneos de símbolos e palavras antes e depois do lote, dentro da transação, e grava `SYMBOL_RENAMED`, `SYMBOL_CREATED`, `SYMBOL_DESTROYED`, `SYMBOL_SIGNATURE_CHANGED`, `SYMBOL_BODY_CHANGED`, `WORD_CREATED` e `WORD_DESTROYED` no lote `b-<n>` (sequencial, sem relógio); renomeação exige mesmo `kind`, `sig_hash` e `body_hash` e pareamento único; as transições de nota (§7.3) rodam a cada indexação, contra o grafo atualizado, e são idempotentes (comparam estado com estado): `stale` com `estado_anterior` (P3), `orfa`, reancoragem por renomeação sem `stale`, `stale_on` configurável, âncoras `word:`, `file:` e `edge:` (aresta sumida: `stale` se um extremo mudou de assinatura no lote, senão `orfa`); `note recheck` atualiza os hashes e restaura o estado anterior; `NOTE_ADDED`, `NOTE_CONFIRMED`, `NOTE_SUPERSEDED`, `NOTE_STALE` e `NOTE_ORPHANED` são gravados; comando `memo index` com o resumo de uma linha; `--no-index` não aplica nenhuma transição (P4).
- Consequência da spec que errei nos testes: `body_hash` de módulo e de classe cobre o corpo inteiro, então renomear ou editar um método também gera `SYMBOL_BODY_CHANGED` para a classe e o módulo (por isso o resumo mostra `~2 alterados` após um rename). Corrigi as expectativas dos testes, e não o código. Efeito prático: com `stale_on` padrão, uma nota ancorada em classe ou módulo fica `stale` a cada mudança dentro dela (ponto em aberto 8 da spec).
- Decisão de detalhe: os `.md` das notas são reescritos depois do commit do SQLite; se o processo morrer entre os dois, a próxima indexação recalcula as transições de estado (idempotentes), mas perde o par de renomeação do lote e os eventos `NOTE_*` daquele lote.
- Impacto: 129 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. A9 e A10 cobertos; P3 e P4 cobertos.
- Limites: âncoras secundárias só acompanham renomeações, não geram `stale` nem `orfa`; `derive`, `promote`, `drop`, `status` e o lock de P1 (30 s, exit 3) ficam para a alteração 12.
- Critérios afetados: A9 e A10 cobertos; A11 e A13 pendentes.

### [2026-11-08T07:00-03:00] — Grafo por task, lock e status (Marco 4 fechado)
- Tipo: alteração
- Contexto: alteração atômica 12 aprovada (`1`); commits `62749a6` (test, red) e `20a5d4d` (green).
- Decisão tomada: `lock.rs` usa `File::try_lock` (std) com espera de 30 s, `MEMO_LOCK_TIMEOUT_MS` a substitui e estourar dá exit 3 (D6 e D10); todo comando segura o lock do banco que usa; `derive <task>` copia a base pela API de backup do SQLite e reindexa a árvore atual (recusa task já derivada, exit 1); `--task` (ou o branch atual, com `/` virando `-`) usa `features/<task>.db` se existir, senão a base; `GitRef` implementa `SourceTree` por `git ls-tree` e `git cat-file --batch` (ref fixada no commit, `mtime_ns` derivado do blob); `index --ref` e `--full` (invalida hashes sem gerar eventos); `meta.indexed_ref` e `indexed_commit`; `promote` reindexa a base pela principal (`main_branch`, `origin/HEAD`, `main`, `master`) e troca `task:<id>` por `repo`, com `NOTE_PROMOTED` e `orfa` para âncora ausente na base; `drop` remove feature e notas da task; `status` resume base, features, notas por estado e tasks com branch mergeado.
- Decisão de escopo das transições: o grafo da base só avalia notas de escopo `repo`, e o da feature só as de `task:<id>`; assim, trabalhar numa feature não marca como `stale` notas do repositório antes do `promote`.
- Divergência da spec (a revisar com o humano): a spec manda a base indexar a branch principal por ref e a feature a árvore de trabalho. Aqui, sem feature derivada, os comandos reindexam a árvore de trabalho na base (comportamento que os testes dos Marcos 1 a 3 já assumem); `index --ref` e `promote` indexam a base pela ref. O `init` (Marco 5) deve indexar a base pela principal.
- Limites: `derive` só avisa "base atrás da principal" quando `meta.indexed_commit` existe; `status` lista como promovíveis só as tasks cujo branch tem o mesmo nome e está mergeado (sem `gh pr view` nem comparação de símbolos); âncoras secundárias seguem sem `stale`/`orfa`.
- Impacto: 148 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. A11 coberto; P1 coberto (lock e concorrência); Marco 4 fechado.
- Critérios afetados: A11 coberto; resta o A13 (Marco 5).

### [2026-11-08T09:00-03:00] — init e adaptadores de harness (Marco 5, parte 1)
- Tipo: alteração
- Contexto: alteração atômica 13 aprovada (`1`); commits `ab7219e` (test, red) e `feat: add init command and claude and codex harness adapters` (green).
- Documentação consultada em 2026-11-08 (WebSearch; páginas oficiais):
  - Claude Code: https://code.claude.com/docs/en/mcp-quickstart e https://code.claude.com/docs/en/mcp — `.mcp.json` na raiz do projeto, objeto `mcpServers`, servidor stdio com `type`, `command`, `args`.
  - Codex: https://developers.openai.com/codex/guides/agents-md (instruções em `AGENTS.md`), https://developers.openai.com/codex/mcp e https://developers.openai.com/codex/config-reference — servidores MCP em tabelas `[mcp_servers.<nome>]` de `config.toml`; no projeto, `.codex/config.toml` (só vale em projeto confiável).
  - Limite da consulta: foi uma busca com resumo, não a leitura integral das páginas; o formato de `.mcp.json` e a tabela `[mcp_servers.<nome>]` com `command` e `args` vieram dos resumos. Confirmar contra o harness real antes de usar fora deste repositório.
- Decisão tomada: trait `HarnessAdapter { id, detect, install, uninstall, doctor }` em `memo-harness`; `claude` grava `CLAUDE.md` e, com `--mcp`, `.mcp.json` (merge, mantém outros servidores, não reescreve se já está igual, JSON inválido é erro e nada é gravado); `codex` grava `AGENTS.md` e, com `--mcp`, um bloco marcado em `.codex/config.toml` (conflito se a tabela existe fora dos marcadores); `agy` e ids desconhecidos dão exit 1 antes de qualquer escrita; o bloco da §9.3 é fixo e a reinstalação troca só o que está entre os marcadores; `memo init [<repo>] <harness> [--ref] [--mcp] [--print]` segue a ordem da §9.1 (cria `$COMMON/memo`, grava `memo.toml` com as sementes sem nunca sobrescrever, indexa a base pela principal por ref, instala a integração); `--print` não escreve nada, nem `$COMMON/memo`; sem branch principal a base fica adiada.
- Resolvido: a divergência do Marco 4 (base pela ref): o `init` agora indexa a base pela principal; consultas posteriores sem feature ainda reindexam a árvore de trabalho na base e, se a base vinha de uma ref, o `meta` passa a `indexed_ref=worktree`.
- Limites: o `config.toml` do codex é do projeto (`.codex/`), não o global em `~/.codex`; `doctor` existe na trait e é testado, mas não há comando de CLI para ele (não está na §8.2); `.mcp.json` reformatado em JSON indentado com chaves em ordem alfabética quando é preciso alterá-lo.
- Impacto: 171 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. A13 coberto.
- Critérios afetados: A13 coberto; resta o servidor MCP (`serve`).

### [2026-11-08T09:30-03:00] — Decisão do servidor MCP
- Tipo: decisão
- Contexto: GRILL da ENGINEERING; `grill-me · diferida · — · ENGINEERING · 1 rodada · 1 decisão · STATUS:READY`. Verificação do `rmcp` por busca: uma fonte secundária cita a 3.8.0; páginas oficiais citam versões antigas; sem notas de versão confirmadas.
- Decisão tomada: D11, JSON-RPC mínimo à mão por stdio.
- Impacto: sem dependência nova no `serve`; o protocolo fica sob testes nossos.
- Critérios afetados: nenhum

### [2026-11-08T11:00-03:00] — Servidor MCP (Marco 5 fechado)
- Tipo: alteração
- Contexto: alteração atômica 14 aprovada (`1`); commits `2bd53d9` (test, red) e `d49e08d feat: add memo serve mcp server over stdio` (green).
- Decisão tomada (D11): `memo-mcp` fala JSON-RPC por linhas sobre stdio, só com `serde_json`: `initialize` (ecoa a versão pedida; sem pedido, `2024-11-05`), `notifications/initialized` sem resposta, `ping`, `tools/list` (4 ferramentas, descrição de uma linha, `inputSchema`) e `tools/call`; erros `-32700`, `-32600`, `-32601` e `-32602`; erros de domínio saem como `isError: true`. O protocolo e o contrato das ferramentas ficam em `memo-mcp`; quem executa é um `Backend` que, no CLI, roda o comando equivalente em processo (`execute(argv)`), então o texto é idêntico ao do shell. `note_add` grava `origem: agente`.
- Refatoração: o `main` do CLI virou `execute(args) -> Outcome` (código, stdout, stderr); as saídas existentes não mudaram (golden files iguais).
- Não verificado: os formatos de `initialize`, `tools/list` e `tools/call` seguem o que eu conheço do protocolo MCP, sem leitura da especificação vigente nesta sessão; a versão de protocolo `2024-11-05` de fallback também não foi conferida. Conferir com um cliente real (Claude Code ou Codex) antes de depender dele.
- Limites: sem `resources`, `prompts` nem notificações de progresso; sem transporte HTTP; sem lote JSON-RPC (`-32600`).
- Impacto: 193 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos.
- Critérios afetados: Marco 5 fechado (A13 e `serve`).

### [2026-11-08T12:00-03:00] — Conformidade com §7 e medições (fim da ENGINEERING)
- Tipo: validação
- Contexto: alteração atômica 15 aprovada (`1`); commit `a2990bf test: align test names with task section 7`.
- Decisão tomada: 18 testes renomeados para os nomes de §7 (com seus `.snap` por `git mv`) e 1 teste novo (`test_given_no_index_flag_when_query_run_then_no_note_transition_no_event`); os 33 nomes de §7 existem agora; todos os testes têm docstring GIVEN/WHEN/THEN em inglês (varredura sem faltas); §4 e §7 marcados com a evidência. Os renomeados eram não conformidades minhas (nome diferente do §7).
- Medições da §11.4 (binário `--release`, repositório sintético de 5.000 arquivos Python gerado em /tmp, fora do repositório; máquina de desenvolvimento, uma execução):
  - indexação completa: 45,9 s (meta 60 s: atendida, com pouca folga);
  - reindexação sem mudança: 24 a 31 ms em 5 amostras (meta 300 ms);
  - `brief`: 26 a 32 ms (meta 500 ms); `query symbol`: 30 ms.
  - Ressalva: o repositório sintético tem estrutura repetitiva e poucas arestas entre arquivos; o repositório real pode diferir. A indexação completa é o ponto a vigiar.
- Impacto: 194 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos.
- Pendências conhecidas para o CODE_REVIEW (todas registradas acima): critério golden parcial; `serve` sem conferência contra a especificação MCP; formatos de `.mcp.json` e `.codex/config.toml` vindos de busca; divergência de base (árvore de trabalho × ref) mitigada; âncoras secundárias sem `stale`/`orfa`; `ESCREVE` de variável de módulo e `global` não implementado.

### [2026-11-08T13:00-03:00] — ENGINEERING → CODE_REVIEW
- Tipo: alteração
- Contexto: GATE.OUT da ENGINEERING cumprido (194 testes, `fmt` e `clippy` limpos, alterações commitadas, decisões D1–D11 gravadas); humano aprovou (`1`) a leitura da skill `code-reviewer` e o início da revisão.
- Decisão tomada: STATE=CODE_REVIEW, ROLE=code-reviewer, estado `in-progress`; revisão dos commits `01f4792..a2990bf` da branch `master`; header da TASK conferido (branch `master`, workdir atual).
- Verificações executadas em repositório descartável em /tmp (apagado): `derive ../evil` criou `evil.db` fora de `features/`; `drop ../base` apagou `base.db`; uma nota corrompida fez `query` sair com exit 3; `--task` com quebra de linha duplicou `estado:` no frontmatter da nota; o argumento posicional `--json` foi aceito como flag.
- Impacto: início do CODE_REVIEW.
- Critérios afetados: nenhum

### [2026-11-08T13:30-03:00] — code review Requer alterações
- Tipo: bloqueio
- Contexto: revisão de `01f4792..a2990bf4` (26 commits); `n-partite · delta · full · CODE_REVIEW`; `grill-me · diferida · — · CODE_REVIEW · 1 rodada · 3 decisões · 0 pendências · STATUS:READY`.
- Decisão tomada: verdict Requer alterações; 4 🔴 (TASK desatualizada por DRIFT-2 e DRIFT-3, DRIFT-1 código errado, id de task sem validação, injeção de frontmatter), 4 🟡, 4 🟢, 1 ⚖️; STATE:BLOCKED com retorno à DEFINITION (D13 e D14) e, depois, REQUEST CHANGES para a eng (itens 2 a 6 e 8).
- Impacto: DoD não satisfeito; sem merge.
- Critérios afetados: §4 (golden parcial); §5 e a spec (D13, D14 a registrar na DEFINITION).

### [2026-11-08T14:00-03:00] — Retorno à DEFINITION: D13 e D14
- Tipo: alteração
- Contexto: code review devolveu `TASK desatualizada` (DRIFT-2 e DRIFT-3, classificados pelo humano como modelo desatualizado); humano aprovou (`1`) o retorno.
- Decisão tomada: D13 e D14 refletidos em §4 e §5 da TASK e na spec (§2 princípio 3, §4 contexto e camada, §6.1 chaves do `memo.toml`); `grill-me` não reaberto (decisões já tomadas na rodada do CODE_REVIEW); n-partite não reexecutada (sem mudança de conceito, relação ou ciclo).
- Impacto: sem mudança de código; aguarda aprovação única do §D para voltar a ENGINEERING com os itens 🔴 2 a 4 e 🟡 do review.
- Critérios afetados: §4 (D13, D14 novos); §5 restrições técnicas

### [2026-11-08T14:30-03:00] — DEFINITION aprovada (2ª) e TEST_ANALYSIS dos itens do review
- Tipo: decisão
- Contexto: humano aprovou (`1`) D13 e D14; a TEST_ANALYSIS derivou testes para D12 e os itens 🔴 2 a 4 e 🟡 5 e 6 do review.
- Decisão tomada: 12 testes novos em §7 (nomes ≤ 89 caracteres; docstrings GIVEN/WHEN/THEN escritas na ENGINEERING); valores concretos vêm do review: regex do id de task, exit 1 para id inválido, exit 4 para quebra de linha, `cobertura.notas_com_erro` para nota corrompida, separador `--` no argv do MCP; 5 critérios novos em §4. Lacunas de valor esperado: 0, então o `grill-me` não foi chamado. Fora de teste: 🟡 7 (perf, por medição) e 8 (validação com clientes reais), 🟢 e ⚖️.
- Impacto: STATE → ENGINEERING (re-entrada REQUEST CHANGES); o frame de `n-partite` de DEFINITION continua válido.
- Critérios afetados: §4 (5 novos)

### [2026-11-08T15:30-03:00] — REQUEST CHANGES, parte 1: segurança (🔴 3 e 4, 🟡 6)
- Tipo: alteração
- Contexto: alteração atômica 16 aprovada (`1`); commits `3605ec3` (test, red) e `fix: validate task ids, reject multiline note fields and guard mcp argv` (green).
- Decisão tomada: `memo_core::task::is_valid_id` (`^[A-Za-z0-9][A-Za-z0-9._-]*$`, sem `..`, até 64 caracteres); o CLI recusa id inválido em `--task`, `derive`, `promote` e `drop` com exit 1 antes de qualquer E/S; branch atual com caracteres fora do padrão simplesmente não tem feature; `NoteDir::add` rejeita `\n` e `\r` em `task`, `ancora`, `secundarias`, `aliases` e `substitui` (exit 4) e id de task inválido; o parser rejeita chave duplicada no frontmatter; o `serve` coloca `--` antes dos posicionais de `brief`, `query` e `note_add`.
- Ajuste: 5 testes do protocolo MCP existentes tiveram o argv esperado atualizado, porque o contrato do argv mudou de propósito; um teste meu de alias passava por engano (palavra esqueleto), corrigido antes do commit vermelho.
- Impacto: 202 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. 8 dos 12 testes novos de §7 marcados; ficam os 4 da alteração 17 (D12 e nota corrompida).
- Critérios afetados: id de task, campos de uma linha e `--` do MCP

### [2026-11-08T16:30-03:00] — REQUEST CHANGES, parte 2: D12 e nota corrompida (🔴 2, 🟡 5)
- Tipo: alteração
- Contexto: alteração atômica 17 aprovada (`1`); commits `ddd60ce` (test, red) e `fix: index the base from the main ref and skip corrupted notes` (green).
- Decisão tomada: sem feature derivada, todo comando reindexa a base a partir da ref principal (`main_branch`, `origin/HEAD`, `main`, `master`; incremental por blob); a árvore de trabalho só alimenta a feature (`derive`, ou `--task`/branch com feature); `meta.indexed_ref/indexed_commit` descrevem sempre a ref (`mark_worktree` removido). Repositório sem branch principal (decisão de borda minha): a base indexa a árvore de trabalho e o CLI avisa em uma linha no stderr. `NoteDir::scan` pula arquivos de nota inválidos e o CLI os conta em `cobertura.notas_com_erro` (JSON sempre; texto só quando > 0).
- Ajuste de testes de CLI: `Repo::fixture()` passa a commitar a fixture em `main` (`fixture_uncommitted()` para o caso sem commits); `edit_py` commita após editar, porque agora é o commit que a base enxerga; o helper de commit usa `commit.gpgsign=false` e datas fixas (a configuração GPG do ambiente tornava o hash do commit não determinístico e quebrava o golden do `status`); o golden de `status` agora mostra `ref=main commit=<hash determinístico>`. Nomes de §7 intactos.
- Impacto: 206 testes passam (duas execuções seguidas, sem `.snap.new`); `fmt` e `clippy --all-targets -D warnings` limpos. 🔴 2 e 🟡 5 do review corrigidos.
- Pendentes do review: 🟡 7 (perf da indexação completa), 🟡 8 (clientes reais), 🟢 e ⚖️ (decisão do humano).

### [2026-11-08T17:30-03:00] — code review Aprovado c/ ressalvas
- Tipo: validação
- Contexto: re-review de `a2990bf4..bfc0c71` (4 commits novos); `n-partite · delta · full · CODE_REVIEW`; sem gatilho de grill-me (decisões D15 e D16 vieram da resposta `1` do humano às recomendações); `grill-me · não chamado · sem gatilho · CODE_REVIEW`.
- Decisão tomada: verdict Aprovado c/ ressalvas; 🔴=0; DRIFT=0; DOD satisfeito (206 testes, `fmt` e `clippy` limpos); ressalvas: 🟡 8 (clientes reais) e 🟡 7 (risco aceito, D15), 🟢 e ⚖️ (D16) herdados.
- Impacto: STATE → DONE (`Estado: done`). O merge continua dependendo de aprovação humana.
- Critérios afetados: todos os de §4 atendidos, exceto "toda saída da §8 coberta por golden file", que segue parcial (registrado em §4).

### [2026-11-08T17:35-03:00] — Correção: estado não é `done`
- Tipo: alteração
- Contexto: a entrada anterior marcou `Estado: done`, mas o DOD exige todos os critérios de §4 atendidos, e o critério "toda saída da spec §8 coberta por golden file" segue parcial.
- Decisão tomada: `Estado` volta a `in-progress`; o verdict do review continua Aprovado c/ ressalvas; o `done` depende de o humano aceitar o critério parcial (registrando a decisão) ou de a eng completar os golden de `note list/show/confirm/recheck`, `derive` e `drop`.
- Impacto: nenhum no código.
- Critérios afetados: §4 (golden parcial)

### [2026-11-08T17:45-03:00] — DONE
- Tipo: decisão
- Contexto: humano respondeu `1` à opção A (aceitar o critério de golden como parcial), após rejeitar a primeira tentativa de gravação e reenviar a aprovação.
- Decisão tomada: D17 gravada em §9; critério aceito; DOD satisfeito (critérios de §4 atendidos ou aceitos, 206 testes passando, review Aprovado c/ ressalvas, 🔴=0); STATE → DONE, `Estado: done`.
- Impacto: ressalvas abertas: 🟡 8 (validar `serve` e os formatos de `.mcp.json` e `.codex/config.toml` com clientes reais), 🟡 7 (perf, D15), 🟢 9 a 12, ⚖️ traits (D16). O merge depende de aprovação humana; nada foi publicado.
- Critérios afetados: §4 todos atendidos ou aceitos

### [2026-11-08T17:50-03:00] — Lifecycle Coverage marcado
- Tipo: validação
- Contexto: itens de Lifecycle Coverage de §7 estavam sem marca, embora os testes existam e passem (`renamed_function`, `changed_signature`, `index_again_no_writes`, `confirm_flag`, `estado_anterior_restored`, `derived_feature`, `task_promoted`, `dropped_task`).
- Decisão tomada: marcar os 3 itens; "orfa" tem cobertura em `note_lifecycle.rs` (símbolo, palavra, arquivo e aresta). Os checkboxes de §6 (Impacto em Dados) são declarativos e ficam como estão.
- Impacto: nenhum no código.
- Critérios afetados: nenhum

### [2026-11-08T19:00-03:00] — Bug: `UNIQUE constraint failed: symbol.qname` no `init`
- Tipo: alteração
- Contexto: o humano rodou `memo init claude --mcp` num repositório real e a indexação falhou com exit 3. Reproduzido com repositório descartável: duas causas de `qname` duplicado, que a fixture não cobria.
- Causas: (1) mesma `qname` dentro de um arquivo: `@property` com `@x.setter`, atributo e método com o mesmo nome, função definida duas vezes; (2) mesma `qname` em arquivos diferentes: `pkg/__init__.py` com `foo` e o módulo `pkg/foo.py`, ou `pkg.py` e `pkg/__init__.py`.
- Decisão tomada: vale a primeira definição. No extrator, `add_symbol` descarta a redefinição no mesmo arquivo (efeitos e referências dela seguem atribuídos ao símbolo mantido); na indexação, um `qname` já presente no grafo é pulado. 4 testes novos em `crates/memo-core/tests/indexing.rs`, vermelhos antes da correção. Fora de §7 (extra, aguardando aprovação do humano para o commit).
- Limites: o corpo de um setter não entra no `body_hash` do `@property` (mudar só o setter não gera evento nem `stale`); na colisão entre arquivos, indexação incremental pode manter outro vencedor do que uma indexação completa (a ordem de arquivos muda), então o resultado não é garantidamente idêntico nesse caso raro.
- Impacto: 210 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos; `init` no repositório de teste com os quatro casos sai com exit 0. Desempenho da indexação completa não foi remedido depois do `SELECT` extra por símbolo.
- Critérios afetados: nenhum de §4
