# tasks/: histórico e como retomar o trabalho

Estas são as TASKs do `memo`, em ordem. Cada arquivo é um registro completo e autônomo de uma etapa do projeto: objetivo, contexto, decisões, testes, plano e um log append-only do que foi feito e verificado. Para retomar, **não é preciso nenhuma conversa anterior**: leia este arquivo, o `README.md` da raiz e `docs/`.

## O projeto em cinco linhas

`memo` é um CLI em Rust que indexa um repositório Python como grafo de palavras, símbolos e chamadas, responde consultas sobre esse grafo (`brief`, `query ...`) e guarda notas humanas ancoradas nos símbolos, para dar a um agente de IA contexto compacto e verificável. Entrega fatos e não julga código. O estado fica fora da árvore versionada e é descartável. Contrato completo: `Spec CLI de memória estrutural para IA.md`. O que faz, o que não faz e para onde pode ir: `docs/funcionalidades.md`, `docs/limitacoes.md` e `docs/caminhos.md`. Como refazer as medições: `docs/medicoes.md`.

## As TASKs

| TASK | Estado | O que entregou | Commits |
| --- | --- | --- | --- |
| [TASK-1](TASK-1-cli-memoria-estrutural-ia_2026-11-07.md) | done | O MVP: extração Python, grafo, consultas, notas, eventos, features por task, `init`, `serve` (MCP), testes de aceitação A1 a A14 | histórico até `a7aa04d` (README); correções `ad9ab1f` e outras |
| [TASK-2](TASK-2-grafo-de-palavras-com-pontes_2026-11-07.md) | done | A palavra como conceito: fontes de palavras (nome, diretório, import, docstring de teste), pontes tipadas com pesos, coerência, ficha `query word`, `--no-tests`, evidência de import, aliases; correção de desempenho | `3b6c54f` a `94dd5e5` e `dfc4077` |
| [TASK-3](TASK-3-coerencia-sem-ruido-e-config-no-indice_2026-11-08.md) | done | Esqueleto só pela lista `skeleton`; índice que reindexa quando a config muda (hash em `meta`); representatividade em arquivos na ficha `query word` | `e7ff342` a `7a21ca5`, `73e522f` |
| [TASK-4](TASK-4-brief-sem-dominancia-de-hubs_2026-11-09.md) | done | `brief` com dois níveis (quem carrega a âncora antes; vizinhos depois, com cota 10/5) e amortecimento do salto pelo grau do vizinho | `a691499` a `d0871b0` |
| [TASK-5](TASK-5-brief-centralidade-no-nivel-1_2026-11-08.md) | **draft** | Nada ainda. Rascunho: o nível 1 do `brief order` virou uma lista de módulos; falta medir três candidatos antes de definir | nenhum |
| [TASK-6](TASK-6-camada-relacional-palavra-item_2026-11-08.md) | **review** | Camada relacional palavra ↔ item: cada uso (`raw_ref`) é dono das próprias palavras, gravadas por arquivo e independentes da resolução; `impact` com a seção `por palavra` aditiva (tudo o que contém o nome do alvo, por uso e por tipo, com linha e evidência como rótulo). Três rodadas: PR #1 (mergeado sem revisão) e a branch `feat/task-6-completion` (rodadas 2 e 3); falta a re-revisão da rodada 3 | `12359d6` a `819c122` (PR #1); `d8d95f8` a `e916d5d` e docs seguintes |
| [TASK-7](TASK-7-localizador-compacto_2026-11-08.md) | **DEFINITION** | O memo como localizador: notas inteiras pelo MCP, testes ocultos por padrão, listas longas agrupadas, seção `ler:`, `LE` para `X.attr`, membros de Enum, `query word` composta | — |
| [TASK-8](TASK-8-estrutura-e-relevancia_2026-11-08.md) | **CODE_REVIEW** | Chamadas encadeadas, escritas de atributo/SQLAlchemy, âncoras estruturais, contexto base × feature, ranking por evidência e trechos sem sobreposição; 395 testes aprovados | `e97106d` (branch `feat/structural-query-relevance`) |
| [TASK-9](TASK-9-estrategias-de-relevancia_2026-11-08.md) | **CODE_REVIEW** | Relevância com estratégias e pesos no output (vizinhança até 3 saltos com 1/d, caminho, conexão, nome exato), âncoras do `brief` pela interseção das palavras, detector de nome genérico, entrypoints HTTP, testes via HTTP, homônimos ligados como grupo; 417 testes | rodada 2: `a94e76f`, branch `feat/word-intersection-anchors` |
| [TASK-10](TASK-10-simbolos-variaveis-mutacoes-estaticos_2026-11-09.md) | done | Variáveis de módulo e de classe como símbolos, `ESCREVE`/`LE` de nomes simples (incl. `global`, sem `nonlocal`), `impact` com `escritores`/`leitores`, palavras do corpo (identificadores, comentários, docstring, decoradores, bases, `lambda`, definições aninhadas) ligadas ao símbolo com a etiqueta `corpo`; 467 testes | PR #5, merge `a74191d` |

