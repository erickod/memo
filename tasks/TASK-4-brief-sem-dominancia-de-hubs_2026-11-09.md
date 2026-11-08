# TASK-4 — `brief` sem dominância de hubs: símbolos com a palavra antes dos vizinhos acumulados
branch: master
workdir: /var/home/erickod/Documentos/dev/memmo
created_at: 2026-11-09

## Status
- Estado: done
- Owner: Erick Duarte
- ROLE requerida: eng

---

## 1. Objetivo
Fazer `memo brief order` listar primeiro os símbolos que realmente carregam a palavra e as suas interconexões diretas, e não os hubs do repositório que só acumulam pontuação por vizinhança. (REF: DOR#CRITERIA)

---

## 2. Contexto
- Origem: na TASK-3 o `brief order` passou a trazer resultado, mas os primeiros símbolos continuaram sendo `Entity.get_events` e `PaymentGatewayRegistry.get` (etiqueta `vizinho`), antes dos métodos de `OrderEntity`. A D8 da TASK-3 propunha ranquear o `brief` por representatividade.
- Causa medida (cópia do repositório real `/tmp/eli`, sem alterar código): em `brief_weighted` (`brief.rs`), um símbolo recebe `normalized(peso) × slot_bonus` por cada ocorrência da âncora, e cada vizinho por aresta `CHAMA`, `RECEBE` ou `RETORNA` (nos dois sentidos) recebe `HOP_FACTOR` (0,5) vezes a pontuação do símbolo ligado. A soma não é normalizada pelo número de ligações. `order` tem 421 símbolos diretos (soma 450,0, máximo 2,0 por símbolo); `Entity.get_events` não tem a palavra e acumula 20,0 por ser vizinho de muitos deles; `conftest.session_factory` acumula 17,0.
- Conclusão: o problema não é falta de representatividade, é dominância de hubs na propagação por salto. A representatividade da ficha (TASK-3) é informação sobre palavras; o salto opera sobre símbolos. A D8 como escrita não resolveria isso sozinha.
- Estado atual do código: `scores` soma direto, teste (docstring), import e salto; ordenação por pontuação descrescente e nome; testes vão para uma seção própria (D7 da TASK-2); `MAX_SYMBOLS` 15.
- Stack: Rust, SQLite, as mesmas dependências das TASKs anteriores.

---

## 3. Escopo

### Dentro do escopo
- Em `brief_weighted`, dois níveis de símbolos de produção: (1) os que carregam a âncora (têm pontuação direta: nome, docstring de teste ou módulo que importa a âncora) e (2) os que só chegam por salto; vagas: até 10 do nível 1 e até 5 do nível 2, e a vaga que um nível não preencher passa ao outro.
- A pontuação por salto do vizinho passa a ser dividida pelo grau dele (número de arestas `CHAMA`, `RECEBE` e `RETORNA` incidentes, nos dois sentidos), para um hub não encabeçar os vizinhos.
- Dentro de cada nível, a ordem continua por pontuação decrescente e nome; os testes seguem na seção própria (D7 da TASK-2).
- Medir antes e depois no `easy_lease_integrated` e no repositório sintético (`brief`, tempo e posição dos símbolos de `OrderEntity`).
- Spec (§5.2 do `brief`) e README.

### Fora do escopo
- Decidir se uma palavra é de domínio.
- Usar a representatividade de palavras no ranking de símbolos.
- Mudar pontes, esqueleto, a ficha `query word` ou o número total de símbolos (`MAX_SYMBOLS` 15, testes 5).
- Chave de `memo.toml` para o fator de salto ou para a divisão 10/5 (D2).

---

## 4. Critérios de Aceitação
- [x] na fixture, um símbolo que carrega a âncora fica acima de um vizinho que só acumula pontuação por salto, mesmo quando o vizinho é chamado por muitos símbolos da âncora
- [x] com mais de 10 símbolos que carregam a âncora e vizinhos disponíveis, o `brief` lista 10 do nível 1 e 5 do nível 2, nessa ordem
- [x] quando o nível 2 tem menos de 5 vizinhos, as vagas que sobram vão para mais símbolos do nível 1, até 15 no total; quando o nível 1 tem menos de 10, as que sobram vão para vizinhos
- [x] a pontuação por salto de um vizinho é dividida pelo número de arestas `CHAMA`, `RECEBE` e `RETORNA` incidentes nele: um hub com soma maior mas muito mais ligações fica abaixo de um vizinho específico (refinado na TEST_ANALYSIS: o teste usa soma maior no hub, que é o caso real)
- [x] um símbolo que carrega a âncora e também é vizinho fica no nível 1
- [x] o `brief` continua determinístico, byte a byte; testes seguem na seção própria e `--no-tests` continua válido
- [x] medido no repositório real: os métodos de `OrderEntity` aparecem antes de `Entity.get_events` e de `PaymentGatewayRegistry.get` no `brief order`, e vizinhos aparecem nas 5 últimas vagas de produção (medida, não bloqueia)
- [x] `brief` ≤ 500 ms em 5.000 arquivos; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas

---

## 5. Restrições e Regras
- técnicas: princípios 1 a 7 da spec §2 (determinístico, saída compacta, cobertura explícita); `memo-core` não depende de `clap` nem de MCP.
- negócio: o `memo` entrega fatos, não julga código.
- dependências obrigatórias: TASK-3 concluída (`done`).
- regra de dados: sem mudança de esquema; o banco continua derivado.
- invariante (reescrita no code review, A2): dentro da lista, um símbolo com pontuação direta vem antes de um símbolo que só chegou por salto, na mesma seção (produção ou testes), respeitada a cota de vagas (10 do nível 1 e 5 do nível 2 na produção): os símbolos do nível 1 além da cota ficam de fora enquanto vizinhos entram; guarda: a partição em níveis e `pick_slots` em `brief_weighted`.

---

## 6. Impacto em Dados
- [x] Nenhuma alteração de schema
- [ ] Uso de entidades existentes
- [x] Leitura apenas
- [ ] Escrita controlada

---

## 7. Estratégia de Testes
escritor: test-analyst

**Cenários** — Happy Path: símbolos que carregam a âncora antes dos vizinhos, cota 10/5, amortecimento por grau, nível 1 que também é vizinho, módulo que importa a âncora · Error Scenarios: nenhum comando novo; `--no-tests` segue válido · Edge Cases: nível 2 com menos de 5 (vagas vão ao nível 1), nível 1 com menos de 10 (vagas vão aos vizinhos), empate de pontuação (qname crescente), seção de testes com o mesmo critério, determinismo.

**Fixtures** (um módulo `app/o/core.py` com todas as funções, para as chamadas se resolverem como `CHAMA` exatas no mesmo módulo; nomes sem a palavra `order` fora dos símbolos de âncora; `<tok>` = `waa`, `wab`, …):
- **A**: `emit()`, `audit()`, `order_a()` a `order_d()` (cada um chama `emit()`; só `order_a` e `order_b` chamam `audit()`), e `noise_waa()` a `noise_wae()` mais `noise_waf()` (6 funções que chamam só `emit()`). Graus: `emit` 10 arestas incidentes (4 de `order_*` e 6 de `noise_*`), `audit` 2.
- **B**: `emit()`, `order_<tok>()` para `waa` a `wal` (12; cada um chama `emit()`, e os 6 primeiros chamam também `leaf_<tok>()`), `leaf_waa()` a `leaf_wae()` mais `leaf_waf()` (6 folhas), e 8 funções `noise_<tok>()` que chamam só `emit()`. `emit` tem grau 20; cada folha tem grau 1. Esperado: dez `order_waa` a `order_wak`, depois `leaf_waa` a `leaf_wae` (cada folha 0,5 vezes a pontuação direta; `emit` 6,0 sobre 20 = 0,3 vezes essa pontuação, abaixo das folhas); a `leaf_waf` e `emit` ficam fora.
- **C**: `emit()`, `audit()` e 14 símbolos `order_<tok>()` que chamam `emit()` e `audit()` (só dois vizinhos).
- **D**: 3 símbolos `order_a()`, `order_b()`, `order_c()`, cada um chamando 3 folhas próprias (`leaf_a1x` com letras, sem dígito).
- **E**: `emit()`, `order_a()` (chama `order_b()` e `emit()`), `order_b()` (chama `emit()`).
- **F**: `app/o/tests/test_core.py` com `helper()`, `test_order_a()` a `test_order_d()` (cada um chama `helper()`) e 8 testes `test_zz_<tok>()` que chamam só `helper()`.
- **G**: a fixture A mais `app/o/user.py` com `from app.o.core import order_a`.

**Rastreabilidade**
- critério 1 (anchor acima do hub) → `hub_called_by_anchor_symbols`, `neighbor_below_anchor_symbols`
- critério 2 (10 mais 5) → `twelve_anchor_symbols_and_neighbors`
- critério 3 (vagas que sobram) → `two_neighbors_only`, `three_anchor_symbols`
- critério 4 (grau) → `hub_with_many_links` (soma maior, mais ligações: o hub fica abaixo)
- critério 5 (nível 1 que é vizinho) → `anchor_symbol_that_is_also_neighbor`, `module_importing_anchor_word`
- critério 6 (determinismo, testes, `--no-tests`) → `same_tree_twice`, `test_hub`, `no_tests_flag_when_brief_hub_fixture`
- invariante do §5 (direto nunca abaixo de só-vizinho, na mesma seção) → `hub_called_by_anchor_symbols`, `test_hub`
- critério 7 (medido no repositório real) → não coberto por teste: medição no log (não bloqueia)
- critério 8 (desempenho) → não coberto por teste: medição no log (meta, não bloqueia); `fmt`, `clippy` e `test` por comando
- não coberto: calibração de 10/5 e do amortecimento; efeito do grau com arestas aproximadas (`~`, o grau conta todas as arestas `CHAMA`, `RECEBE` e `RETORNA` resolvidas)

**Lifecycle Coverage**
- [ ] Nível: derivado da pontuação direta a cada consulta; cardinalidades 0, 1 e N no nível 2 → `three_anchor_symbols` (9 vizinhos), `two_neighbors_only` (2), fixture D (0 vizinhos do nível 1 sem vagas sobrando)
- [ ] Vaga: cota 10/5 com redistribuição → `twelve_anchor_symbols_and_neighbors`, `two_neighbors_only`, `three_anchor_symbols`
- [ ] Símbolo que é nível 1 e vizinho: fica no nível 1 → `anchor_symbol_that_is_also_neighbor`
- [ ] Escritor do ranking sem guarda: `brief_weighted` é o único; invariante coberta pelos testes acima

### Testes Unitários
(nenhum: a pontuação depende do grafo no banco; é testada por integração com banco em memória)

### Testes de Integração
- [ ] test_given_hub_called_by_anchor_symbols_when_brief_then_anchor_symbols_listed_first  (83 chars)
    - GIVEN fixture A (below) and the request "order"; WHEN brief is computed; THEN the production symbols start with order_a, order_b, order_c, order_d, before emit and audit
- [ ] test_given_neighbor_below_anchor_symbols_when_brief_then_neighbors_still_listed  (79 chars)
    - GIVEN fixture A and the request "order"; WHEN brief is computed; THEN emit and audit are still listed, after the four order_* symbols
- [ ] test_given_hub_with_many_links_when_brief_then_it_ranks_below_specific_neighbor  (79 chars)
    - GIVEN fixture A, where emit has 10 incident call edges and 4 anchor callers and audit has 2 incident call edges and 2 anchor callers; WHEN brief is computed; THEN audit is listed before emit (0.5 against 0.2 times the anchor score; before the change emit came first)
- [ ] test_given_twelve_anchor_symbols_and_neighbors_when_brief_then_ten_then_five_slots  (82 chars)
    - GIVEN fixture B (below) and the request "order"; WHEN brief is computed; THEN 15 production symbols: the first 10 are order_waa to order_wak, then leaf_waa to leaf_wae
- [ ] test_given_two_neighbors_only_when_brief_then_spare_slots_go_to_anchor_symbols  (78 chars)
    - GIVEN fixture C (14 anchor symbols, 2 neighbors) and the request "order"; WHEN brief is computed; THEN 15 production symbols: 13 order_* symbols followed by the 2 neighbors
- [ ] test_given_three_anchor_symbols_when_brief_then_spare_slots_go_to_neighbors  (75 chars)
    - GIVEN fixture D (3 anchor symbols, each calling 3 distinct leaves) and the request "order"; WHEN brief is computed; THEN 12 production symbols: the 3 order_* symbols followed by the 9 leaves
- [ ] test_given_anchor_symbol_that_is_also_neighbor_when_brief_then_it_stays_in_first_tier  (85 chars)
    - GIVEN fixture E (order_a calls order_b and emit, order_b calls emit) and the request "order"; WHEN brief is computed; THEN order_a and order_b are listed before emit, and emit is still listed
- [ ] test_given_test_hub_when_brief_then_direct_tests_precede_neighbor_in_tests_section  (82 chars)
    - GIVEN fixture F (4 tests named test_order_*, each calling helper, and 8 other tests calling helper) and the request "order"; WHEN brief is computed; THEN in the tests section the 4 test_order_* come before helper
- [ ] test_given_module_importing_anchor_word_when_brief_then_module_is_in_first_tier  (79 chars)
    - GIVEN fixture G (a module that imports order_a from the core module, plus emit called by order_a) and the request "order"; WHEN brief is computed; THEN the importing module is listed before emit
- [ ] test_given_same_tree_twice_when_brief_then_symbols_are_identical  (64 chars)
    - GIVEN fixture B indexed twice into separate stores; WHEN brief is computed on each; THEN both results are equal, symbol by symbol and score by score
- [ ] test_given_hub_fixture_when_brief_then_text_lists_anchor_symbols_before_neighbors  (81 chars)
    - GIVEN fixture A committed to a git repository; WHEN memo brief order runs; THEN the output line for order_a comes before the lines for emit and audit
- [ ] test_given_no_tests_flag_when_brief_hub_fixture_then_production_order_unchanged  (79 chars)
    - GIVEN fixture A plus one test file with test_order_a, committed; WHEN memo brief order runs with and without --no-tests; THEN the production symbol lines are identical in both outputs

**FIRST**
- [x] Fast: banco em memória e árvore temporária com poucos arquivos
- [x] Isolated: cada teste monta o próprio repositório
- [x] Repeatable: pontuações por construção, ordens totais (pontuação, depois qname)
- [x] Self-validating: cada teste afirma a ordem exata dos qnames
- [x] Timely: escritos antes do código; os testes de ordem e de cota falham hoje (um hub chamado por quatro símbolos empata ou vence os símbolos da âncora)

---

## 8. Dependências
- tasks predecessoras: TASK-3
- módulos | serviços externos: nenhum
- flags | configurações: a definir (um possível parâmetro de `memo.toml` para o fator de salto)

---

## 9. Observações

### Decisões (grill-me)
Origem: humano, 2026-11-09, resposta `1` às recomendações Q1 a Q3.
- D1: a propagação por salto passa a ter dois níveis: símbolos que carregam a âncora primeiro, vizinhos depois; e o salto é amortecido pelo número de ligações do vizinho (alternativa C). (humano, recomendação aceita)
- D2: o fator de salto (`HOP_FACTOR` 0,5) não vira chave do `memo.toml` nesta TASK. (humano, recomendação aceita)
- D3: hubs de infraestrutura como `Entity.get_events` continuam aparecendo, depois dos símbolos que carregam a âncora e amortecidos. (humano, recomendação aceita)
- D4: vagas para vizinhos (Q4, alternativa a): até 10 símbolos do nível 1 e até 5 do nível 2 entre os 15 de produção; a vaga que um nível não preencher passa ao outro. A divisão 10/5 é um valor inicial sem medição, a calibrar pelo humano. (humano, recomendação aceita com o `1`)
- D5: [DEFINITION] o amortecimento é a divisão da pontuação por salto pelo grau do vizinho (arestas `CHAMA`, `RECEBE` e `RETORNA` incidentes, nos dois sentidos), sem raiz: com a divisão simples um hub com 80 ligações e soma 20,0 cai para 0,25 e fica abaixo de um vizinho específico com 2 ligações e soma 1,0 (0,5). Refina D1; fórmula escolhida por mim, a validar na aprovação. (recomendação)
- (Q4 original, resolvida em D4): com `MAX_SYMBOLS` 15 e centenas de símbolos que carregam a âncora (`order`: 421), os vizinhos podem ficar sem vaga e a interconexão desaparece. Alternativas: (a) reservar vagas para vizinhos (ex.: 10 diretos e 5 vizinhos); (b) sem reserva, vizinhos só entram se sobrar vaga; (c) intercalar por pontuação normalizada dentro de cada nível. Recomendação: (a), porque o objetivo do humano são as interconexões e (b) as apagaria justamente nas palavras centrais.

### Premissas
- [DEFINITION] A hipótese da D8 (ranquear por representatividade) foi substituída pela causa medida (dominância de hubs na propagação por salto).
- [DEFINITION] Verificado em `/tmp/eli`: `Entity.get_events` aparece entre os 3 primeiros em `order`, `reservation`, `payment`, `cancel order` e `account` (quatro dos cinco pedidos testados, `hold` o traz em 3º); o problema é geral, não de `order`.
- [TEST_ANALYSIS] O grau conta as arestas `CHAMA`, `RECEBE` e `RETORNA` com `dst` resolvido, nos dois sentidos, a mesma consulta de `neighbors()` em `brief.rs`; arestas sem `dst` não contam. Verificado no código.
- [TEST_ANALYSIS] Na fixture os símbolos que carregam a âncora têm pontuação direta de mesmo valor `s` (o slot de `order` é igual em todos os nomes `order_*`); os testes afirmam relações de ordem, não o valor absoluto de `s`.
- [DEFINITION] A medição acima usou um script de leitura sobre o banco da cópia, reproduzindo a fórmula de `brief_weighted`; o `brief` real mostrou o mesmo `Entity.get_events` no topo.

### N_PARTITE (DEFINITION)
[N_PARTITE] modo: concept | alvo: snapshot | referência: — | perfil: stacked + core | budget: 10 | frame: base | fonte: redação da DEFINITION de 2026-11-09 (frame manual; a skill n-partite não foi executada como CALL)

Elementos: L4 ranking do `brief` | L2 Símbolo (nível 1, nível 2) · Vizinho · Hub · Vaga | L3 pontuação direta · propagação por salto · amortecimento por grau · partição em níveis · preenchimento de vagas | L1 `HOP_FACTOR` 0,5 · grau · divisão 10/5 · `MAX_SYMBOLS` 15.
lifecycle.onto: Pontuação (calculada por consulta, sem estado) · Nível (derivado da existência de pontuação direta, muda com o texto do pedido e com o índice) · Vaga (cota fixa, sem ciclo) — `não definido`=0.

[N_PARTITE:concept] par: Símbolo (L2) ↔ Vizinho (L2) — acoplamento: alto | evidência: medido (`Entity.get_events` 20,0 contra 2,0) | abstração: nível 2 só por salto, amortecido pelo grau | lifecycle: divergente (o nível depende do pedido) → derivado por consulta, nunca gravado
[N_PARTITE:concept] par: Vaga (L2) ↔ propagação por salto (L3) — acoplamento: médio | evidência: não definido | abstração: cota que garante a interconexão | lifecycle: coerente
[N_PARTITE.STRUCT:concept] alvo: `brief_weighted` — problema: soma direta, teste, import e salto num só mapa `scores` | estrutura proposta: separar a pontuação direta da pontuação por salto e juntar só na ordenação por nível | base: `scores` e `direct` em `brief.rs`
[N_PARTITE.INV?] invariante: um símbolo com pontuação direta nunca fica abaixo de um símbolo só-vizinho na lista | escritor: `brief_weighted` | guarda: partição por nível | nível: a promover no §5 (aprovação)

### Plano de Execução
#### Etapas
1. Níveis, cotas e amortecimento em `brief_weighted` (testes red primeiro) — depende de: NONE
2. Medições no repositório sintético e no `easy_lease_integrated`; spec (§5.2) e README — depende de: 1
#### Gates
- após cada etapa: testes da etapa e `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` limpos
- após a etapa 1: determinismo do `brief` conferido em duas execuções e goldens revisados lendo, um a um
- após a etapa 2: medidas registradas no log (não bloqueiam o aceite)
#### Riscos
- a divisão 10/5 não serve a todos os pedidos → valor inicial, medida no repositório real; sem chave de config nesta TASK (D2)
- o amortecimento tira vizinhos úteis (hubs legítimos) → eles ainda entram nas 5 vagas, só ordenados abaixo (D3)
- os goldens do `brief` mudam → revisar cada um lendo
- o `brief` pode ficar lento com o cálculo de grau por vizinho → agregar os graus em uma consulta; meta ≤ 500 ms
- pedidos com vários pontos de entrada deixam o nível 2 grande demais → vale a cota de 5
#### Assunções
- o banco derivado e descartável; sem mudança de esquema
- a calibração de 10/5 e do amortecimento no repositório real é do humano e não bloqueia o aceite
- precisa existir uma fixture com um hub chamado por muitos símbolos que carregam a âncora e com mais de 10 símbolos diretos

### N_PARTITE (CODE_REVIEW)
[N_PARTITE] modo: multi (concept×code) | alvo: delta | referência: — | perfil: stacked + full | budget: 10 | frame: base | fonte: 73e522f..1ac7ff5
- REUSE: miss (sem frame de CODE_REVIEW anterior para esta TASK); frame base. Frame escrito à mão a partir da leitura do diff, sem executar a skill n-partite como CALL (limitação registrada).
[N_PARTITE:code] par: brief_weighted (L3) ↔ pick_slots/rank_tier/link_degrees (L3) — acoplamento: médio | evidência: não definido | abstração: três funções pequenas extraídas, `brief_weighted` segue com a soma direta, de teste e de import | lifecycle: coerente (sem estado)
[N_PARTITE:code] par: tier 1 (L1) ↔ cota NEIGHBOR_SLOTS (L1) — acoplamento: alto | evidência: medido (`order`: 10 vagas do nível 1 tomadas por módulos) | abstração: cota fixa 10/5 como constante, sem config (D2) | lifecycle: divergente (o corpus cresce, a cota não) → registrada como limite
[N_PARTITE.STRUCT:code] alvo: `brief_weighted` — problema: função longa (soma direta, teste, import, salto, níveis, cotas, contorno, padrão, notas) | estrutura proposta: separar a pontuação (direta e por salto) da montagem das seções | base: `brief.rs`, `brief_weighted` ~190 linhas
[N_PARTITE.DRIFT] par: invariante do §5 ("direto nunca abaixo de só-vizinho na mesma seção") ↔ pick_slots — situação: lacuna de redação: com mais de 10 símbolos de nível 1, os do nível 1 que passam da cota ficam de fora enquanto vizinhos entram (D4), então "nunca abaixo" só vale dentro da lista, na cota → corrigir o texto do §5 (A2); par: D1 a D5 ↔ brief.rs — situação: sem drift
[N_PARTITE.FAIL] elemento: link_degrees — falha: varredura completa da tabela `edge` por `brief` | proteção: medido 0,38 s em 5.000 arquivos (meta 500 ms) | lacuna: sem teste com aresta duplicada entre o mesmo par (COUNT conta linhas)
[N_PARTITE.GAP] completeness: não definido (sem referência)
[N_PARTITE.CLUSTER] brief.rs, bridges.rs, stats.rs e representativeness.rs mudam juntos nas TASKs 2 a 4; o `brief` agora tem dois critérios de "vizinho" (`neighbors()` por arestas e `bridge_tags` por pontes de palavra) sem teste que os cruze
[N_PARTITE.INV?] invariante: o resultado do `brief` é determinístico | escritor: `brief_weighted` | guarda: `BTreeMap` e ordens totais (pontuação, depois qname) | nível: guardado e testado (`same_tree_twice`)
[N_PARTITE.SIDE] efeito colateral: a lista de `scores` deixa de ser monotônica no JSON (há uma subida na fronteira entre níveis); consumidores que assumirem ordem por pontuação decrescente podem errar | não verificado em `serve`/MCP
[N_PARTITE.RANK] 1) invariante do §5 desalinhada com a cota (A2) · 2) comentário de documentação no lugar errado (A1) · 3) `brief_weighted` longa (A3) · 4) score não monotônico no JSON (A4) · 5) módulos tomam as vagas do nível 1 (limite conhecido)

