# TASK-2 — Construir o grafo de palavras com pontes tipadas no `memo`
branch: master
workdir: /var/home/erickod/Documentos/dev/memmo
created_at: 2026-11-07

## Status
- Estado: done
- Owner: Erick Duarte
- ROLE requerida: eng

---

## 1. Objetivo
Implementar no `memo` o grafo de palavras com pontes tipadas (símbolo, arquivo, chamada, teste e import), a partir de nomes, diretórios, docstrings de teste e imports. (REF: DOR#CRITERIA)

---

## 2. Contexto
- Origem: uso do `memo` num repositório real (`easy_lease_integrated/backend`, 801 arquivos, 6.341 símbolos). A palavra `order`, núcleo do domínio (450 usos em 118 arquivos), foi marcada `esqueleto` (especificidade 0,06) e deixou de ser âncora; o `brief` listou fakes e testes acima do código de produção; 1.740 chamadas ficaram sem resolução, como `order.cancel(...)` em `RequestPaymentUseCase` e `CancelOrderUseCase`.
- Estado atual (TASK-1, spec §5.2 e §5.6): palavras só vêm de nomes de símbolos (e módulo); uma palavra enxerga outra só por coocorrência no mesmo símbolo; `esqueleto` decide por IDF (`min_specificity`) ou lista `skeleton`; `brief` dá um salto por `CHAMA`, `RECEBE` e `RETORNA`.
- Este trabalho muda o modelo da spec (§5.2 e §5.6, e o que `brief` e `query word` devolvem); a spec será atualizada na consolidação.
- Stack: Rust, SQLite, `tree-sitter-python`, as mesmas dependências da TASK-1.

---

## 3. Escopo

### Dentro do escopo
- Fontes de palavras: nome de símbolo (já existe), nome de arquivo e de diretório, docstrings de testes (GIVEN/WHEN/THEN) e imports (nome importado e caminho do módulo; externos com relevância menor, não zero).
- Pontes tipadas entre palavras: mesmo símbolo, teste e símbolo testado, import, chamada (chamador e chamado), mesmo arquivo, mesmo contexto; pesos configuráveis.
- Medida de coerência da vizinhança para decidir o que é conceito central e o que é `esqueleto`.
- `memo query word` passa a devolver a ficha do conceito (vizinhança por tipo de ponte, com o motivo de cada item).
- `memo brief` usa a vizinhança só para ranquear, mostra a etiqueta das pontes por símbolo e agrupa os testes numa seção própria ao final.
- Flag global `--no-tests` (padrão falso) nas consultas; erro de uso em `query tests`.
- Relatório de aliases sugeridos (somente leitura): pares de palavras com prefixo comum ou vizinhança parecida, com o `note add` pronto.
- Marca `âncora genérica` em `note list` e `status` para nota cuja palavra virou `esqueleto`.
- Evidência de import para resolver chamadas ambíguas como aresta aproximada (`~`).

### Fora do escopo
- Stemming ou tradução automáticos (a spec proíbe); sinônimos seguem por nota `glossario`.
- Outras linguagens, LLM, rede e relógio nas consultas.
- Docstrings e comentários de produção (segunda fase; `source` já prevê o valor `doc`).
- Materializar as pontes em tabela (só se a medição de desempenho exigir).

---

## 4. Critérios de Aceitação
verificáveis (teste | comando | valor) · regras de transição

- [x] `occurrence` ganha `owner_kind` (symbol, file, import) e `source` (name, path, import, test_doc); palavras de diretório, import e docstring de teste são gravadas com dono e origem (teste de integração sobre a fixture)
- [x] palavras de docstring de teste só entram se existirem como palavra de algum identificador; `given`, `when` e `then` são marcadores de segmento, não palavras
- [x] o segmento de contexto (após `app/`) grava o slot `context`; as demais palavras seguem o vocabulário (ação, objeto, papel)
- [x] import gera palavras pelo nome importado e pelo caminho do módulo; import de alvo fora do repositório pesa metade (0,3)
- [x] pesos das pontes em `[bridges]` do `memo.toml`: símbolo 1,0 · teste e símbolo testado 0,8 · import 0,6 · chamada 0,5 · arquivo 0,3 · contexto 0,1; ponte aproximada vale metade
- [x] coerência = peso das 10 vizinhas mais fortes (não esqueleto, desempate alfabético) dividido pelo peso total das pontes da palavra, de 0 a 1
- [x] `esqueleto` = palavra em `skeleton` OU (IDF abaixo de `min_specificity` E coerência abaixo de `min_coherence`, padrão 0,30); na fixture, palavra frequente e coerente não é esqueleto, e palavra genérica espalhada é
- [x] palavras de um teste só se propagam ao símbolo testado por chamada `CHAMA` exata; a sobreposição de palavras fica só em `query tests`
- [x] `memo query word <palavra>` devolve a ficha: vizinhas por tipo de ponte com os pesos e o motivo de cada símbolo (golden `insta`)
- [x] `memo brief` mostra a etiqueta de pontes por símbolo e agrupa os testes numa seção ao final; o corte de `--max-chars` tira primeiro os testes e o `+N omitidos` conta cada seção (golden `insta`)
- [x] `--no-tests` esconde os testes da saída de `brief` e de `query`, não altera as estatísticas, informa `N testes ocultos` na cobertura, dá exit 1 em `query tests` e vira `no_tests` no MCP
- [x] chamada ambígua cujo arquivo importa uma única classe com aquele método vira `CHAMA ~` para ela; com mais de uma classe candidata importada, segue sem aresta
- [x] nota com âncora `word:` cuja palavra virou `esqueleto` continua ativa, sem evento, e aparece com `âncora genérica` em `note list` e `status`
- [x] o relatório de aliases lista pares por prefixo comum ou vizinhança parecida com o `note add` pronto e não grava nada
- [x] `schema_version` sobe para 2: um banco derivado com outra versão é descartado e reindexado, sem tocar nas notas
- [x] mesma árvore e mesma config geram a mesma saída byte a byte, incluindo coerência e esqueleto
- [x] indexação completa de 5.000 arquivos ≤ 60 s, reindexação sem mudança ≤ 300 ms, `brief` ≤ 500 ms (mesmo gerador sintético da TASK-1); medida, sem bloquear o aceite
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas

---

## 5. Restrições e Regras
- técnicas: princípios 1 a 7 da spec §2 (determinístico, saída compacta, cobertura explícita, falha parcial não é falha); dependência fora da lista da spec exige justificativa; `memo-core` não imprime nem depende de `clap` ou MCP.
- negócio: o `memo` entrega fatos, não julga código.
- dependências obrigatórias: TASK-1 concluída (`done`).
- regra de dados: o banco do `memo` é derivado e descartável (princípio 1 da spec); a mudança de esquema não migra, recria; as notas (`.md`) não mudam.
- calibração: `min_coherence`, `min_specificity` e os pesos são valores iniciais; a calibração no repositório real é do humano e não bloqueia o aceite.
- segurança · performance: a indexação completa de 5.000 arquivos deve continuar dentro de 60 s (meta da spec §11.4, remedir depois da mudança).

---

## 6. Impacto em Dados
alteração estrutural (schema) → task inválida

- [ ] Nenhuma alteração de schema
- [x] Uso de entidades existentes
- [ ] Leitura apenas
- [x] Escrita controlada

Observação (aprovação explícita no §D): a TASK altera o esquema do banco do `memo`, que é derivado: `occurrence` ganha `owner_kind` e `source`, e `schema_version` sobe para 2 com recriação. A regra do template ("alteração estrutural → task inválida") foi escrita para bancos de aplicação; aqui o banco é descartável e reconstruído do código. Nenhuma tabela de pontes é criada (D19).

---

## 7. Estratégia de Testes
escritor: test-analyst

**Cenários** — Happy Path: fontes novas, pontes, coerência, ficha, `brief`, `--no-tests`, evidência de import, aliases · Error Scenarios: `query tests --no-tests` (exit 1), banco com outra `schema_version` · Edge Cases: mais de 10 vizinhas, desempate alfabético, duas classes importadas candidatas, palavra que morre pela edição da docstring, palavra de prosa fora do vocabulário.

**Rastreabilidade**
- fontes e ocorrência (critérios 1 a 4) → testes 1 a 10 de unidade e 1 de integração de determinismo
- pesos e pontes (critério 5) → `default_bridges`, `approximate_bridge`, `edge_change`
- coerência e esqueleto (critérios 6 e 7) → `more_than_ten_neighbors`, `equal_neighbor_weights`, `skeleton_list_word`, `low_idf_and_*`, `order_like_fixture`, `get_like_fixture`
- propagação de teste (critério 8) → `test_calls_symbol_exact`, `test_calls_symbol_approx`, `word_overlap_only`
- ficha e `brief` (critérios 9 e 10) → `card_lists_neighbors`, `symbols_show_bridge_tags`, `tests_listed_in_final_section`, `tests_cut_first`
- `--no-tests` (critério 11) → `no_tests_flag_when_*` (4) e `no_tests_argument_when_mcp`
- evidência de import (critério 12) → `ambiguous_call_and_one_imported_class`, `two_imported_classes`
- nota × esqueleto (critério 13) → `note_on_word_turned_esqueleto`, `note_on_generic_word_when_*`
- aliases (critério 14) → `common_prefix_5`, `jaccard_over_0_6`, `alias_report_when_run`, `alias_report_cli`
- esquema v2 (critério 15) → `old_schema_version`
- determinismo (critério 16) → `same_tree_twice`
- desempenho (critério 17) → não coberto por teste de passa/falha: medição registrada no log (meta, não bloqueia o aceite)
- qualidade (critério 18) → comandos `cargo fmt`, `clippy` e `test`
- não coberto: calibração de `min_coherence`, `min_specificity` e dos pesos no repositório real — decisão do humano (TASK §5)

**Lifecycle Coverage**
- [x] Palavra: nasce em docstring, import ou caminho → `given_test_docstring...`, `given_import_statement...`, `given_directory_segments...`
- [ ] Palavra: morre quando a última ocorrência some → `given_word_only_in_test_docstring_when_docstring_edited_then_word_destroyed_event`
- [x] Esqueleto: estado recalculado quando o corpus muda → `given_docstring_edit_when_reindexed_then_esqueleto_status_recomputed`
- [ ] Ponte: derivada e recalculada junto com as fontes → `given_edge_change_when_reindexed_then_bridge_weights_follow`
- [x] Nota `word:` × esqueleto: sem transição → `given_note_on_word_turned_esqueleto_when_reindexed_then_active_without_event`

### Testes Unitários
- [x] test_given_symbol_word_when_stored_then_occurrence_has_owner_kind_and_name_source  (81 chars)
- [x] test_given_directory_segments_when_indexed_then_words_have_path_source_and_file_owner  (85 chars)
- [x] test_given_import_statement_when_indexed_then_words_from_name_and_path_with_import_source  (89 chars)
- [x] test_given_test_docstring_when_indexed_then_words_have_test_doc_source_and_segments  (83 chars)
- [x] test_given_prose_word_absent_from_identifiers_when_indexed_then_not_stored  (74 chars)
- [x] test_given_given_when_then_markers_when_docstring_split_then_markers_not_words  (78 chars)
- [x] test_given_context_segment_when_indexed_then_word_has_slot_context  (66 chars)
- [x] test_given_context_root_path_prefix_when_indexed_then_context_is_next_segment  (77 chars)
- [x] test_given_import_path_when_split_then_root_and_layer_segments_dropped  (70 chars)
- [x] test_given_external_import_when_split_then_all_segments_kept_with_weight_0_3  (76 chars)
- [x] test_given_default_bridges_when_loaded_then_weights_match_decision_d18  (70 chars)
- [x] test_given_approximate_bridge_when_weighted_then_half_of_type_weight  (68 chars)
- [x] test_given_more_than_ten_neighbors_when_coherence_then_top_ten_weight_over_total  (80 chars)
- [x] test_given_equal_neighbor_weights_when_coherence_then_alphabetical_tie_break  (76 chars)
- [x] test_given_skeleton_list_word_when_decided_then_esqueleto_regardless_of_coherence  (81 chars)
- [x] test_given_low_idf_and_high_coherence_when_decided_then_not_esqueleto  (69 chars)
- [x] test_given_low_idf_and_low_coherence_when_decided_then_esqueleto  (64 chars)
- [x] test_given_test_calls_symbol_exact_when_bridged_then_test_words_propagate  (73 chars)
- [x] test_given_test_calls_symbol_approx_when_bridged_then_test_words_do_not_propagate  (81 chars)
- [x] test_given_word_overlap_only_when_bridged_then_graph_unchanged_but_query_tests_lists_it  (87 chars)
- [x] test_given_ambiguous_call_and_one_imported_class_when_resolved_then_chama_approx  (80 chars)
- [x] test_given_two_imported_classes_with_method_when_resolved_then_no_edge  (70 chars)
- [x] test_given_pair_with_common_prefix_5_and_rest_3_when_reported_then_reason_prefix  (80 chars)
- [x] test_given_pair_with_jaccard_over_0_6_when_reported_then_reason_neighbors  (73 chars)
- [x] test_given_alias_report_when_run_then_nothing_is_written  (56 chars)
- [x] test_given_old_schema_version_when_opened_then_db_discarded_and_notes_kept  (74 chars)
- [x] test_given_note_on_word_turned_esqueleto_when_reindexed_then_active_without_event  (81 chars)
- [ ] test_given_word_only_in_test_docstring_when_docstring_edited_then_word_destroyed_event  (86 chars)
- [x] test_given_docstring_edit_when_reindexed_then_esqueleto_status_recomputed  (73 chars)
- [x] test_given_edge_change_when_reindexed_then_bridge_weights_follow  (64 chars)

### Testes de Integração
- [x] test_given_word_when_query_word_then_card_lists_neighbors_by_bridge_type  (72 chars)
- [x] test_given_brief_when_run_then_symbols_show_bridge_tags  (55 chars)
- [x] test_given_tests_in_results_when_brief_then_tests_listed_in_final_section  (73 chars)
- [x] test_given_small_max_chars_when_brief_then_tests_cut_first_and_omitidos_per_section  (83 chars)
- [x] test_given_no_tests_flag_when_brief_then_section_absent_and_count_in_cobertura  (78 chars)
- [x] test_given_no_tests_flag_when_query_callers_then_test_callers_hidden  (68 chars)
- [x] test_given_no_tests_flag_when_query_tests_then_usage_error_exit_1  (65 chars)
- [x] test_given_no_tests_flag_when_word_stats_read_then_same_as_without_flag  (71 chars)
- [x] test_given_no_tests_argument_when_mcp_brief_called_then_flag_passed_to_cli  (74 chars)
- [x] test_given_note_on_generic_word_when_note_list_then_marked_ancora_generica  (74 chars)
- [x] test_given_note_on_generic_word_when_status_then_marked_ancora_generica  (71 chars)
- [x] test_given_same_tree_twice_when_index_then_coherence_and_esqueleto_bytes_identical  (82 chars)
- [x] test_given_order_like_fixture_when_indexed_then_frequent_coherent_word_not_esqueleto  (84 chars)
- [x] test_given_get_like_fixture_when_indexed_then_spread_generic_word_is_esqueleto  (78 chars)
- [x] test_given_alias_report_cli_when_run_then_note_add_command_ready  (64 chars)

- [ENGINEERING] 2 testes extras da etapa 7 (aprovados com o `1` da etapa): `test_given_import_of_class_without_the_method_when_resolved_then_no_edge` e `test_given_import_removed_when_reindexed_then_import_evidence_edge_disappears`

**F.I.R.S.T Checklist**
- [x] Fast · Isolated · Repeatable · Self-validating · Timely (fixture local, SQLite temporário, git local, sem rede)
- [x] nenhum nome > 89 chars
- [ ] docstrings GIVEN/WHEN/THEN em inglês: escritas junto com cada teste na ENGINEERING, com os valores de §4 e das decisões D1 a D23
- [x] todo critério §4 tem ≥ 1 teste, exceto o de desempenho (medição) e a calibração
- [x] toda transição ONTO do estado empilhado tem ≥ 1 teste
- [x] nenhum valor concreto inventado (origem: §4 e Decisões D18, D20, D21 a D23)

---

## 8. Dependências
- tasks predecessoras: TASK-1
- módulos | serviços externos: nenhum
- flags | configurações: `memo.toml` (`[bridges]` com os pesos e `min_coherence`); flag global `--no-tests`; argumento `no_tests` no MCP

---

## 9. Observações
Esta TASK nasce de uma conversa de design com o humano. As decisões D1 a D7 abaixo já vieram respondidas.

### N_PARTITE (DEFINITION)
[N_PARTITE] modo: concept | alvo: snapshot | referência: — | perfil: stacked + full | budget: 10 | frame: base | fonte: 9763c533c5a6..9763c533c5a6 (sha256 de TASK §1–§5)

Elementos (EXTRACT.CORE, resumo): L5 repositório · contexto delimitado | L4 grafo de palavras | L2 Palavra · Ponte · Fonte (origem da ocorrência) · Ficha do conceito · Esqueleto · Alias (glossário) | L3 extração por fonte · coerência da vizinhança · decisão de esqueleto · resolução de chamada por import · ranking do `brief` · agrupamento de testes · flag `--no-tests` | L1 peso da ponte · origem · coerência · limiar.
lifecycle.onto: Palavra (nasce na primeira ocorrência em qualquer fonte, morre na última) · Ponte (derivada, recriada a cada indexação, sem identidade própria) · Esqueleto (estado derivado do corpus, pode mudar quando o corpus muda) · Fonte (segue o ciclo do arquivo, do teste ou do import que a contém) — `não definido`=0.
lifecycle.hist: não definido (snapshot de TASK, sem histórico) · evidence: não definido.

[N_PARTITE:concept] par: Palavra (L2) ↔ Ponte (L2) — acoplamento: alto | evidência: não definido | abstração: reificar a ponte como relação L3 com tipo, peso, origem e marca de aproximação | lifecycle: divergente (a palavra persiste, a ponte é recalculada) → tratar a ponte como derivada, nunca como estado
[N_PARTITE:concept] par: Ponte de chamada (L3) ↔ resolução de chamadas (L3) — acoplamento: alto | evidência: não definido | abstração: a ponte herda `approx` da aresta `CHAMA` | lifecycle: divergente (muda quando a resolução muda) → coerente, sem reorganização
[N_PARTITE:concept] par: Ponte de import (L3) ↔ aresta `IMPORTA` existente (L3) — acoplamento: alto | evidência: não definido | abstração: a ponte de import derivar da aresta `IMPORTA`, sem tabela própria | lifecycle: coerente → evitar duplicar a fonte de verdade
[N_PARTITE:concept] par: Ponte teste↔símbolo (L3) ↔ palavras de docstring (L2) — acoplamento: alto | evidência: não definido | abstração: ver STRUCT 3 | lifecycle: divergente → ciclo de reforço
[N_PARTITE:concept] par: Esqueleto (L2) ↔ Coerência (L1) — acoplamento: alto | evidência: não definido | abstração: uma única regra de decisão | lifecycle: divergente (o esqueleto muda com o corpus) → ver STRUCT 1 e SIDE 1
[N_PARTITE.STRUCT:concept] alvo: decisão de `esqueleto` — problema: duas medidas para a mesma propriedade (IDF com `min_specificity`, e coerência da vizinhança) e nada diz como se combinam | estrutura proposta: uma regra única e explícita, com a lista `skeleton` como sobrescrita | base: TASK §3 "Medida de coerência"; spec §5.6
[N_PARTITE.STRUCT:concept] alvo: ocorrência de palavra — problema: `occurrence` pertence a um símbolo, e palavras de diretório, import e docstring de teste pertencem a arquivo, import ou teste | estrutura proposta: dono explícito da ocorrência (símbolo, arquivo ou import) e campo de origem | base: TASK §3 "Fontes de palavras"; spec §4 (`occurrence`)
[N_PARTITE.STRUCT:concept] alvo: ponte teste→símbolo — problema: `query tests` liga teste e símbolo também por sobreposição de palavras, e as palavras da docstring entram nessa sobreposição, então a ponte reforça a si mesma | estrutura proposta: propagar palavras só por ponte exata (chamada `CHAMA` não aproximada) | base: TASK §3 "teste e símbolo testado"; spec §8.2 (`query tests`)
[N_PARTITE.STRUCT:concept] alvo: flag `--no-tests` — problema: a flag pode alterar só a exibição ou também as estatísticas (coerência, esqueleto), e o resultado mudaria conforme a flag | estrutura proposta: a flag só filtra a exibição; as estatísticas sempre contam o corpus inteiro | base: TASK §9 D6
[N_PARTITE.STRUCT:concept] alvo: palavras de docstring — problema: sem regra de filtro, palavras comuns de prosa (`the`, `when`, `returns`) entram como conceitos | estrutura proposta: vocabulário fechado, ou seja, só conta a palavra que já existe em algum identificador | base: TASK §3
[N_PARTITE.FAIL] ação: cálculo de coerência (L3) — eixo: falha | status: não coberto (sem ordem estável de vizinhos ou sem desempate, dois cálculos dão números diferentes) → ordenação total e arredondamento fixo
[N_PARTITE.FAIL] ação: indexação de docstring (L3) — eixo: interrupção | status: coberto por transação única de indexação (spec §3, regras de implementação)
[N_PARTITE.SIDE] ação: edição de docstring de teste (L3) — eixo: visibilidade | efeito: muda a estatística de palavras, pode virar ou desvirar `esqueleto` e alterar o `brief` de símbolos não tocados | quando: imediato | status: pergunta
[N_PARTITE.SIDE] ação: palavra que vira `esqueleto` (L3) — eixo: cascata | efeito: nota com âncora `word:` sobre essa palavra perde a âncora válida, hoje só validada no `note add` | quando: imediato | status: pergunta
[N_PARTITE.GAP] item: slot das palavras de docstring, import e diretório — status: ausente-não-decidido | base: spec §5.2 passo 3 classifica ação, objeto e papel só pelo vocabulário de nomes → decidir se esses slots valem para as novas fontes
[N_PARTITE.GAP] item: completeness — status: não definido (sem referência) | base: perfil full sem referência → —
invariantes: nenhum declarado em §5; sem código no ALVO; c_código não calculável → `[N_PARTITE.INV?]` não emitido.
[N_PARTITE.RANK] 1 · STRUCT ocorrência sem dono (modelo) · impacto: alto · alcance: não definido · custo: alto · 80/20
[N_PARTITE.RANK] 2 · STRUCT ciclo teste↔palavras de docstring · impacto: alto · alcance: não definido · custo: médio · 80/20
[N_PARTITE.RANK] 3 · STRUCT duas medidas de esqueleto · impacto: alto · alcance: não definido · custo: médio · 80/20
[N_PARTITE.RANK] 4 · STRUCT `--no-tests` e estatísticas · impacto: médio · alcance: não definido · custo: baixo · —
[N_PARTITE.RANK] 5 · SIDE esqueleto × notas `word:` · impacto: médio · alcance: não definido · custo: médio · —
[N_PARTITE.RANK] 6 · GAP slot das novas fontes · impacto: médio · alcance: não definido · custo: baixo · —
[N_PARTITE.STACK] decisão: base | frames: 1 | fonte: 9763c533c5a6..9763c533c5a6 | recalculados: 0 | herdados: 0 | resolvidos: 0 | revalidar: 0

#### Delta rodada 1
[N_PARTITE] modo: concept | alvo: delta | referência: — | perfil: stacked + full | budget: 10 | frame: delta | fonte: D9..D12 (decisões da rodada 1)
[N_PARTITE:concept] par: ocorrência de palavra (L2) ↔ dono: símbolo, arquivo ou import (L2) — acoplamento: alto | evidência: não definido | abstração: `owner_kind` e `source` na própria `occurrence` | lifecycle: coerente (a ocorrência morre com o dono) → resolvido (STRUCT 2)
[N_PARTITE:concept] par: ponte teste→símbolo (L3) ↔ resolução de chamadas (L3) — acoplamento: alto | evidência: não definido | abstração: palavras só se propagam por `CHAMA` exata | lifecycle: coerente → ciclo cortado (STRUCT 3)
[N_PARTITE:concept] par: esqueleto (L2) ↔ coerência (L1) — acoplamento: alto | evidência: não definido | abstração: regra única `skeleton OU (IDF baixo E coerência baixa)` | lifecycle: divergente (o status muda com o corpus) → resolvido (STRUCT 1); resta SIDE esqueleto × notas
[N_PARTITE.STRUCT:concept] alvo: `--no-tests` — status: resolvido (D12)
[N_PARTITE.STACK] decisão: push delta | frames: 2 | fonte: D9..D12 | recalculados: 3 | herdados: 6 | resolvidos: 4 | revalidar: 0

#### Delta rodada 2
[N_PARTITE] modo: concept | alvo: delta | referência: — | perfil: stacked + full | budget: 10 | frame: delta | fonte: D13..D20 (decisões da rodada 2)
[N_PARTITE:concept] par: palavra de docstring (L2) ↔ vocabulário de identificadores (L4) — acoplamento: alto | evidência: não definido | abstração: vocabulário fechado | lifecycle: coerente (a palavra de docstring existe só enquanto existir como identificador) → resolvido (STRUCT 5)
[N_PARTITE:concept] par: slot (L1) ↔ fonte da palavra (L1) — acoplamento: baixo | evidência: não definido | abstração: mesmo vocabulário, slot `context` para o segmento de contexto | lifecycle: coerente → resolvido (GAP slot)
[N_PARTITE:concept] par: nota `word:` (L2) ↔ esqueleto (L2) — acoplamento: médio | evidência: não definido | abstração: nota ativa com marca `âncora genérica`, sem transição | lifecycle: divergente → aceito (SIDE esqueleto × notas)
[N_PARTITE:concept] par: ponte (L3) ↔ tabelas `occurrence`, `edge`, `file` (L4) — acoplamento: alto | evidência: não definido | abstração: ponte derivada, sem tabela | lifecycle: coerente (recalcula junto com as fontes) → resolvido
[N_PARTITE.STRUCT:concept] alvo: coerência — status: resolvido (D20, fórmula única e determinística)
[N_PARTITE.FAIL] ação: cálculo de coerência (L3) — eixo: falha | status: coberto por ordenação total (peso decrescente, nome crescente) e arredondamento fixo, fixados em D20 e no critério de determinismo
[N_PARTITE.SIDE] ação: edição de docstring de teste (L3) — eixo: visibilidade | efeito: pode mudar o status de esqueleto de palavras e o `brief` de símbolos não tocados | quando: imediato | status: informativo (efeito aceito: a vizinhança mede o corpus; D15 protege as notas)
[N_PARTITE.GAP] item: calibração de `min_coherence` e dos pesos — status: ausente-intencional | base: D18 e D20 são valores iniciais e a TASK §5 registra a calibração como do humano
[N_PARTITE.STACK] decisão: push delta | frames: 3 | fonte: D13..D20 | recalculados: 4 | herdados: 8 | resolvidos: 8 | revalidar: 0

### Decisões (grill-me)
Origem: humano, 2026-11-07, resposta à discussão de design.
- D1: o import conta pelo nome importado e pelo caminho do módulo. (humano)
- D2: pesos das pontes na ordem: mesmo símbolo > teste e símbolo testado > import > chamada > mesmo arquivo > mesmo contexto; valores numéricos a definir. (humano, leitura confirmada em D8)
- D3: imports externos entram com relevância menor, não zero. (humano)
- D4: a evidência de import para resolver chamadas ambíguas entra nesta TASK. (humano, leitura confirmada em D8)
- D5: `query word` devolve a ficha do conceito e `brief` usa a mesma vizinhança só para ranquear, com a etiqueta das pontes por símbolo. (humano, recomendação aceita)
- D6: flag global `--no-tests` (padrão falso) nas consultas; em `query tests` dá erro de uso. (humano)
- D7: o `brief` agrupa os testes numa seção própria ao final, mesmo sem a flag; o corte de `--max-chars` tira primeiro os testes; o agrupamento vale só para o `brief`. (humano)
- D8: as leituras marcadas com ⚠ nas respostas de D2 e D4 foram aceitas com o `1` que abriu esta TASK. (humano)
- D9: [DEFINITION] `esqueleto` = palavra na lista `skeleton` OU (IDF baixo E coerência baixa); limiar `min_coherence` em `memo.toml`, calibrado no repositório real. (humano, recomendação aceita)
- D10: [DEFINITION] `occurrence` ganha `owner_kind` (symbol, file, import) e `source` (name, path, import, test_doc); uma tabela só. (humano, recomendação aceita)
- D11: [DEFINITION] palavras de um teste só se propagam ao símbolo testado por chamada exata (`CHAMA` não aproximada); a sobreposição de palavras fica só em `query tests`. (humano, recomendação aceita)
- D12: [DEFINITION] `--no-tests` só filtra a saída; as estatísticas contam o repositório inteiro; a cobertura informa `N testes ocultos`. (humano, recomendação aceita)
- D13: [DEFINITION] vocabulário fechado nas docstrings de teste: a palavra só conta se existe como palavra de algum identificador; `given`, `when` e `then` são marcadores de segmento. (humano, recomendação aceita)
- D14: [DEFINITION] o vocabulário (`verbs`, `roles`) decide o slot das palavras de diretório, import e docstring; o segmento de contexto (após `app/`) ganha o slot `context`. (humano, recomendação aceita)
- D15: [DEFINITION] nota cuja palavra de âncora virou `esqueleto` continua ativa, sem evento, e é marcada `âncora genérica` em `note list` e `status`. (humano, recomendação aceita)
- D16: [DEFINITION] docstrings de produção ficam para uma segunda fase; `source` já prevê o valor `doc`. (humano, recomendação aceita)
- D17: [DEFINITION] relatório de aliases sugeridos, somente leitura, entra nesta TASK; nada é gravado sem o humano. (humano, recomendação aceita)
- D18: [DEFINITION] pesos iniciais: mesmo símbolo 1,0 · teste e símbolo testado 0,8 · import 0,6 (externo 0,3) · chamada 0,5 · arquivo 0,3 · contexto 0,1; ponte aproximada vale metade; tudo editável em `memo.toml`. (humano, recomendação aceita)
- D19: [DEFINITION] as pontes são derivadas (consultas sobre `occurrence`, `edge` e `file`), sem tabela própria; materializar só se a medição de desempenho exigir. (humano, recomendação aceita)
- D20: [DEFINITION] coerência = peso das 10 vizinhas mais fortes (não esqueleto, desempate alfabético) / peso total das pontes da palavra; `min_coherence` inicial 0,30. (humano, recomendação aceita)
- D21: [TEST_ANALYSIS] relatório de aliases: par sugerido quando (a) prefixo comum de pelo menos 5 caracteres e resto de até 3, ou (b) Jaccard das vizinhas ≥ 0,6 com pelo menos 3 vizinhas em cada palavra; ordem: motivo (a antes de b), semelhança decrescente, alfabética. (humano, recomendação aceita; refina o critério 14)
- D22: [TEST_ANALYSIS] palavras do caminho de um import: o primeiro segmento (raiz do pacote) e os segmentos de camada configurados não viram palavra; o segmento de contexto vira palavra com slot `context`; os demais viram palavras comuns de origem `import`; alvo externo usa todos os segmentos, com peso 0,3. (humano, recomendação aceita; refina D1)
- D23: [TEST_ANALYSIS] `context_root` aceita um prefixo de caminho (`backend/easy_lease`); o contexto é o segmento seguinte e, para imports, a raiz do pacote é o último segmento do prefixo; o padrão `app` não muda. (humano, recomendação aceita; refina D14 e o critério de contexto)

### Premissas
- [DEFINITION] A numeração D1 a D20 é local a esta TASK.
- [DEFINITION] A evidência de import (D4) vale quando o arquivo importa uma única classe com o método ambíguo; com duas ou mais candidatas importadas, nenhuma aresta é criada.
- [DEFINITION] O relatório de aliases é o comando `memo query aliases` (nome provisório).
- [DEFINITION] Uma ponte aproximada herda `~` da aresta de origem e pesa metade.

### Plano de Execução
#### Etapas
1. Esquema v2: `occurrence.owner_kind` e `source`, slot `context`, `schema_version` 2 com recriação do banco derivado — depende de: NONE
2. Fontes novas de palavras: diretório (contexto), import (nome e caminho; externo com peso menor) e docstring de teste (vocabulário fechado, segmentos) — depende de: 1
3. Pontes derivadas e pesos: `[bridges]` em `memo.toml`, aproximada vale metade — depende de: 2
4. Coerência (top-10) e nova regra de `esqueleto` (`min_coherence`); propagação de teste só por `CHAMA` exata — depende de: 3
5. Ficha do conceito em `query word`; `brief` com etiquetas de pontes e seção de testes ao final — depende de: 3, 4
6. Flag `--no-tests` (CLI e `no_tests` no MCP) e `N testes ocultos` na cobertura — depende de: 5
7. Evidência de import na resolução de chamadas ambíguas — depende de: 2
8. Marca `âncora genérica` em `note list` e `status` — depende de: 4
9. Relatório de aliases sugeridos (`memo query aliases`) — depende de: 3
10. Medições de desempenho, atualização da spec (§4, §5.2, §5.6, §8) e do README — depende de: 1 a 9
#### Gates
- após cada etapa: testes da etapa passando e `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` limpos
- após a etapa 4: determinismo byte a byte conferido em duas indexações seguidas
- após a etapa 10: metas de desempenho medidas e registradas (não bloqueiam o aceite)
#### Riscos
- o grafo de palavras deixa a indexação lenta → pontes derivadas por consulta; materializar em cache só se a meta de 60 s for violada
- a coerência não separa conceito de esqueleto no repositório real → `skeleton` como sobrescrita e `min_coherence` editável; calibração do humano
- ruído de imports externos (`typing`, `uuid`) → peso metade e vocabulário só de identificadores
- oscilação de `esqueleto` com o corpus → nota ativa com marca (D15), sem evento
- mudança de esquema recria bancos derivados → notas ficam intactas; a primeira indexação depois do upgrade é completa
#### Assunções
- o banco derivado é descartável, então a mudança de esquema recria em vez de migrar
- a calibração de `min_coherence`, `min_specificity` e dos pesos no repositório real é do humano e não bloqueia o aceite
- precisa existir uma fixture com palavra frequente e coerente (tipo `order`) e palavra genérica espalhada (tipo `get`), para testar a regra de `esqueleto`

### N_PARTITE (CODE_REVIEW)
[N_PARTITE] modo: multi (concept×code) | alvo: delta | referência: — | perfil: stacked + full | budget: 10 | frame: base | fonte: 94dd5e5..dfc4077
- REUSE: miss (sem frame anterior de CODE_REVIEW); frame base.
[N_PARTITE:code] par: Corpus (L2) ↔ word_bridges (L3) — acoplamento: alto | evidência: não definido | abstração: manter `Corpus` como struct privada de carga | lifecycle: coerente (nasce e morre dentro da chamada) → nenhuma
[N_PARTITE:code] par: word_core (L3) ↔ WordInfo (L2) — acoplamento: alto | evidência: não definido | abstração: separar `WordPattern` de `WordInfo` | lifecycle: divergente (o padrão é leitura curta, a ficha carrega pontes e símbolos) → tipo próprio (A2)
[N_PARTITE:code] par: brief.rs (L4) ↔ query.rs (L4) — acoplamento: baixo | evidência: não definido | abstração: `brief` só depende de `word_pattern` | lifecycle: coerente → nenhuma
[N_PARTITE.STRUCT:code] alvo: word_core (query.rs) — problema: `Option<&BridgeWeights>` seleciona dois modos de saída | estrutura proposta: duas funções, uma por tipo de saída | base: `word_core(store, word, weights: Option<&BridgeWeights>)`
[N_PARTITE.DRIFT] par: D19 (concept) ↔ bridges.rs Corpus (code) — situação: sem drift (pontes seguem derivadas, sem tabela) → nenhuma
[N_PARTITE.FAIL] elemento: load_corpus — falha: banco mudar durante a leitura (reindex concorrente) | proteção: lock por DB já existente (30 s) | lacuna: não verificada nesta revisão
[N_PARTITE.GAP] completeness: não definido (sem referência)
[N_PARTITE.CLUSTER] bridges.rs, query.rs e brief.rs mudam juntos nos commits das etapas 3, 5 e 10 → cluster de pontes; `stats.rs` e `aliases.rs` usam `core_neighborhoods` (versão em lote) → dois cálculos de vizinhança (por palavra e em lote) sem teste que cruze os dois
[N_PARTITE.INV?] invariante inferido: `word_pattern` e `word_info` concordam em verbos, papéis e camadas | escritor: `word_core` (único) | guarda: por construção | nível: ⚠ inferido, sem teste (A2)
[N_PARTITE.SIDE] nenhum efeito colateral novo (somente leitura) — verificado em: dfc4077
[N_PARTITE.RANK] 1) A2 tipo e teste de `word_pattern` · 2) vizinhança por palavra × em lote sem teste de paridade · 3) A1 checkboxes do §4 · 4) A3 e A4

