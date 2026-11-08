#!/usr/bin/env python3
"""Representatividade de palavras em arquivos, lendo o banco do `memo` (somente leitura).

Uso: representatividade.py <repo> <palavra> [<palavra> ...] [--vs <outra>]

Para cada palavra: fração dos arquivos do repositório que a têm, fração por contexto (e o quanto a
distribuição é concentrada: maior fração sobre a média, só contextos com 30 arquivos ou mais) e,
com `--vs`, `P(palavra|outra)` e `P(outra|palavra)`. Foi com isto que se mediu, no repositório real,
que a concentração entre contextos separa palavras de domínio das de infraestrutura e que a
coerência (top 10 vizinhas sobre o total) não separa. A unidade é o arquivo; só nomes de símbolos
contam. O banco precisa existir: rode `memo index` antes.
"""
import argparse
import collections
import sqlite3
import subprocess
from pathlib import Path


def open_db(repo: str) -> sqlite3.Connection:
    common = subprocess.check_output(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], cwd=repo, text=True
    ).strip()
    return sqlite3.connect(f"file:{Path(common) / 'memo' / 'base.db'}?mode=ro", uri=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("repo")
    parser.add_argument("words", nargs="+")
    parser.add_argument("--vs", help="outra palavra, para P(a|b) e P(b|a)")
    args = parser.parse_args()
    c = open_db(args.repo)
    text = dict(c.execute("select id, text from word"))
    wid = {v: k for k, v in text.items()}
    context = {f: cx for f, cx in c.execute("select id, context from file")}
    total = len(context)
    by_context = collections.defaultdict(set)
    for f, cx in context.items():
        if cx:
            by_context[cx].add(f)
    big = [k for k, v in by_context.items() if len(v) >= 30]
    files = collections.defaultdict(set)
    for w, f in c.execute(
        "select o.word_id, s.file_id from occurrence o join symbol s on s.id = o.owner_id"
        " where o.owner_kind = 'symbol' and o.source = 'name'"
    ):
        files[w].add(f)
    print(f"arquivos: {total}; contextos com 30 arquivos ou mais: {', '.join(sorted(big)) or 'nenhum'}")
    for word in args.words:
        if word not in wid:
            print(f"{word}: não existe no índice")
            continue
        a = files[wid[word]]
        shares = [len(a & by_context[k]) / len(by_context[k]) for k in sorted(big)]
        mean = sum(shares) / len(shares) if shares else 0
        conc = max(shares) / mean if mean else 0
        parts = " ".join(f"{k}={s:.2f}" for k, s in zip(sorted(big), shares))
        print(f"{word}: {len(a)}/{total} ({len(a) / total:.2f}); concentração {conc:.1f}; {parts}")
        if args.vs and args.vs in wid:
            b = files[wid[args.vs]]
            co = len(a & b)
            if a and b:
                print(f"  P({word}|{args.vs})={co / len(b):.2f} P({args.vs}|{word})={co / len(a):.2f} comum={co}")


if __name__ == "__main__":
    main()