### Code Review
#### [2026-11-09T05:40-03:00] — 1ac7ff55
- WT: /var/home/erickod/Documentos/dev/memmo (branch master; sem PR, revisão local; `.agents`, `.mcp.json` e `CLAUDE.md` não rastreados, gerados pelo `memo init`)
- lang sem reference: rust (só CRITERIA base)
- flags: ddd, perf_deep e refactor_only não ativadas na TASK (§9/Plano/Assunções); não inferidas.

**SUMMARY — Aprovado c/ ressalvas**
- Positivos: os 3 commits (`73e522f..1ac7ff5`) seguem red, green e docs, com `clippy` conferido antes de cada commit; as decisões D1 a D5 têm código e teste; `brief` determinístico por `BTreeMap` e ordens totais; a mudança de comportamento foi decidida com o humano durante a etapa (módulos no nível 1) e registrada; 304 testes passam, `fmt` e `clippy` limpos; 0,38 s em 5.000 arquivos.
- Preocupações: (1) o comentário de documentação de `brief_weighted` ficou em cima de `type Ranked`; (2) a invariante do §5 está mais forte que a cota D4; (3) `brief_weighted` tem cerca de 190 linhas; (4) o JSON do `brief` perde a monotonia do `score`; (5) a revisão não inclui `serve`/MCP nem outros repositórios.