### Code Review
#### [2026-11-08T15:00-03:00] — dfc4077b
- WT: /var/home/erickod/Documentos/dev/memmo (branch master; sem PR, revisão local; `.agents`, `.mcp.json` e `CLAUDE.md` não rastreados, gerados pelo `memo init`)
- lang sem reference: rust (só CRITERIA base; nenhuma regra específica de linguagem inventada)
- flags: ddd, perf_deep e refactor_only não ativadas na TASK (§9/Plano/Assunções); não inferidas.

**SUMMARY — Aprovado c/ ressalvas**
- Positivos: a correção de desempenho preserva a semântica (consultas antigas e código novo conferidos lado a lado: pontes de arquivo, contexto e import equivalentes); D19 mantida (sem tabela); 257 testes passam em duas execuções, `fmt` e `clippy -D warnings` limpos; metas medidas (índice 43 s, sem mudança 0,04 s, `brief` 0,36 s).
- Preocupações: (1) checkboxes do §4 desatualizados; (2) `word_pattern` devolve `WordInfo` com campos vazios; (3) `query word` ainda custa 1–2 s em palavra muito frequente; (4) só o commit final foi relido a fundo; os 24 commits anteriores da TASK foram conferidos por testes, goldens e leitura durante a implementação, não relidos linha a linha agora.

