# TASK-9 — Relevância por estratégias com peso, entrypoints e testes por HTTP

## Status
- Estado: CODE_REVIEW
- ROLE requerida: code-reviewer
- Branch da rodada 2: feat/word-intersection-anchors; base master. PR #3 da primeira rodada já mergeado.
- Origem: atritos de uso no `bope` (ruído de `run` no `impact`, `testes exatos: nenhum` com testes de integração diretos, nome ambíguo resource → service) e pedido do humano para implementar as três correções, mostrar o peso de cada estratégia e testar uma quarta (nome exato).

## 1. Objetivo
Responder a um alvo explícito com o que está estruturalmente perto dele, ordenado por evidência visível, sem o ruído de nomes genéricos.

## 3. Escopo
1. Vizinhança exata até 2 saltos (mesma direção) e caminho de entrypoint → alvo, com conexão local como desempate e o peso de cada estratégia no output.
2. Homônimos ligados por chamada exata respondidos como grupo.
3. `is_entrypoint` e rota para decoradores HTTP literais e `entrypoint_decorators`; testes ligados por URL literal.
4. Nó de nome exato como evidência fraca, só para nome não genérico; detector de nome genérico.
Fora: prefixos de router, callbacks de fila registrados por chamada, calibração dos pesos em outros repositórios.

## 4. Critérios
- C1: `impact` mostra `nome:`, `pesos:` e `relevantes:` com o peso de cada estratégia e a soma; nome genérico resume os `só nome` (`--all` lista).
- C2: vizinhança não inclui irmãos ligados só por hub; conexão de hub fica abaixo de helper específico.
- C3: rota literal marca entrypoint; rota dinâmica não; `entrypoint_decorators` marca entrypoint sem rota.
- C4: requisição HTTP casa método e caminho (`{}` = segmento); várias rotas → `~`; rota nova em outro arquivo religa requisições existentes.
- C5: `query tests` e `impact` listam/contam `testes via HTTP`; testes ocultos aparecem por tipo.
- C6: homônimos ligados por chamada exata direta → grupo (CLI exit 0, JSON `grupo`); sem ligação → ambíguo.
- C7: `[strategies]` validado e refletido no output; não reindexa.
- C8: fmt, clippy, suíte completa; schema 8 reconstrói o grafo e preserva notas.

## 9. Decisões
- D1 (humano): implementar as três estratégias e mostrar os pesos de cada uma.
- D2 (humano, recomendação aceita): detector de genéricos sempre ativo; nome exato como evidência fraca (peso 0,25), só para nome não genérico.
- D3 (premissa medida): genérico por número de definições (P99, piso 4) ou usos (P99, piso 10) com menos da metade resolvida; o grau bruto foi descartado porque `normalize_national_id` (2 definições, 949 usos, todos resolvidos) seria marcado.
- D4 (ENGINEERING, medido): conexão como fração das ligações do candidato que tocam a região do alvo, sem normalizar pelo máximo; normalizar dava 0,50 a `get_session` (145 chamadores). Segundo salto só na mesma direção, para tirar irmãos de hubs.
- D5 (ENGINEERING): entrypoint como propriedade (`is_entrypoint`, `route`), não como novo `kind`: o símbolo continua sendo função.

- D6 (humano): rodada 2 na mesma branch: vizinhança até 3 saltos com peso 1/d (estratégia 6), âncoras pela interseção das palavras no `brief` (estratégia 5), método herda palavras da classe e `skeleton` desempata.
- D7 (ENGINEERING, medido): só tokens com forma de identificador viram identidade explícita; `lock`, `summary` e `recover` casavam com símbolos únicos (incluindo testes) e desligavam a interseção.
- D8 (ENGINEERING, medido): âncoras só entre as empatadas em cobertura total e cobertura pelo próprio nome; métodos `__x__` não herdam palavras. Sem isso, métodos auxiliares herdeiros (`round_suggested_credit_limit`, `__init__`) disputavam com o símbolo certo.

## Medições (bope, índice `c196887`, cópia temporária)
- Hipótese das três vias (offline, `scripts/measure/three_routes.py`): `brief` 71%/73%, `impact` 77%/77% (top 5/top 10); três vias 81–84%/92–94%; grau global 69%/81%; chamadas exatas até 2 saltos 96%/100%, sem falsos positivos conhecidos.
- Nome exato: 744 usos de produção sem chamada exata a nome do repositório; 699 de nomes genéricos; 45 restantes, ~11/15 corretos numa amostra (erros: homônimos de bibliotecas externas). Nos 8 casos, nada acrescentado.
- Implementação: `relevantes` do `impact` 94,6%/100%, 0 falsos positivos conhecidos; `GetGuaranteedSaleBlockPoliciesRepository.run`: 245 `só nome` → 1 linha, service e endpoint no topo, 9 testes via HTTP; 57 entrypoints HTTP, 544 requisições exatas e 48 `~`. `impact` em 0,12 s.
- Limite: 8 casos e referência montada pelo agente; um repositório.
- Rodada 2 (`brief` antigo × novo, top 5* / top 10*, *normalizado pelo teto): desenvolvimento (10) 40/48 → 75/81%, âncora certa 10/10; holdout (6) 42/42 → 89/89%, 6/6; conjunto novo (6, congelado antes da implementação e nunca usado para ajuste) 27/41 → 72/86%, 6/6; fluxos longos (8) 15/20 → 17/22%, 5/8. D7 e D8 vieram da inspeção do desenvolvimento e dos fluxos longos; o holdout já tinha revelado a herança de palavras.
- Decaimento por nível (offline): até 3 saltos empata nos fluxos curtos e eleva o alcance nos longos de 65% para 87%; 4 ou mais saltos, decaimento geométrico e soma entre âncoras não ganharam.

## 10. Log
- 2026-11-08 — DEFINITION/TEST_ANALYSIS pela conversa: escopo e D1–D2 decididos pelo humano; hipóteses medidas antes da implementação.
- 2026-11-08 — ENGINEERING: implementação; 12 testes de núcleo e 5 de CLI novos; testes escritos depois do código, conferidos por mutação (re-resolução global e filtro de irmãos removidos → 5 falhas). Três snapshots atualizados após leitura (linhas de nome, pesos, relevantes e testes ocultos). Bug achado e corrigido: etiqueta `nome` de ponte colidia com a estratégia `nome` no motivo de `ler:`.
- 2026-11-08 — ENGINEERING → CODE_REVIEW: 412 testes, clippy -D warnings e fmt sem erros. Binário global não atualizado; o índice do `bope` não foi tocado (medições numa cópia em /tmp).
- 2026-11-08 — Rodada 2 (D6–D8): 3 saltos com 1/d, âncoras pela interseção, herança de palavras da classe, desempate por `skeleton`, tokens soltos não viram identidade. 5 testes novos, conferidos por mutação (o do desempate foi refeito porque a primeira fixture não o isolava). Golden do `brief "calculate limit"` atualizado após leitura (âncora `CalculateLimitUseCase.execute`, relevantes e contorno). 417 testes, clippy e fmt sem erros.
- 2026-11-09T07:49:12-03:00 — Publicação da rodada 2 autorizada pelo humano. PR #3 já mergeado em master (4a83486); branch feat/word-intersection-anchors criada para PR independente. Implementação, testes e script de medição registrados em a94e76f. Validação repetida: 417 testes do workspace e 5 controles Python aprovados, fmt/clippy/diff-check sem erros. Arquivo local tasks/memo.md fora dos commits; sem merge automático.