**ANALYSIS**
- [ ] design: `link_degrees`, `rank_tier` e `pick_slots` coesas e puras; `brief_weighted` longa (achado A3).
- [ ] DDD: não identificado — verificado em: TASK §9/Plano (flag ddd não ativada; não inferida).
- [ ] reliability: `pick_slots` (brief.rs:358-366): `total - second.len().min(reserved)` não subtrai abaixo de zero (15 e 5, ou 5 e 0), `take_second` limitado por `total - take_first`; leitura apenas, sem retry nem dual write; idempotente.
- [ ] data: não identificado — verificado em: diff 1ac7ff5 (sem valor monetário nem PII).
- [ ] contract: `Brief` e `ScoredSymbol` não mudam de forma; muda a semântica do `score` na lista (uma subida na fronteira entre níveis) — achado A4; consumidores por `grep -w brief_weighted|brief_with|ScoredSymbol` em `crates/`: `brief.rs`, `memo-cli/src/main.rs`; `serve`/MCP não verificado.
- [ ] env_k8s: não identificado — verificado em: diff sem settings nem manifest.
- [ ] concurrency: não identificado — verificado em: `brief` só lê, sob o lock por banco já existente.
- [ ] migrations: não identificado — verificado em: sem mudança de esquema (`§6` da TASK).
- [ ] perf: `link_degrees` varre `edge` uma vez por `brief` e `rank_tier` busca o endereço de cada candidato (já era assim); medido 0,38 s em 5.000 arquivos e 0,04 s no repositório real; sem N+1 novo.
- [ ] security: não identificado — verificado em: SQL fixo, sem entrada do usuário.
- [ ] errors: não identificado — verificado em: `?` com `CoreError`; sem `unwrap` em código de produção.
- [ ] readability: comentário `/// Como brief_with...` (brief.rs:323) agora documenta `type Ranked` (achado A1); nomes `first`/`second` para níveis são genéricos mas locais.
- [ ] layers: não identificado — verificado em: imports do diff (`memo-core` sem `clap` nem MCP).
- [ ] tests: §7 presente com nomes exatos (12 testes); os 4 que passavam antes do green são guardas declaradas; falta teste de aresta duplicada entre o mesmo par e de arestas aproximadas no grau (A5); dois testes antigos trocados com aval explícito.
- [ ] task: §4 critérios 1–8 com teste ou medição; D1 por `tier`, D4 por `pick_slots` e testes 4–6, D5 por `hub_with_many_links`; invariante do §5 desalinhada com D4 (A2); D2 (sem config) respeitada.
- [ ] format: não identificado — verificado em: `cargo fmt --check` OK; `clippy -D warnings` sem avisos.
- [ ] implicit: `degree_of` assume grau 1 para vizinho fora do mapa, o que não ocorre porque `neighbors()` e `link_degrees` usam as mesmas arestas; se uma divergir, o amortecimento some em silêncio; sem teste que trave a coincidência (A5).
- [ ] refactor_estrutural: o mapa único `scores` foi dividido em direto e salto; verificado que a pontuação direta é a mesma de antes (mesmas fontes) e que o salto só soma sobre `direct`.
- [ ] smells¹: Long Method — `brief_weighted` ~190 linhas (A3); Long Params — `pick_slots` com 4; Magic Numbers — `NEIGHBOR_SLOTS` nomeada e `MAX_SYMBOLS` nomeada; Feature Envy/Msg Chains/Ignored Exceptions/Dead Code — não identificado — verificado em: diff c198207.
- [ ] N_PARTITE: §9/N_PARTITE (CODE_REVIEW); perfil `stacked + full`, REUSE miss, frame base manual.
- [ ] drift (DEFINITION × código): base e deltas do §9/N_PARTITE (DEFINITION) comparados no mesmo frame — DRIFT=0, com a lacuna de redação da invariante (A2) registrada.

