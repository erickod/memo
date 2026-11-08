# TASK-5 — `brief`: símbolos centrais no primeiro nível, em vez de só módulos
branch: master
workdir: raiz do repositório (onde está o `Cargo.toml`)
created_at: 2026-11-08

## Status
- Estado: draft
- Owner: Erick Duarte
- ROLE requerida: eng (começa em DEFINITION; antes, uma medição sem alterar código)

> Esta TASK é um rascunho autônomo. Leia `tasks/README.md` (fluxo e como retomar), `README.md` e `docs/` (o que o `memo` faz e não faz). Nenhuma decisão foi tomada ainda; as decisões abertas estão no §9.

---

## 1. Objetivo
Fazer o primeiro nível do `memo brief` (os símbolos que carregam a palavra do pedido) listar as classes, funções e métodos centrais da palavra, em vez de uma lista de módulos, e devolver o `contorno` que hoje some. (REF: DOR#CRITERIA)

---

## 2. Contexto
- **O que é o `brief`:** dado um pedido em texto, devolve âncoras (palavras casadas), símbolos, contorno (quem chama e quem é chamado dos 5 primeiros símbolos de produção), padrão, escritores e notas. Está em `crates/memo-core/src/brief.rs` (`brief_weighted`). Ver `README.md` e a spec §8.5.
- **Estado atual do ranking (TASK-4, `done`):** dois níveis. Nível 1 = símbolos com pontuação direta (a âncora está no nome, na docstring de teste, ou o módulo a importa); nível 2 = símbolos que só chegam por salto de chamada ou tipo, com a pontuação dividida pelo grau. Das 15 vagas de produção, até 10 são do nível 1 e até 5 do nível 2 (`MAX_SYMBOLS`, `NEIGHBOR_SLOTS`). Dentro de um nível a ordem é pontuação decrescente, depois `qname`. A pontuação direta de um símbolo é a soma, por ocorrência da âncora no nome, do bônus do slot (`object` 1,0; `action` 0,7; demais 0,2); módulos que importam a âncora somam `peso_import × WEAK_SIGNAL` (0,6 × 0,2 = 0,12).
- **O problema observado**, no `brief order` de um repositório real de backend (801 arquivos; `order` está em 118 arquivos e tem 421 símbolos diretos):
  - os 10 lugares do nível 1 foram todos para módulos (`mod cancel_order_on_hold_failed [nome, import]`, `mod order`, ...); a classe `OrderEntity` não aparece;
  - o `contorno` ficou `nenhum`, porque os 5 primeiros símbolos de produção são módulos, e módulos não têm arestas de chamada;
  - os métodos de `OrderEntity` aparecem, mas só como vizinhos (nível 2, as 5 vagas).
- **Causa provável (hipótese, não testada):** (1) um módulo soma o nome (1,0) mais o import (0,12) e passa a classe, que só tem o nome (1,0); (2) entre os símbolos diretos quase todos empatam em 1,0 e o desempate é o `qname`, ou seja, alfabético, então o corte de 10 é arbitrário; falta um sinal de centralidade entre quem carrega a palavra.
- **Decisão anterior do humano que limita o escopo:** na TASK-4 ficou decidido manter módulos que importam a âncora no nível 1 (a alternativa de tratá-los como nível 2 foi recusada). Esta TASK pode propor outra forma de tratá-los, mas isso é uma nova decisão, não um erro da anterior.
- **Não classificar domínio:** o humano decidiu que o `memo` ainda não decide o que é conceito de domínio; o foco é interconexão. A solução não pode depender dessa classificação.
- **Como reproduzir as medições:** `docs/medicoes.md` e `scripts/measure/` (`salto_do_brief.py` reproduz a pontuação do `brief`; `representatividade.py` mede fração por arquivo, por contexto e entre palavras). O repositório real é privado; num repositório com a mesma estrutura, configure `context_root` no `memo.toml`.
- Stack: Rust, SQLite, as mesmas dependências das TASKs anteriores.

---

## 3. Escopo (rascunho)

### Dentro do escopo
- **Etapa 0, medição, sem alterar código:** medir no repositório real, para `order` e para outros pedidos (`reservation`, `payment`, `cancel order`, `hold`, `account`), o efeito de três candidatos: (a) peso menor para módulos na pontuação direta; (b) limite de módulos no nível 1; (c) desempate do nível 1 pelo número de símbolos da âncora que apontam para cada símbolo (centralidade dentro do grupo da âncora).
- Depois da medição, uma DEFINITION com as decisões abertas, e a implementação do que o humano escolher.
- Spec (§8.5) e README.

### Fora do escopo
- Decidir se uma palavra é de domínio.
- Mudar pontes, esqueleto, a ficha `query word`, o hash de config.
- Chave no `memo.toml` para os pesos (só se o humano decidir na DEFINITION).

---

## 4. Critérios de Aceitação (rascunho; a refinar depois da medição)
- [ ] no `brief order` do repositório real, ao menos uma classe ou função central (como `OrderEntity`) aparece no nível 1, e o `contorno` deixa de vir vazio (medida)
- [ ] o resultado do `brief` continua determinístico, byte a byte
- [ ] o `brief` continua em até 500 ms no repositório sintético de `docs/medicoes.md` e nos testes de golden revistos um a um (hoje o `brief` leva 0,66 s no gerador versionado, acima da meta; a causa não foi investigada e não faz parte desta TASK)
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` sem falhas

---

## 5. Restrições e Regras
- técnicas: princípios 1 a 7 da spec §2 (determinístico, saída compacta, cobertura explícita, falha parcial não é falha); `memo-core` não depende de `clap` nem de MCP.
- negócio: o `memo` entrega fatos, não julga código nem classifica domínio.
- dependências obrigatórias: TASK-4 concluída (`done`).
- regra de dados: sem mudança de esquema; o banco é derivado.
- invariante herdada (TASK-4, §5): dentro da lista, um símbolo com pontuação direta vem antes de um que só chegou por salto, na mesma seção, respeitada a cota de vagas.

---

## 6. Impacto em Dados
- [x] Nenhuma alteração de schema
- [ ] Uso de entidades existentes
- [x] Leitura apenas
- [ ] Escrita controlada

---

## 7. Estratégia de Testes
escritor: test-analyst (preenchido em TEST_ANALYSIS, depois da DEFINITION)

---

## 8. Dependências
- tasks predecessoras: TASK-4
- módulos | serviços externos: nenhum
- flags | configurações: a definir

---

## 9. Observações

### Decisões (grill-me)
(nenhuma ainda)
- Aberta Q1: qual candidato (a, b, c) ou combinação, depois da medição da etapa 0.
- Aberta Q2: o peso dos módulos (ou o limite) vira chave do `memo.toml`?
- Aberta Q3: a cota 10/5 muda?
- Aberta Q4: o `score` do JSON deve ganhar o campo do nível (dívida A4 da TASK-4)?

### Premissas
- [DEFINITION] A causa é hipótese até a etapa 0 medir. A medição pode contradizer a hipótese (nas TASKs 3 e 4, duas hipóteses iniciais foram desmentidas pela medição antes de qualquer código).

### Plano de Execução (rascunho)
1. Etapa 0: medir os candidatos (a), (b) e (c) no repositório real e no sintético, sem alterar código; registrar os números no log — depende de: NONE
2. DEFINITION: decisões Q1 a Q4 com o humano; N_PARTITE; critérios finais; plano — depende de: 1
3. TEST_ANALYSIS: §7 — depende de: 2
4. ENGINEERING: testes red, depois green — depende de: 3
5. CODE_REVIEW — depende de: 4

### Code Review
(vazio)

## 10. Log de Decisões e Alterações

### [2026-11-08T09:00-03:00] — Criação do rascunho da TASK-5
- Tipo: criação
- Contexto: depois do DONE da TASK-4, o humano rodou o `memo brief order` no repositório real e viu os 10 símbolos do nível 1 todos módulos e o `contorno` vazio; a TASK foi aberta como rascunho para o humano continuar depois, e o histórico das conversas não é necessário para retomá-la.
- Decisão tomada: nenhuma; a etapa 0 (medição) vem antes de qualquer definição.
- Impacto: STATE=DEFINITION não iniciada; `Estado: draft`.
- Não verificado: a causa provável, e se os candidatos (a), (b) e (c) resolvem sem piorar outros pedidos.