**ANALYSIS**
- [ ] design: `Corpus` e `load_corpus` isolam a carga; `word_core` com `Option<&BridgeWeights>` mistura dois modos de saída (query.rs, `word_core`) — achado A2.
- [ ] DDD: não identificado — verificado em: TASK §9/Plano (flag ddd não ativada; não inferida).
- [ ] reliability: não identificado — verificado em: bridges.rs `load_corpus` e `import_bridges` (só leitura, sem retry nem escrita em 2 sistemas); `texts[&id]` só indexa ids vindos da própria tabela `word`.
- [ ] data: não identificado — verificado em: diff dfc4077 (sem valor monetário, PII ou dado financeiro).
- [ ] contract: `word_info` mantém assinatura e saída; `word_pattern` é nova; consumidores por `grep -w`: `memo-cli/src/main.rs:378` (`word_info`) e `brief.rs:433` (`word_pattern`); nada quebrado.
- [ ] env_k8s: não identificado — verificado em: diff sem settings nem manifest.
- [ ] concurrency: não identificado — verificado em: diff sem worker, consumer nem entrypoint.
- [ ] migrations: não identificado — verificado em: diff sem migration (a mudança de esquema da TASK é do commit 4b542f0, aprovada no §6).
- [ ] perf: `load_corpus` carrega o corpus inteiro por chamada de `word_bridges` (O(ocorrências) por consulta; ok para `query word`); `tagged_symbols` faz 6 consultas por símbolo (query.rs:326-340), 1–2 s em palavra com 9.000 usos — achado A3; sem N+1 nas pontes nem paginação nova.
- [ ] security: não identificado — verificado em: SQL do diff é texto fixo, sem entrada do usuário concatenada.
- [ ] errors: não identificado — verificado em: `?` com `CoreError` em todas as consultas novas; sem `unwrap`.
- [ ] readability: `files_with_word` guarda contextos, não arquivos (bridges.rs, `file_and_context_bridges`) — achado A4; o `HashSet seen` em `tagged_symbols` não mudou o tempo medido (2,2 s antes e depois), só tirou um laço quadrático.
- [ ] layers: não identificado — verificado em: imports do diff (`memo-core` sem `clap` nem MCP).
- [ ] tests: goldens de `query word` e `brief` sem mudança cobrem a equivalência; falta teste que trave a paridade de `word_pattern` com `word_info` (verbos, papéis, camadas) — achado A2.
- [ ] task: critérios §4 1–3 estão implementados (testes de `sources` e `schema` passam) mas seguem `[ ]`; o de desempenho e o de `fmt/clippy/test` também, embora medidos e verdes — achado A1; D19 verificada por ausência de tabela de pontes em `store.rs`; D20 por `coherence.rs`.
- [ ] format: não identificado — verificado em: `cargo fmt --check` OK e `cargo clippy -D warnings` sem avisos.
- [ ] implicit: `file_and_context_bridges` depende de `file.context` nulo para arquivo fora de contexto (filtro `IS NOT NULL` na carga); sem teste dedicado, coberto pelo golden da fixture; não identificado como defeito.
- [ ] refactor_estrutural: SQL trocado por mapas em memória preserva a semântica — verificado em: contagem por arquivo e por contexto distintos, exclusão da própria palavra, prefixo de `qname` no limite do ponto (conjunto `prefixes` ≡ `EXISTS(qname = alvo OR qname LIKE alvo.%)`).
- [ ] smells¹: Long Method — `load_corpus` ~45 linhas, aceitável; Long Params — `import_bridges` com 5; Feature Envy, Msg Chains, Magic Numbers, Ignored Exceptions, Duplicate Code, Dead Code — não identificado — verificado em: diff dfc4077.
- [ ] N_PARTITE: ver §9/N_PARTITE (CODE_REVIEW); perfil `stacked + full`, REUSE miss, frame base.
- [ ] drift (DEFINITION × código): base e deltas do §9/N_PARTITE (DEFINITION) comparados no mesmo frame — DRIFT=0 para pontes, coerência, esqueleto, nota e `--no-tests`; nenhum DRIFT-n aberto.

