"""Precisão das arestas de método vindas de `?.metodo` (TASK-7): só lê o índice e o código.

Uso: python3 -I scripts/measure/precisao_chamada_criacao.py <raiz-do-repo-indexado>

Para cada `raw_ref` `?.metodo` resolvida com aresta `CHAMA` a um método do mesmo nome, confere se
a linha da chamada tem `Classe(` da classe dona da aresta. É checagem fraca (o nome na linha).
Mistura todas as regras que resolvem `?.metodo`, não só a do construtor: os desvios que sobram
costumam vir da regra 7 (nome único no repositório), como `.label()` ligado a um enum.
"""
import re
import sqlite3
import sys

root = sys.argv[1]
db = sqlite3.connect(f"file:{root}/.git/memo/base.db?mode=ro", uri=True)
print("?. total e não resolvidas:", db.execute(
    "select count(*), sum(resolved = 0) from raw_ref where callee_text like '?.%'").fetchone())
rows = db.execute("""
    select r.line, f.path, d.qname, e.approx
    from raw_ref r
    join symbol s on s.id = r.src
    join file f on f.id = s.file_id
    join edge e on e.src = r.src and e.line = r.line and e.kind = 'CHAMA' and e.dst is not null
    join symbol d on d.id = e.dst and d.kind = 'method' and d.name = substr(r.callee_text, 3)
    where r.callee_text like '?.%' and r.resolved = 1""").fetchall()
ok = 0
misses = []
cache = {}
for line, path, qname, _approx in rows:
    cls = qname.split(".")[-2]
    text = cache.setdefault(path, open(f"{root}/{path}", errors="ignore").read().splitlines())
    src_line = text[line - 1] if 0 < line <= len(text) else ""
    if re.search(rf"\b{re.escape(cls)}\s*\(", src_line):
        ok += 1
    else:
        misses.append((path, line, qname, src_line.strip()[:70]))
print(f"resolvidas com aresta de método: {len(rows)} | com 'Classe(' na linha: {ok} | sem: {len(misses)}")
for miss in misses[:10]:
    print(" ", miss)
