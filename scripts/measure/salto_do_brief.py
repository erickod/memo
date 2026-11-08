#!/usr/bin/env python3
"""Reproduz a pontuação do `brief` para uma palavra, lendo o banco do `memo` (somente leitura).

Uso: salto_do_brief.py <repo> <palavra>

Mostra, para a palavra como âncora única, os símbolos com pontuação direta (soma do bônus do slot:
`object` 1,0, `action` 0,7, demais 0,2) e a pontuação por salto que cada vizinho acumula
(fator 0,5 por aresta CHAMA, RECEBE ou RETORNA, nos dois sentidos), antes e depois de dividir
pelo grau do vizinho. Foi com isto que se viu que um hub (`Entity.get_events`) somava 20,0 contra
no máximo 2,0 de qualquer símbolo que carrega `order` (TASK-4). Não inclui as pontuações de
docstring de teste e de import, que o `brief` também soma.
"""
import collections
import sqlite3
import subprocess
import sys
from pathlib import Path

HOP = 0.5
KINDS = "('CHAMA', 'RECEBE', 'RETORNA')"


def main() -> None:
    repo, word = sys.argv[1], sys.argv[2]
    common = subprocess.check_output(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], cwd=repo, text=True
    ).strip()
    c = sqlite3.connect(f"file:{Path(common) / 'memo' / 'base.db'}?mode=ro", uri=True)
    row = c.execute("select id from word where text = ?", (word,)).fetchone()
    if not row:
        sys.exit(f"{word}: não existe no índice")
    bonus = {"object": 1.0, "action": 0.7}
    direct = collections.defaultdict(float)
    for sid, slot in c.execute(
        "select symbol_id, slot from occurrence where word_id = ? and symbol_id is not null", (row[0],)
    ):
        direct[sid] += bonus.get(slot, 0.2)
    degree = collections.Counter()
    for src, dst in c.execute(f"select src, dst from edge where dst is not null and kind in {KINDS}"):
        degree[src] += 1
        degree[dst] += 1
    hop = collections.defaultdict(float)
    for sid, score in direct.items():
        for (dst,) in c.execute(
            f"select dst from edge where src = ? and dst is not null and kind in {KINDS}", (sid,)
        ):
            hop[dst] += HOP * score
        for (src,) in c.execute(f"select src from edge where dst = ? and kind in {KINDS}", (sid,)):
            hop[src] += HOP * score
    name = dict(c.execute("select id, qname from symbol"))
    print(f"símbolos diretos: {len(direct)}; soma {sum(direct.values()):.1f}; máximo {max(direct.values(), default=0):.1f}")
    print("\ntop por salto acumulado (sem amortecer -> dividido pelo grau):")
    for sid, v in sorted(hop.items(), key=lambda kv: -kv[1])[:8]:
        print(f"  {v:6.1f} -> {v / max(degree[sid], 1):5.2f}  grau {degree[sid]:4d}  {name[sid]}  direto={direct.get(sid, 0)}")
    print("\ntop diretos:")
    for sid, v in sorted(direct.items(), key=lambda kv: (-kv[1], name[kv[0]]))[:5]:
        print(f"  {v:6.1f}  {name[sid]}")


if __name__ == "__main__":
    main()
