# Como reproduzir as medições

As TASKs em `tasks/` citam medições feitas em dois lugares que não estão no repositório: um repositório sintético de 5.000 arquivos e uma cópia de um repositório real de backend (privado). Esta página diz como refazê-las, com os scripts de `scripts/measure/`. Todos só leem; nenhum altera o repositório medido.

## Pré-requisitos

- Rust estável (`cargo build --release` gera `target/release/memo`), `git` e Python 3.
- Para repositórios com `app/<contexto>/<camada>`, a raiz do contexto vem de `context_root` no `memo.toml`. O padrão é `app`.

## Repositório sintético

```bash
python3 scripts/measure/gen_repo.py /tmp/memo-syn --files 5000
cd /tmp/memo-syn
/usr/bin/time -f "índice completo: %es" /caminho/para/target/release/memo index
/usr/bin/time -f "sem mudança:     %es" /caminho/para/target/release/memo index
/usr/bin/time -f "brief:           %es" /caminho/para/target/release/memo brief "calculate total limit"
/usr/bin/time -f "query word:      %es" /caminho/para/target/release/memo query word limit
```

**Atenção: este gerador não é o que produziu os números das TASKs.** O gerador original (em `/tmp`) se perdeu. O atual é mais leve e deixa `calculate`, `total` e `limit` em quase todo arquivo. Medido em 2026-11-08, uma execução, binário `--release`:

| | gerador atual | número registrado nas TASKs (gerador perdido) |
| --- | --- | --- |
| índice completo | 1,5 s | 42 a 44 s |
| reindexação sem mudança | 0,04 s | 0,04 s |
| `brief "calculate total limit"` | 0,66 s | 0,36 a 0,38 s |
| `query word limit` | 1,5 s | 2,3 s |

Não compare um número de um gerador com o do outro. Para ver o efeito de uma mudança, meça antes e depois **com o mesmo gerador**. O `brief` em 0,66 s está acima da meta de 500 ms da spec (§11): com este gerador, em que toda palavra do pedido aparece em todos os arquivos, é o pior caso para o `brief`. A causa não foi investigada.

## Repositório real

Os números "do repositório real" (801 arquivos, 6.012 símbolos, `order` em 118 arquivos) vêm de um backend privado com a estrutura `backend/easy_lease/<contexto>/<camada>`. Se você tiver um repositório parecido:

```bash
git clone <seu-repositorio> /tmp/real && cd /tmp/real
mkdir -p .git/memo && printf 'context_root = "backend/seu_pacote"\n' > .git/memo/memo.toml
memo index
memo brief order
memo query word order
```

Sem o `context_root` certo, o nome do pacote vira palavra (`easy`, `lease`, `backend`) e polui as pontes de import.

## Scripts de análise (leem o banco)

O banco fica em `$(git rev-parse --git-common-dir)/memo/base.db`. Rode `memo index` antes.

- `scripts/measure/representatividade.py <repo> <palavra>... [--vs <outra>]`: fração dos arquivos que têm a palavra, fração por contexto e concentração entre contextos, e `P(a|b)` e `P(b|a)` com `--vs`. Foi com ele que se viu que a concentração entre contextos separa palavras de domínio das de infraestrutura e que a coerência não separa.
- `scripts/measure/salto_do_brief.py <repo> <palavra>`: reproduz a pontuação direta e a por salto do `brief` (antes e depois de dividir pelo grau do vizinho). Foi com ele que se viu um hub somar 20,0 contra 2,0 dos símbolos que carregam a palavra.

Resultados no repositório real, para comparar com o seu (`order`): 118 de 801 arquivos (0,15); contexto `orders` 70/188, `booking` 32/128, `tests` 9/39; `P(order|reservation)` 0,65 e `P(reservation|order)` 0,39; coerência de `order` 0,21 contra 0,48 de `id`.