**VERIFY** — reli o diff dfc4077 sem olhar a ANALYSIS: nenhuma omissão além de A1–A4.

**ACTIONS**
- 🟡 A1 — TASK §4: marcar `[x]` nos critérios 1–3, no de desempenho (com as medidas) e no de `fmt/clippy/test`, que estão verdes. Não é código; entra no GATE.OUT da ENGINEERING.
- 🟡 A2 — `word_pattern` devolve `WordInfo` com `symbols`, `bridges` e `stats` vazios (query.rs, `word_core`): quem chamar sem ler o comentário recebe uma ficha incompleta. Correção: tipo próprio `WordPattern` com verbos, papéis e camadas, mais um teste que compare esses três campos com `word_info` na fixture. before: `Result<WordInfo,_>`; after: `Result<WordPattern,_>`.
- 🟢 A3 — `tagged_symbols`: trocar as 6 consultas por símbolo por consultas em lote por palavra, se `query word` em palavra frequente incomodar; sem meta de spec, medido 1,1–2,2 s.
- 🟢 A4 — renomear `files_with_word` para `contexts_with_word`.
- ⚖️ A5 — `load_corpus` a cada `query word`: (a) manter, simples, custo O(corpus) por consulta; (b) cachear no processo (o `serve`/MCP faz várias consultas); (c) materializar pontes em tabela (contraria D19, decisão do humano). Recomendação: (a) agora; (b) só se o `serve` mostrar lentidão.
- ❓ nenhuma pergunta ao autor com 🔴 potencial.