**VERIFY** — reli o diff `73e522f..1ac7ff5` sem olhar a ANALYSIS: sem omissão além de A1–A5; conferi que símbolos de nível 1 também vizinhos recebem salto amortecido (`score + damped`), que `second` exclui os de `direct` e que não há divisão por zero (`degree_of` ≥ 1).

**ACTIONS**
- 🟡 A1 — o comentário `/// Como brief_with, com os pesos de ponte do repositório...` deixou de documentar `brief_weighted` e está sobre `type Ranked` (brief.rs:323-324). Correção: mover o comentário para cima de `pub fn brief_weighted`. before: doc na alias de tipo; after: doc na função.
- 🟡 A2 — a invariante do §5 diz "um símbolo com pontuação direta nunca fica abaixo de um que só chegou por salto"; com a cota D4 (10 do nível 1, 5 do nível 2) os símbolos do nível 1 além do décimo ficam fora enquanto vizinhos entram (teste 4 mostra isso). Correção: reescrever o §5 como "dentro da lista, um símbolo com pontuação direta vem antes de um que só chegou por salto, respeitada a cota de vagas".
- 🟢 A3 — `brief_weighted` com ~190 linhas; extrair a pontuação (direta e por salto) das seções.
- 🟢 A4 — documentar (spec e README) que `score` no JSON do `brief` deixa de ser monotônico na lista; ou expor o nível em `ScoredSymbol`.
- 🟢 A5 — testes de arestas duplicadas entre o mesmo par no grau e de coincidência entre `neighbors()` e `link_degrees`.
- ⚖️ A6 — módulos tomam as vagas do nível 1 no `order`: (a) manter (decisão do humano); (b) dar peso próprio a módulos; (c) limitar módulos no nível 1. Recomendação: (a) agora; decidir (b) ou (c) numa TASK com medição.
- ❓ nenhuma pergunta ao autor com 🔴 potencial.

