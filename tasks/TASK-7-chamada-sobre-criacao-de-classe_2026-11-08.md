# TASK-7 — Resolver `Classe(...).metodo()` para `Classe.metodo`
branch: feat/task-6-word-item-layer (mesmo PR da TASK-6)
workdir: raiz do repositório (onde está o `Cargo.toml`)
created_at: 2026-11-08

## Status
- Estado: review (implementado e medido; falta a revisão de código)
- Owner: Erick Duarte
- ROLE requerida: eng

---

## 1. Objetivo
Quando o receptor de uma chamada é a criação de uma classe na mesma expressão (`Classe(args).metodo()`), ligar a chamada ao método da classe criada com uma aresta `CHAMA ~`, sem inferência nova: os dados já estão no banco. (REF: DOR#CRITERIA)

---

## 2. Contexto
- Origem: o `impact` do bope perdia consumidores como `await Repositorio(session).run(...)`. O extrator já emite a chamada como `?.run` (receptor que não é nome) e a criação como outra `CHAMA` na mesma linha do mesmo símbolo; faltava ligá-las.
- Correção de um diagnóstico errado, registrada para não se repetir: afirmei que a chamada `.run` não era extraída. Ela é (`('CHAMA', '?.run', None, 4801, 0)` no banco do bope). O extrator não precisou mudar.
- Ligação local (`x = Classe(...); x.m()`) já existe no extrator (`collect_locals`); não é parte desta task.

---

## 3. Escopo
### Dentro do escopo
- Regra no resolvedor (`created_method`, `resolve.rs`): para `?.metodo`, considerar as criações de classe (`CRIA`) na mesma linha do mesmo símbolo; se exatamente uma classe (herança incluída) tem o método, criar `CHAMA ~` para ele.
- `schema_version` 3 → 4, para índices existentes serem re-resolvidos.

### Fora do escopo
- Acesso a atributo de classe (`OrderStatus.X`): não é chamada, não vira `raw_ref`; etapa própria.
- Expressão que cruza várias linhas com a criação em outra linha.
- Falsos positivos da regra 7 (nome único no repositório), vistos na medição.

---

## 4. Critérios de Aceitação
- [x] `Alpha(1).run()` com `run` em `Alpha` e `Beta` gera `CHAMA ~` só para `Alpha.run`
- [x] método herdado (`Child().run()` com `run` só em `Base`) resolve para `Base.run`
- [x] duas criações na linha com o método (`Alpha(Beta()).run()`) não geram aresta e a chamada segue não resolvida
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas (314)

---

## 5. Restrições e Regras
- técnicas: princípios 1 a 7 da spec §2; aresta sempre aproximada (`~`); `memo-core` não depende de `clap` nem de MCP.
- dependências obrigatórias: TASK-6.
- regra de dados: banco derivado; `schema_version` sobe para reindexar.

---

## 9. Observações
### Medição no bope (binário `--release`, índice recriado, uma execução)
- Não resolvidas: 1385 → 1193 (−192); aproximadas: 2105 → 2297 (+192).
- `?.metodo` não resolvidos: 228 → 36.
- `GetOrdersAndTaxesByIdempotencyKeyRepository.run`: o ponto `service.py:4801` passou a ser chamador d1 (e `process_debt_fees`, d2); `chamadores` foi de 2 para 4.
- Precisão: de 540 `?.metodo` resolvidos com aresta de método, 451 têm `Classe(` da própria aresta na linha; os 89 restantes que olhei (`.label()` → `OrderStatus.label`, `super().__init__`) vêm da regra 7 antiga, não desta. A precisão da regra nova isolada não foi medida.
- Índice completo do bope: 35,7 s (1.017 arquivos).

### Pendências
- `por palavra` do `…Repository.run` ainda tem 160 itens (nome do alvo só `run`); exigir a palavra da classe dona para métodos fecha a Q6 da TASK-6.
- A regra 7 liga métodos de bibliotecas (`label`, `__init__`) a símbolos do repositório de mesmo nome; precisa de critério.
- Acesso a atributo de classe, etapa seguinte.

## 10. Log de Decisões e Alterações
### [2026-11-08] — Regra do construtor
- Tipo: implementação
- Decisão tomada: resolver só quando exatamente uma classe criada na linha tem o método; o resto segue não resolvido (e coberto por palavra).
- Impacto: `schema_version` 4.