**COMMITS**
- ✓ [dfc4077](https://github.com/erickod/memo/commit/dfc4077b9a6c4cd5f235392afe67386b8fd5080c) perf: compute import, file and context bridges in memory and keep brief light — sem teste novo em red (declarado no log; refatoração coberta pelos goldens); a mensagem leva `Co-Authored-By` por instrução do sistema, contra a regra de commit sem atribuição da TASK (ressalva de processo, não de código).
- ⚠ 3b6c54f..94dd5e5 (24 commits da TASK-2, testes red e feat green, mais docs): ✓ por testes verdes e goldens; não relidos linha a linha nesta review.

**EVIDÊNCIAS**
- SHA revisado: dfc4077b9a6c4cd5f235392afe67386b8fd5080c (HEAD); sem PR, então `headRefOid` não se aplica.
- Comandos: `cargo fmt --check` OK; `cargo clippy --all-targets -- -D warnings` sem avisos; `cargo test` duas vezes: 257 passam, 0 falham; repositório sintético de 5.000 arquivos: índice 43 s, sem mudança 0,04 s, `brief` 0,36 s, `query word total` 1,1 s, `query word limit` 2,2 s.
- Consumidores: `grep -w word_info|word_pattern` em `crates/` (2 chamadores); vazio não prova ausência, então li `word_core` e `word_bridges` direto. Semble não foi consultado nesta review.
- Não executado: revisão em PR (não há); releitura linha a linha dos 24 commits anteriores; teste em repositório real; clientes MCP e harness reais.


## 10. Log de Decisões e Alterações
append-only · entrada ✗ alterada | removida (REF: agentsmd#LOG)

### [2026-11-07T21:15-03:00] — Criação da TASK-2
- Tipo: decisão
- Contexto: o humano respondeu `1` para abrir a TASK-2 com as decisões 1 a 7 resolvidas; `issue_key=-`; spec §5.2 e §5.6 serão revistas na consolidação.
- Decisão tomada: DEST criado a partir do rascunho (A.1 e A.2); próximo passo, modelagem n-partite e grill-me sobre os pontos abertos.
- Impacto: STATE=DEFINITION, `Estado: draft`.
- Critérios afetados: §4 ainda sem critérios

### [2026-11-07T21:25-03:00] — n-partite executada
- Tipo: validação
- Contexto: A.3 da DEFINITION (≥ 2 conceitos).
- Decisão tomada: `n-partite · snapshot · full · DEFINITION` (stacked, frame base, fonte 9763c533c5a6); 3 STRUCT de impacto alto viram sementes do GRILL, mais 2 STRUCT, 2 SIDE e 1 GAP.
- Impacto: sementes do grill-me: decisão de esqueleto, dono da ocorrência, ciclo teste↔docstring, `--no-tests` × estatísticas, vocabulário de docstring, esqueleto × notas, slot das novas fontes.
- Critérios afetados: nenhum ainda

### [2026-11-07T21:40-03:00] — Rodada 1 do grill-me
- Tipo: decisão
- Contexto: `grill-me@DEFINITION`, rodada 1, 4 perguntas (esqueleto, dono da ocorrência, ciclo teste↔docstring, `--no-tests`); todas com a recomendação aceita.
- Decisão tomada: D9 a D12 gravadas em §9; `n-partite · delta · full · DEFINITION · rodada 1`, 4 itens resolvidos.
- Impacto: sementes restantes: esqueleto × notas `word:`, slot das novas fontes, vocabulário fechado de docstring, docstrings de produção, aliases sugeridos, pesos numéricos, ponte de import × `IMPORTA`.
- Critérios afetados: nenhum ainda

### [2026-11-07T22:30-03:00] — Rodada 2 do grill-me e consolidação
- Tipo: decisão
- Contexto: `grill-me@DEFINITION`, rodada 2, 8 perguntas (vocabulário de docstring, slot, nota × esqueleto, docstring de produção, aliases, pesos, ponte derivada, fórmula da coerência); todas com a recomendação aceita.
- Decisão tomada: D13 a D20 gravadas em §9; `n-partite · delta · full · DEFINITION · rodada 2`; A.5: §1, §3, §4 (19 critérios), §5, §6, §8 e o Plano de Execução preenchidos a partir das decisões D1 a D20. `grill-me · diferida · — · DEFINITION · 2 rodadas · 20 decisões · 0 pendências · STATUS:READY`.
- Impacto: aguarda a aprovação única do §D. A mudança de esquema (§6) precisa de aprovação explícita.
- Critérios afetados: §4 (19 critérios novos)

### [2026-11-07T22:45-03:00] — Aprovação única do §D e início da TEST_ANALYSIS
- Tipo: validação
- Contexto: o humano respondeu `1` ao resumo do §D (qualificação, modelo, decisões D1 a D20, plano e mudança de esquema derivado, §6).
- Decisão tomada: DEFINITION aprovada; STATE → TEST_ANALYSIS (`Estado: in-progress`). Lacunas de valor esperado encontradas ao derivar os cenários: limiares do relatório de aliases e quais segmentos do caminho de import viram palavra; seguem para `grill-me@TEST_ANALYSIS`.
- Impacto: ROLE test-analyst assume a TASK-2.
- Critérios afetados: nenhum

### [2026-11-07T23:10-03:00] — TEST_ANALYSIS concluída
- Tipo: validação
- Contexto: cenários derivados dos 19 critérios de §4 e do lifecycle do estado empilhado; 3 lacunas de valor esperado (aliases, caminho de import, raiz de contexto).
- Decisão tomada: `grill-me · diferida · — · TEST_ANALYSIS · 1 rodada · 3 decisões · STATUS:READY`; as 3 respostas refinam critérios existentes (D21, D22, D23) e não os alteram, então não há retorno à DEFINITION; 45 testes definidos em §7 (30 de unidade e 15 de integração, nomes ≤ 89 caracteres). Desempenho e calibração ficam como medição, registrados como não cobertos por teste.
- Impacto: STATE → ENGINEERING.
- Critérios afetados: nenhum alterado; critérios 4, 12, 14 e de contexto refinados por D21 a D23

### [2026-11-07T23:50-03:00] — Etapa 1: esquema v2
- Tipo: alteração
- Contexto: alteração atômica 1 aprovada (`1`); commits `3b6c54f` (test, red) e `feat: add occurrence owner and source columns with schema version 2` (green).
- Decisão tomada: `occurrence` ganha `owner_kind` (padrão `symbol`), `owner_id` e `source` (padrão `name`); a chave primária passa a `(owner_kind, owner_id, pos)` e `symbol_id` fica opcional, preenchido só quando o dono é um símbolo (assim todas as consultas atuais, que juntam por `symbol_id`, não mudam de resultado); índice novo `occurrence(symbol_id)`; `schema_version` 2: ao abrir, um banco com outra versão (ou sem `meta`) é descartado (tabelas apagadas com `foreign_keys` desligado) e recriado, e as notas `.md` não são tocadas.
- Ajuste: a primeira versão da recriação falhou com `FOREIGN KEY constraint failed` (o `rusqlite` embutido liga as chaves estrangeiras por padrão); corrigido desligando-as durante a troca.
- Fora da etapa: o slot `context` (é só um valor de texto da coluna `slot`, vem com a extração na etapa 2).
- Impacto: 212 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos; nenhum golden mudou. O primeiro comando do `memo` depois do upgrade reindexa tudo.
- Critérios afetados: critérios 1 (parcial: colunas) e 15 (esquema v2)

### [2026-11-08T01:30-03:00] — Etapa 2: fontes novas de palavras
- Tipo: alteração
- Contexto: alteração atômica 2 aprovada (`1`); commits `650b721` (test, red) e `feat: index path, import and test docstring words with owner and source` (green).
- Decisão tomada: `sources.rs` (funções puras) gera as palavras de diretório, de import e de docstring de teste: diretório e import seguem D14, D22 e D23 (`context_root` como prefixo, raiz e camadas fora, contexto com slot `context`, alvo que não começa pela raiz do pacote usa todos os segmentos); docstring segue D13 (`GIVEN`, `WHEN` e `THEN` viram segmento, e só entra palavra que já existe em nome, caminho ou import); `Slot::Context` novo; `FileFacts.docs` guarda a docstring das funções; a tabela `doc(symbol_id, text)` guarda o texto das docstrings de teste para as palavras serem refeitas quando o vocabulário muda; palavras de import e de docstring são reconstruídas a cada indexação que escreve (dependem de quais alvos e palavras existem), as de caminho na entrada de cada arquivo; a chave primária de `occurrence` ganhou `source` (`owner_kind, owner_id, source, pos`), ainda dentro da `schema_version` 2 (nenhum banco real nesta versão ainda); o dump canônico inclui as ocorrências não-nome, então o determinismo as cobre.
- Decisão de escopo: as palavras novas têm `symbol_id` nulo, então as estatísticas, o `brief` e o `query word` não as veem; o `brief` só casa palavras que algum símbolo usa. Elas aparecem só nos eventos de palavra e nas âncoras `word:`. Passam a valer nas etapas 3 a 5.
- Ajuste de teste: `test_given_symbol_word_when_stored_...` (etapa 1) agora filtra as ocorrências de nome (`source = 'name'`), porque o seu enunciado original valia só antes de existirem outras fontes.
- Problema encontrado em §7 (TEST_ANALYSIS): o teste `test_given_word_only_in_test_docstring_when_docstring_edited_then_word_destroyed_event` descreve uma palavra que existe só na docstring, o que o vocabulário fechado (D13) impede por construção. Não o implementei; precisa voltar à TEST_ANALYSIS para trocar o cenário (por exemplo, palavra que só existe num import ou num diretório). Segue pendente.
- Observação: o teste `..._root_and_layer_segments_dropped` passou na fase vermelha de forma vazia (sem palavras de import ainda); o teste de import positivo cobre o caso.
- Impacto: 220 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos; nenhum golden mudou. Desempenho não remedido.
- Critérios afetados: 1 a 4 (fontes, vocabulário fechado, slot `context`, caminho do import); falta o peso do externo (etapa 3)

### [2026-11-08T03:10-03:00] — Etapa 3: pontes derivadas e pesos
- Tipo: alteração
- Contexto: alteração atômica 3 aprovada (`1`); commits `cff2678` (test, red) e `feat: derive word bridges and add bridge weights to memo.toml` (green).
- Decisão tomada: `[bridges]` em `memo.toml` com os valores da D18 (`symbol` 1,0 · `test` 0,8 · `import` 0,6 · `import_external` 0,3 · `call` 0,5 · `file` 0,3 · `context` 0,1 · `approximate` 0,5), validados (finitos, não negativos) e gravados pelo `init`; `bridges::word_bridges(store, palavra, pesos)` devolve as vizinhas por tipo de ponte (símbolo, teste, import, chamada, arquivo, contexto), sem tabela nova (D19); ponte de teste só por chamada exata; chamada de teste aproximada não gera nenhuma ponte (nem de chamada); import interno pesa `import`, externo `import_external`; a ponte de chamada aproximada pesa metade.
- Decisões de detalhe minhas: `CRIA` exata conta como chamada (o teste que constrói a classe leva as palavras a ela); o lado "arquivo" de uma ponte de import usa as palavras do módulo e do caminho do arquivo; as pontes de arquivo e contexto só são calculadas por palavra (a versão em lote, para a coerência, será feita na etapa 4 apenas com as pontes de símbolo, teste, import e chamada, porque as de arquivo e contexto formariam pares demais).
- Impacto: 227 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos; nenhum golden mudou (as pontes ainda não aparecem em nenhuma saída). Desempenho não remedido.
- Limite: o peso do arquivo e do contexto soma por ocorrência em arquivo e em contexto; o slot `context` das palavras de caminho ainda não gera ponte de contexto próprio.
- Critérios afetados: 4 (peso do externo), 5 (pesos), 8 (propagação de teste só por chamada exata)

### [2026-11-08T05:20-03:00] — Etapa 4: coerência e regra de esqueleto
- Tipo: alteração
- Contexto: alteração atômica 4 aprovada (`1`); commits `b110cc4` (test, red) e `feat: compute word coherence and decide esqueleto by idf and coherence` (green).
- Decisão tomada: `bridges::core_neighborhoods` calcula em lote a vizinhança de todas as palavras pelas pontes de símbolo, teste, import e chamada (as mesmas regras de `word_bridges`, soma em ordem estável); `stats::coherence` = peso das 10 vizinhas mais fortes / peso total (peso decrescente, nome crescente), 0 sem vizinhas; `word_stats.coerencia` nova coluna; `esqueleto` = palavra em `skeleton` OU (especificidade < `min_specificity` E coerência < `min_coherence`); `min_coherence` (padrão 0,30) em `memo.toml`, validado de 0 a 1, e gravado pelo `init`; o dump canônico traz a coerência.
- Esclarecimento de D20, forçado por um teste: a D20 diz "vizinhas não esqueleto". Na primeira versão excluí também as vizinhas de especificidade baixa, e o teste da palavra `order` deu coerência 0: os vizinhos de um conceito (`placed`, `canceled`, ...) também são frequentes e sumiam. Passei a excluir só as vizinhas da lista `skeleton`, que é a única exclusão sem circularidade (o status final depende da própria coerência). Isso é uma interpretação minha de D20; registro para você confirmar ou reverter.
- Consequência a notar: palavra com 10 vizinhas ou menos tem coerência 1,0, então nunca é esqueleto pela regra nova (só pela lista). Em repositórios pequenos quase nada vira esqueleto por IDF.
- Ajustes de teste: `test_given_low_specificity_when_stats_then_esqueleto_by_threshold` (TASK-1) foi renomeado `test_given_low_specificity_but_coherent_when_stats_then_not_esqueleto` e passou a afirmar a regra nova; dois golden de `query word` mudaram como previsto (na fixture, `limit` deixa de ser esqueleto e `usecase` volta a `papéis`), revisados por leitura. Docstring de teste agora entra na vizinhança só pela ponte de teste exata.
- Impacto: 236 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos; desempenho não remedido (cada indexação que escreve agora calcula a vizinhança em lote).
- Critérios afetados: 6 (coerência), 7 (esqueleto), 16 (determinismo)

### [2026-11-08T07:00-03:00] — Etapa 5: ficha do conceito e brief com pontes
- Tipo: alteração
- Contexto: alteração atômica 5 aprovada (`1`, com a D20 aceita); commits `fc19dd1` (test, red) e `feat: show word card by bridge type and group tests at the end of brief` (green).
- Decisão tomada: `query word` devolve a ficha (estatísticas com `coer=`, verbos, papéis, camadas, `vizinhas por ponte` com as 8 mais pesadas de cada tipo e seus pesos, e os símbolos com as pontes que os trazem); `tags.rs` define as etiquetas `nome`, `docstring`, `teste`, `chamada`, `tipo`, `import` (e `vizinho` quando o símbolo só entrou pela expansão de um salto); a lista de símbolos da ficha inclui, além dos que têm a palavra no nome, a docstring de teste com a palavra, o símbolo testado por teste com ela e o módulo cujo arquivo a importa; `brief_weighted` usa os pesos do `memo.toml`: docstring de teste soma o peso de teste e import soma o peso de import com fator 0,2 (sinal fraco, igual ao bônus de papel), só para ranquear.
- Decisões de detalhe minhas: a seleção do `brief` fica com até 15 símbolos de produção mais até 5 testes (os testes deixam de disputar as vagas do código); "teste" inclui o módulo de um arquivo de teste (`mod test_...`), não só as funções; o contorno só descreve símbolos de produção e resume os testes que chamam como `<- +N testes`; sob `--max-chars` os testes saem primeiro, o marcador `+K testes omitidos` fica na seção, e o `+N omitidos` final soma as duas seções; o `--limit` só corta a parte principal.
- Ajustes de teste: o teste do `brief` (`five_anchors_and_fifteen_symbols_at_most`, TASK-1) agora conta 15 de produção e 5 testes, cada grupo em ordem de score; o teste de `--max-chars` do `query word` subiu de 600 para 1100, porque a ficha tem um cabeçalho maior que o limite antigo (o cabeçalho não é cortado); 3 golden mudaram (`query word` e `brief`) e 1 golden novo (a ficha), todos revisados por leitura.
- Impacto: 240 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos; desempenho não remedido.
- Limite: a ficha mostra todas as palavras vizinhas, mas só as palavras de símbolos entram nas estatísticas (as de caminho e import só como vizinhas por ponte); `--no-tests` ainda não existe (etapa 6).
- Critérios afetados: 9 (ficha), 10 (brief com etiquetas e seção de testes)

### [2026-11-08T08:30-03:00] — Etapa 6: --no-tests
- Tipo: alteração
- Contexto: alteração atômica 6 aprovada (`1`); commits `ffc6a63` (test, red) e `feat: add --no-tests flag to hide tests from query and brief output` (green).
- Decisão tomada: flag global `--no-tests` (padrão falso); `hide.rs` no núcleo filtra só a exibição e devolve quantos itens escondeu: `brief` (símbolos de teste e o resumo `+N testes` do contorno), `callers`/`callees`, `writers`/`readers`, `type`, `symbol` (arestas cujo outro extremo é teste, por `EdgeView.other_is_test`), `impact` (chamadores, consumidores, protocolo e a seção de testes inteira) e `word` (símbolos de teste e a linha de pontes `teste`); `cobertura.testes_ocultos` (sempre no JSON, no texto só quando > 0: `N testes ocultos`); `query tests --no-tests` dá erro de uso (exit 1); as estatísticas e as pontes são calculadas antes do filtro, então não mudam (D12); no MCP, `no_tests` (booleano) de `brief` e `query` vira `--no-tests` antes do subcomando, e o esquema das duas ferramentas o declara.
- Decisão de detalhe minha: "teste" aqui é o teste ou o símbolo de um arquivo de teste (inclui o `mod test_...`); `SymbolAddr::in_tests` virou o critério único, também usado pelo `brief`.
- Observação: o teste `query_tests_then_usage_error_exit_1` passou vazio na fase vermelha (o clap já recusava a flag desconhecida com exit 1); fica como regressão depois que a flag existe.
- Impacto: 245 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos; nenhum golden mudou; conferido de ponta a ponta com o `memo serve` real (a resposta de `brief` com `no_tests: true` não tem a seção `testes:`).
- Critérios afetados: 11 (`--no-tests`)

### [2026-11-08T09:40-03:00] — Etapa 7: evidência de import na resolução de chamadas
- Tipo: alteração
- Contexto: alteração atômica 7 aprovada (`1`, incluindo os 2 testes extras); commits `744ee07` (test, red) e `feat: resolve ambiguous calls by the single imported class that has the method` (green).
- Decisão tomada: nova regra em `resolve_call`, entre a 6 e a 7 da spec: para uma chamada `x.m()` (dois segmentos ou mais) que as regras anteriores não resolveram, as classes que o arquivo importa e que têm `m` (herança incluída, de forma única por método) decidem: exatamente um método candidato vira `CHAMA ~` e a chamada passa a contar como resolvida; zero ou dois ou mais candidatos seguem para a regra 7. Import de classe sem o método, ou de função ou módulo, não conta como evidência. A aresta é aproximada, então não leva palavras de teste (D11) e pesa metade na ponte de chamada.
- Reindexação: já coberta pela regra existente que refaz os arquivos que importam módulos alterados (um método novo na classe importada refaz o arquivo que a importa).
- Impacto: 249 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos; nenhum golden mudou. Desempenho não remedido.
- Não verificado: o efeito no repositório real (a hipótese é que `RequestPaymentUseCase` e `CancelOrderUseCase` passem a aparecer nos chamadores de `OrderEntity.cancel`); depende de o `order` vir de um parâmetro ou de uma chamada sem anotação e de o arquivo importar `OrderEntity` diretamente.
- Critérios afetados: 12 (evidência de import)

### [2026-11-08T10:30-03:00] — Etapa 8: marca âncora genérica
- Tipo: alteração
- Contexto: alteração atômica 8 aprovada (`1`); commits `3c12848` (test, red) e `feat: mark notes anchored on generic words in note list and status` (green).
- Decisão tomada (D15): `notes::is_generic_anchor` diz que uma nota ativa tem âncora (principal ou secundária) numa palavra que hoje é `esqueleto`; `note list` acrescenta `âncora genérica` ao fim da linha da nota; `note show` acrescenta a linha `âncora genérica: sim`; `status` ganhou `generic_anchors` e imprime `âncoras genéricas: <ids>` só quando há alguma (o golden de `status` não mudou); o JSON de `note list` e `note show` ganhou o campo `ancora_generica` (booleano) sem alterar os campos que já existiam; a nota não muda de estado e nenhum evento `NOTE_*` é gravado.
- Teste da coerência: a nota em `word:hub` ficou `proposta` e sem evento depois que a edição de uma docstring de teste fez `hub` virar esqueleto.
- Limite: o `note list` do JSON passou de uma lista de notas para uma lista de notas com um campo a mais; quem lê o JSON antigo continua achando todos os campos de antes.
- Impacto: 252 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos; nenhum golden mudou. Desempenho não remedido.
- Critérios afetados: 13 (nota × esqueleto)

### [2026-11-08T11:50-03:00] — Etapa 9: relatório de aliases
- Tipo: alteração
- Contexto: alteração atômica 9 aprovada (`1`, com as vizinhas de todas as pontes do lote); commits `04d1e59` (test, red) e `feat: add query aliases report suggesting glossary pairs` (green).
- Decisão tomada (D17, D21): `memo query aliases` (nome provisório da TASK mantido) lista pares de palavras: `prefixo` (prefixo comum de pelo menos 5 caracteres e até 3 de resto em cada palavra) e `vizinhança` (Jaccard das vizinhas de pelo menos 0,6, com pelo menos 3 vizinhas em cada palavra, sobre as pontes de símbolo, teste, import e chamada); ordem: prefixo antes de vizinhança, semelhança decrescente, alfabética; palavras `esqueleto` e pares já cobertos por nota `glossario` ativa ficam de fora; cada par traz o `memo note add --tipo glossario ... --alias ...` pronto; é somente leitura (conferido por teste: nem escrita no banco, nem evento, nem nota); `--limit` corta a lista.
- Duas decisões minhas, fora do texto da D21: (1) pares cujas palavras aparecem juntas em algum símbolo (vizinhas diretas) não saem por vizinhança, porque colaboram e não são sinônimos; na primeira versão, o golden da fixture trazia 12 pares de colaboradores como `calculate ~ limit` e `repository ~ sql`; (2) para achar pares sem comparar todas as palavras entre si, só vizinhas compartilhadas por no máximo 50 palavras geram candidatos (com pelo menos 2 vizinhas em comum), e o Jaccard é calculado sobre as vizinhas inteiras. Isso pode deixar de achar um par que só compartilha vizinhas muito comuns.
- Limite honesto: em repositório pequeno a vizinhança ainda sugere pares sem sentido (o golden da fixture mostra `charge ~ test` e `anticipate ~ cancel`); a lista é para o humano aprovar, não para gravar sozinha.
- Impacto: 256 testes passam, duas execuções seguidas; `fmt` e `clippy --all-targets -D warnings` limpos. Desempenho não remedido (o relatório roda só sob demanda).
- Critérios afetados: 14 (aliases)

### [2026-11-08T14:00-03:00] — Etapa 10: correção de desempenho e medição
- Tipo: alteração
- Contexto: regressão da etapa 5 (`brief` e `query word` passavam de 60 s em 5.000 arquivos); aprovada com `1`. Sem teste novo em red: é refatoração de desempenho, o comportamento é coberto pelos testes existentes (golden sem mudança).
- Causa medida: pontes de arquivo e contexto >100 s (auto-junções grandes sobre `occurrence`/`symbol`/`file`), pontes de import 5,9 s (uma consulta por import), pontes de símbolo 105 ms e de chamada 94 ms.
- Decisão tomada: (1) `brief` usa `word_pattern` (verbos, papéis, camadas), sem ficha de pontes nem símbolos etiquetados; (2) pontes de import, arquivo e contexto passaram a trabalhar em memória sobre um `Corpus` carregado com uma consulta por tipo. D19 mantida: nenhuma ponte materializada em tabela.
- Medição (repositório sintético de 5.000 arquivos): índice completo 43 s (meta 60 s; código de indexação não mudou), reindexação sem mudança 0,04 s (meta 300 ms), `brief "calculate total limit"` 0,36 s (meta 500 ms), `query word total` 1,1 s, `query word limit` 2,2 s (palavra com 9.000 usos em quase todo arquivo, caso extremo).
- Limite honesto: `query word` em palavra muito frequente ainda custa 1–2 s (etiquetagem por símbolo e carga do corpus a cada consulta); não há meta de spec para ele.
- Impacto: todos os testes passam; `fmt` e `clippy --all-targets -D warnings` limpos.

### [2026-11-08T15:00-03:00] — code review Aprovado c/ ressalvas
- Revisado dfc4077b; 🔴=0, 🟡=2 (A1 checkboxes do §4, A2 tipo e teste de `word_pattern`), 🟢=2, ⚖️=1; N_PARTITE gravado; GRILL não chamado (sem gatilho: sem DRIFT, sem achado dependente de intenção). Limitação: só o commit final foi relido; os 24 anteriores foram conferidos por testes e goldens.
- DONE depende do `1` do humano; A1 e A2 entram antes (ENGINEERING) ou ficam como dívida declarada.

### [2026-11-08T15:20-03:00] — DONE
- Aprovação do humano (`1`) ao veredito "Aprovado c/ ressalvas" de dfc4077b; Estado: done.
- A1 aplicada agora: checkboxes do §4 marcados (critérios 1–3, desempenho com as medidas da etapa 10, `fmt/clippy/test`).
- Dívida declarada, sem prazo: A2 (tipo `WordPattern` e teste de paridade com `word_info`), A3 (consultas em lote em `tagged_symbols`), A4 (renomear `files_with_word`), A5 (cache do corpus no `serve`, só se houver lentidão).
- Em aberto, não verificado: clientes MCP e harness reais; calibração de `min_coherence`, `min_specificity` e pesos no repositório real (do humano); o `memo` instalado do usuário é antigo (reinstalar recria `base.db`).