(Os nomes dos arquivos têm datas de criação; a TASK-4 saiu com a data 2026-11-09 e a TASK-5 com 2026-11-08 por causa do relógio da máquina na época. A numeração é a ordem real.)

## Próximo passo

Primeiro, a **CODE_REVIEW da TASK-6** (branch `feat/task-6-completion`; ver o §10 dela). Depois, a **TASK-5**. A etapa 0 dela é só medir, sem alterar código: ver o §3 e o §9 dela, `docs/medicoes.md` e `scripts/measure/`. Depois vem a DEFINITION, que precisa de decisões do dono do projeto (Q1 a Q4 no §9 da TASK-5).

Pendências conhecidas que cabem numa TASK ou numa limpeza: ver "Dívida técnica declarada" em `docs/limitacoes.md` (três delas citam as TASKs 2, 3 e 4 como origem) e o `brief` em 0,66 s no gerador versionado, acima da meta de 500 ms da spec (causa não investigada).

## Como se trabalha

**Build e testes.** Rust estável (há `rust-toolchain.toml`).

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --no-fail-fast        # 345 testes na última execução
cargo build --release            # target/release/memo
```

**Fluxo de uma TASK**, como foi praticado (estados `DEFINITION`, `TEST_ANALYSIS`, `ENGINEERING`, `CODE_REVIEW` e `DONE`):

1. **DEFINITION:** escrever o problema (não a solução), as decisões com o dono do projeto (cada uma com origem: `humano` ou recomendação aceita), os critérios de aceitação verificáveis, as restrições, o plano em etapas com gates e riscos, e uma análise de modelo (N_PARTITE: pares de elementos, ciclo de vida, invariantes). Termina com uma aprovação única.
2. **TEST_ANALYSIS:** definir no §7 os testes antes do código, com nome `test_given_..._when_..._then_...` (até 89 caracteres) e docstring GIVEN/WHEN/THEN em inglês, cada critério do §4 com ao menos um teste, e valores esperados que vêm do §4, do §5, das decisões ou de código verificado, nunca inventados.
3. **ENGINEERING:** cada alteração atômica é descrita (o quê, por quê, impacto, decisão) e **espera a aprovação do dono antes de ser aplicada**. Ordem: commit `test:` (red, falhando por asserção) e depois commit `feat:` (green). Teste obsoleto por uma decisão só é trocado com aval explícito, e golden file só é aceito depois de ler o diff. Conferir `fmt` e `clippy` antes de commitar.
4. **CODE_REVIEW:** revisão commit a commit (código, mensagem, testes e log), com achados 🔴 (bloqueia), 🟡, 🟢, ⚖️ (alternativas) e ❓ (pergunta), mais uma análise N_PARTITE. Nenhuma TASK vai a `done` com 🔴.
5. **DONE:** só com aprovação do dono; dívida aceita fica declarada no log.

**Convenções.**
- **Aprovação:** o dono responde `1` para aprovar o que foi proposto. Nada é inferido de silêncio.
- **Log append-only:** cada TASK tem no §10 um log que só cresce; não se reescreve o histórico. Correção entra como nova entrada.
- **Honestidade:** o log registra o que **não** foi verificado, os erros cometidos e o que ficou fora. Medir antes de decidir: nas TASKs 3 e 4, duas hipóteses iniciais foram desmentidas pela medição antes de qualquer código.
- **Commits:** mensagem em inglês no estilo `test:`, `feat:`, `fix:`, `docs:`, `chore:`, `perf:`, sem linha de atribuição (`Co-Authored-By`) por regra do dono do projeto; um commit por alteração atômica; sem commits em lote.
- **Escopo do `memo`:** fatos, não julgamento; não classifica o que é domínio.

**Observações sobre este material.**
- Os caminhos absolutos nos logs (`/var/home/erickod/...`) são da máquina do autor; a raiz do repositório é onde está o `Cargo.toml`.
- Os caminhos `/tmp/eli` e `/tmp/pr` citados nos logs eram cópias de trabalho (um repositório real privado e um repositório sintético) que não existem mais. Use `docs/medicoes.md` e `scripts/measure/` para refazer as medições; **o gerador sintético atual não é o que produziu os números registrados** (o original se perdeu), então compare só medições do mesmo gerador.
- As análises de modelo (N_PARTITE) das TASKs 3 e 4 foram escritas à mão, sem executar a skill correspondente. A TASK-2 declara o mesmo.
- O fluxo acima usa skills do dono do projeto (`agentsmd`, `definition`, `test-analyst`, `eng`, `code-reviewer`, `n-partite`, `grill-me`, `commiter`) que **não estão neste repositório**. Dá para seguir o fluxo à mão com a descrição acima; as TASKs trazem o resultado de cada fase.
- Nas TASKs 1 e 2, boa parte dos checkboxes `[ ]` das listas de testes do §7 nunca foi marcada, embora os testes existam e passem; a fonte de verdade é a suíte (`cargo test`) e o log do §10, não os checkboxes. A partir da TASK-3 os critérios do §4 são marcados.
- O formato dos arquivos de configuração de MCP (`.mcp.json`, `.codex/config.toml`) e o servidor `memo serve` nunca foram testados contra os clientes reais; ver `docs/limitacoes.md`.