**COMMITS**
- ✓ [a691499](https://github.com/erickod/memo/commit/a69149986f89492c4cbfa3abd70503dc2ad2dfcf) test: add brief tier, slot quota and hub damping tests — red por asserção (8 dos 12), 4 guardas declaradas.
- ✓ [c198207](https://github.com/erickod/memo/commit/c198207dbede037f9a450007719780983ffa519b) feat: rank brief symbols by tier with hub damping and neighbor slots — ⚠ comentário de documentação fora do lugar (A1).
- ✓ [1ac7ff5](https://github.com/erickod/memo/commit/1ac7ff5508b32cc80804f477ff99b60b9ef39178) docs: document brief tiers, slot quota and hub damping.
- Mensagens seguem a regra de commit sem atribuição da TASK.

**EVIDÊNCIAS**
- SHA revisado: 1ac7ff5508b32cc80804f477ff99b60b9ef39178 (HEAD); sem PR, então `headRefOid` não se aplica; `repo_url` via `gh repo view --json url -q .url`: https://github.com/erickod/memo.
- Comandos: `cargo fmt --check` OK; `cargo clippy --all-targets -- -D warnings` 0 avisos; `cargo test --no-fail-fast` 304 passam, 0 falham.
- Medidas: repositório sintético de 5.000 arquivos, índice 44 s, sem mudança 0,04 s, `brief` 0,38 s; cópia do repositório real (801 arquivos) `brief order` 0,04 s.
- Consumidores: `grep -w brief_weighted|brief_with|ScoredSymbol` em `crates/`; Semble não foi consultado; vazio não prova ausência.
- Não executado: `serve`/MCP com o `score` não monotônico; teste de arestas duplicadas; binário instalado no repositório do humano; revisão em PR (não há).


## 10. Log de Decisões e Alterações

### [2026-11-09T00:40-03:00] — Criação da TASK-4
- Tipo: criação
- Contexto: pedido do humano (`Faça`) para abrir a TASK da D8 depois do DONE da TASK-3.
- Decisão tomada: antes de definir, conferi a causa do ranking ruim reproduzindo a fórmula de `brief_weighted` sobre a cópia do repositório real. O resultado desmentiu a premissa da D8: o topo é dominado por hubs que acumulam o bônus de salto (`Entity.get_events` 20,0 contra no máximo 2,0 dos símbolos que carregam `order`). A TASK-4 nasce com esse escopo, em `Estado: draft`.
- Impacto: STATE=DEFINITION, com 3 decisões abertas (Q1 a Q3) para o humano; sem N_PARTITE nem plano ainda.
- Não verificado: o efeito de cada alternativa (A, B, C) no `brief` real; se outros pedidos (não só `order`) sofrem do mesmo problema.

### [2026-11-09T01:05-03:00] — Decisões Q1 a Q3 e medição em outros pedidos
- Tipo: alteração
- Contexto: resposta `1` do humano aceitou as recomendações Q1 a Q3 (agora D1 a D3). Medi cinco pedidos no `brief` da cópia do repositório real (`order`, `reservation`, `payment`, `cancel order`, `hold`, `account`): `Entity.get_events` aparece entre os primeiros em quase todos, quase sempre só por vizinhança.
- Decisão tomada: consolidar D1 a D3; abrir Q4 (vagas para vizinhos), que surgiu da análise: os 15 lugares de `MAX_SYMBOLS` podem ser todos tomados por símbolos que carregam a âncora.
- Impacto: DEFINITION segue em `draft`; falta Q4, N_PARTITE, plano e a aprovação única.
- Não verificado: o efeito de cada alternativa de Q4 no `brief` real.

### [2026-11-09T01:30-03:00] — DEFINITION consolidada
- Tipo: alteração
- Contexto: resposta `1` aceitou a alternativa (a) de Q4 (agora D4).
- Decisão tomada: consolidar D4 e D5 (fórmula do amortecimento, escolhida por mim, a validar); reescrever §3 e §4; N_PARTITE em perfil `core` (frame manual); plano de 2 etapas; invariante proposta no §5 (a aprovar). §7 fica para TEST_ANALYSIS.
- Impacto: STATE=DEFINITION concluída, aguardando a aprovação única.
- Não verificado: o efeito real de 10/5 e da divisão por grau no `brief` do repositório real (só na etapa 2); se `grau` por arestas incidentes é barato nas 5.000 arquivos.

### [2026-11-09T02:10-03:00] — TEST_ANALYSIS
- Tipo: alteração
- Contexto: aprovação da DEFINITION (`1`); `test-analyst` escreveu o §7 com 12 testes de integração (10 no núcleo e 2 no CLI), sem testes de unidade (a pontuação depende do grafo), todos com nome `given_..._when_..._then_...` de até 89 caracteres e GIVEN/WHEN/THEN em inglês.
- Decisão tomada: valores esperados por aritmética das fórmulas da D4 e D5 sobre fixtures descritas no §7 (A a G); duas premissas registradas no §9 (grau pelas arestas de `neighbors()`, pontuação direta igual nos `order_*`). O critério 4 foi refinado: o teste usa hub com soma maior (caso real), não "mesma soma", para falhar hoje.
- Lacunas verificadas no ambiente antes de perguntar: nenhuma pergunta aberta. GRILL não chamado (sem lacuna sem resposta).
- Não coberto por teste: calibração de 10/5 e do amortecimento, efeito de arestas aproximadas no grau, medições no repositório real.
- Impacto: TEST_ANALYSIS concluída; aguardando o `1` para a ENGINEERING. N_PARTITE reaproveitado (frame da DEFINITION), sem nova CALL.

### [2026-11-09T04:10-03:00] — Etapa 1: níveis, cota e amortecimento
- Tipo: alteração
- Contexto: alteração atômica 1 aprovada (`1`); commits `a691499` (test, red: 8 dos 12 falhando por asserção, os outros 4 são guardas) e `feat: rank brief symbols by tier with hub damping and neighbor slots` (green).
- Decisão tomada (D1, D4, D5): em `brief_weighted`, a pontuação direta e a de salto passam a ser mapas separados; o salto do vizinho é dividido pelo grau (arestas `CHAMA`, `RECEBE`, `RETORNA` resolvidas incidentes nos dois sentidos, agregadas numa consulta só); nível 1 = quem tem pontuação direta, com o salto amortecido somado; nível 2 = só salto; produção: até 10 do nível 1 e 5 do nível 2 (`pick_slots`, vagas que sobram passam ao outro nível); testes: nível 1 antes do nível 2, sem reserva (5 vagas); dentro de cada nível, pontuação decrescente e qname.
- Decisão do humano durante a etapa (pergunta de reentrada): o golden do `brief "calculate limit"` mostrou que módulos que só importam a âncora (nível 1, sinal 0,12) passam à frente de `CalculateLimitUseCase.execute` (nível 2) e o contorno dele some. Alternativas: manter como definido (1) ou tratar import-only como nível 2 (2, recomendada por mim). O humano escolheu `1`, manter como definido.
- Testes antigos trocados (explicados ao humano antes): `brief.rs::many_words_...` agora aceita no máximo uma subida de pontuação por grupo (a fronteira entre níveis); o golden `query__given_request_when_brief_then_golden_sections.snap` foi atualizado depois de ler o diff (os símbolos de nível 2 foram para o fim; `execute` perdeu o contorno).
- Impacto: 304 testes passam; `fmt` e `clippy --all-targets -D warnings` limpos. Medidas: `brief "calculate total limit"` 0,37 s em 5.000 arquivos (meta 500 ms); reindexação sem mudança 0,04 s.
- Resultado no repositório real (cópia `/tmp/eli`): `brief order` deixou de trazer `Entity.get_events` e `PaymentGatewayRegistry.get`; os métodos de `OrderEntity` (`end_with_reservation`, `confirm_payment`, `register_reference`, `request_payment`, `register_gateway_response`) aparecem como vizinhos nas 5 vagas do nível 2. Mas as 10 vagas do nível 1 foram todas para módulos (`mod cancel_order_..`, `mod order`, etc. com `[nome, import]`), porque um módulo soma nome e import e passa a classe `OrderEntity` (só nome); a classe e os métodos com `order` no nome ficam fora.
- Limite honesto: o objetivo "símbolos que carregam a âncora primeiro" se cumpre, mas no `order` o nível 1 vira uma lista de módulos; o `brief` ainda não mostra as classes e os métodos centrais no nível 1. Possível correção futura (nova decisão): módulos pesam menos que classes e funções, ou limite de módulos por nível.
- Não verificado: outros pedidos no repositório real depois da mudança; efeito de arestas aproximadas no grau.
- Critérios afetados: 1 a 6, invariante do §5

### [2026-11-09T05:00-03:00] — Etapa 2: medições, spec e README
- Tipo: alteração
- Contexto: etapa 2 aprovada (`1`); commit `docs: document brief tiers, slot quota and hub damping`.
- Medições (repositório sintético de 5.000 arquivos): índice completo 44 s (meta 60 s), reindexação sem mudança 0,04 s, `brief "calculate total limit"` 0,38 s (meta 500 ms). Cópia do repositório real (801 arquivos): índice 9 s, `brief order` 0,04 s.
- Outros pedidos na cópia do repositório real, depois da mudança: em `reservation`, `payment`, `cancel order`, `hold` e `account`, `Entity.get_events` e `PaymentGatewayRegistry.get` saíram do topo; os primeiros símbolos têm a palavra no nome (`ItemCalendar.get_reservation`, `OrderEntity.confirm_payment`, `Reservation.create_hold`); em `cancel order`, `hold` e `account` ainda aparecem vários módulos `[nome, import]` entre os primeiros (o limite já registrado na etapa 1).
- Spec §8.5 passo 4 e README: níveis, cota 10/5, amortecimento por grau e o aviso de que 10 e 5 são iniciais e não configuráveis.
- Critérios do §4 marcados `[x]` (os de medição no repositório real e de desempenho são medidas, não testes).
- Limite honesto: o critério 7 (os métodos de `OrderEntity` antes de `Entity.get_events`) se cumpre, mas por serem vizinhos (nível 2); no nível 1 do `order` há só módulos. Módulos pesam como símbolos de nome na cota; uma TASK futura pode decidir um peso ou limite próprio.
- Não verificado: `brief` no repositório do humano com o binário novo instalado; arestas aproximadas no grau; `serve`/MCP (o `brief` é a mesma função).
- Impacto: STATE=ENGINEERING concluída; falta o gate para CODE_REVIEW.

### [2026-11-09T05:40-03:00] — code review Aprovado c/ ressalvas
- Revisado 1ac7ff55 (`73e522f..1ac7ff5`, 3 commits lidos); 🔴=0, 🟡=2 (A1 comentário fora do lugar, A2 invariante do §5 desalinhada com a cota), 🟢=3, ⚖️=1; N_PARTITE gravado (frame manual, sem CALL da skill); GRILL não chamado (sem DRIFT de intenção e sem achado dependente de intenção).
- Estado: segue `in-progress`; DONE depende do `1` do humano. A1 e A2 entram antes (ENGINEERING) ou ficam como dívida declarada.

### [2026-11-09T06:00-03:00] — DONE
- Aprovação do humano (`1`) ao veredito "Aprovado c/ ressalvas" de 1ac7ff55; Estado: done. Escolhida a opção de declarar a dívida, não a de corrigir A1 e A2 antes.
- Dívida declarada, sem prazo: A1 (comentário de `brief_weighted` sobre `type Ranked`, mover), A2 (reescrever a invariante do §5 com a cota), A3 (`brief_weighted` longa), A4 (`score` não monotônico no JSON: documentar ou expor o nível), A5 (testes de aresta duplicada e de coincidência entre `neighbors()` e `link_degrees`), A6 (peso ou limite para módulos no nível 1; decidir com medição).
- Em aberto, não verificado: `brief` no repositório do humano com o binário novo instalado; `serve`/MCP com o `score` não monotônico; outros repositórios.

### [2026-11-09T06:20-03:00] — A1 e A2 resolvidas depois do DONE
- Tipo: alteração (dívida declarada)
- Contexto: autorização do humano (`1`) para corrigir A1 e A2; commit `docs: move the brief_weighted doc comment back to its function`.
- Decisão tomada: A1, o comentário `/// Como brief_with...` voltou para cima de `pub fn brief_weighted` (sem mudança de código); A2, a invariante do §5 foi reescrita para valer dentro da lista, respeitada a cota 10/5.
- Dívida restante: A3 a A6 (sem prazo). Estado segue `done`.

