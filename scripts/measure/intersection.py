#!/usr/bin/env python3
"""Offline test of query-word intersection for multi-word brief requests.

Usage: intersection.py <repo> <manifest.json> <output.json> --binary <memo>

Opens the repository's memo base graph through a disposable backup (the original is never
written). Query words are matched as `memo brief` does: lowercase letter runs, stopwords out,
exact word or a shared prefix of at least 5 letters, `skeleton` words ignored. The manifest's
expected symbols are only used for scoring, never for ranking.

Strategies (each falls back to the current brief order after its own candidates):
  name        symbols whose own name covers the most query words (level 1)
  call        + words covered by exact call neighbors at half weight (level 2)
  file        + words covered by other symbols of the same file at quarter weight (level 3)
  *_idf       the same, each word weighted by log(N / symbols carrying it)
  name_rel    best `name` hit, then its exact two-hop call neighbors, then `name`
  anchors_rel up to 3 callables with the maximum own-name coverage (fewest extra words first),
              then the union of their same-direction two-hop exact neighbors ranked like
              `impact`: 1/depth + 0.5 * share of the neighbor's links that touch the region
"""

import argparse
import collections
import json
import math
import re
import shutil
import sqlite3
import subprocess
import tempfile
from pathlib import Path

STOPWORDS = set("""de da do das dos em na no nas nos um uma uns umas os as ao aos pelo pela pelos pelas
por para com sem que se ou ser ter mais como mas nao sao ha foi isso esse essa este esta seu sua ele
ela ja so the an of to in on at for with and is are be it this that by from as not or was were has
have its into""".split())
MIN_PREFIX = 5


def tokenize(text):
    tokens = []
    for t in re.findall(r"[^\W\d_]+", text.lower()):
        if len(t) >= 2 and t not in STOPWORDS and t not in tokens:
            tokens.append(t)
    return tokens


class Index:
    def __init__(self, c):
        self.sym = {}
        for sid, q, kind, test, path, fid in c.execute(
            "SELECT s.id,s.qname,s.kind,s.is_test,coalesce(f.path,''),s.file_id FROM symbol s "
            "LEFT JOIN file f ON f.id=s.file_id"
        ):
            self.sym[sid] = dict(q=q, kind=kind, file=fid,
                                 test=bool(test) or path.startswith("tests/") or "/tests/" in path)
        self.by_q = {v["q"]: k for k, v in self.sym.items()}
        self.words = collections.defaultdict(set)   # symbol -> word ids (own name)
        self.carriers = collections.defaultdict(set)  # word id -> symbols
        for sid, wid in c.execute("SELECT symbol_id, word_id FROM occurrence WHERE source='name' AND symbol_id IS NOT NULL"):
            self.words[sid].add(wid)
            self.carriers[wid].add(sid)
        self.text = dict(c.execute("SELECT id, text FROM word"))
        self.skeleton = {w for (w,) in c.execute("SELECT word_id FROM word_stats WHERE esqueleto=1")}
        self.nb = collections.defaultdict(set)
        self.fw = collections.defaultdict(set)
        self.rv = collections.defaultdict(set)
        for s, d in c.execute("SELECT src,dst FROM edge WHERE kind IN ('CHAMA','CRIA') AND approx=0 AND dst IS NOT NULL AND src!=dst"):
            self.nb[s].add(d)
            self.nb[d].add(s)
            self.fw[s].add(d)
            self.rv[d].add(s)
        self.by_file = collections.defaultdict(set)
        for sid, s in self.sym.items():
            if s["file"] is not None:
                self.by_file[s["file"]].add(sid)
        self.n = len(self.sym)

    def prod(self, sid):
        return not self.sym[sid]["test"]

    def callable(self, sid):
        return self.prod(sid) and self.sym[sid]["kind"] in ("function", "method")

    def match(self, tokens):
        """Each token -> set of word ids (exact, else prefix >= 5), skeleton removed."""
        out = []
        for t in tokens:
            exact = {w for w, x in self.text.items() if x == t and self.carriers.get(w)}
            ws = exact or {w for w, x in self.text.items() if self.carriers.get(w) and
                           min(len(x), len(t)) >= MIN_PREFIX and (x.startswith(t) or t.startswith(x))}
            ws -= self.skeleton
            if ws:
                out.append(ws)
        return out

    def idf(self, group):
        df = len(set().union(*(self.carriers[w] for w in group)))
        return math.log(self.n / max(df, 1))


def rank(ix, groups, level, use_idf):
    weight = [ix.idf(g) if use_idf else 1.0 for g in groups]
    covers = lambda wordset: [i for i, g in enumerate(groups) if g & wordset]
    cands = set().union(*(ix.carriers[w] for g in groups for w in g)) if groups else set()
    cands = {s for s in cands if ix.prod(s)}
    scored = []
    for s in cands:
        own = set(covers(ix.words[s]))
        score = sum(weight[i] for i in own)
        if level >= 2:
            near = set()
            for n in ix.nb[s]:
                if ix.prod(n):
                    near |= set(covers(ix.words[n]))
            score += 0.5 * sum(weight[i] for i in near - own)
            own |= near
        if level >= 3:
            same = set()
            for n in ix.by_file.get(ix.sym[s]["file"], ()):
                if n != s:
                    same |= set(covers(ix.words[n]))
            score += 0.25 * sum(weight[i] for i in same - own)
        # Fewer extra name words = the name is more about the request.
        extra = len(ix.words[s]) - len(covers(ix.words[s]))
        scored.append((-score, extra, ix.sym[s]["q"], s))
    return [s for *_, s in sorted(scored)]


def two_hops(ix, target):
    seen, front, out = {target}, [target], []
    for _ in range(2):
        nxt = []
        for n in front:
            for m in sorted(ix.nb[n], key=lambda m: ix.sym[m]["q"]):
                if m not in seen and ix.prod(m):
                    seen.add(m); nxt.append(m); out.append(m)
        front = nxt
    return out


def anchors_rel(ix, groups, fallback, max_depth=2, decay=lambda d: 1 / d, accumulate=False):
    covers = lambda s: sum(1 for g in groups if g & ix.words[s])
    cands = [s for g in groups for w in g for s in ix.carriers[w] if ix.callable(s)]
    if not cands:
        return fallback
    best = max(covers(s) for s in cands)
    anchors = sorted({s for s in cands if covers(s) == best},
                     key=lambda s: (len(ix.words[s]) - best, ix.sym[s]["q"]))[:3]
    depth, gain = {}, collections.defaultdict(float)
    for a in anchors:
        best = {}
        for step in (ix.rv, ix.fw):
            front, seen = [a], {a}
            for d in range(1, max_depth + 1):
                nxt = [m for n in front for m in step[n] if ix.prod(m) and m not in anchors and m not in seen]
                seen.update(nxt)
                for m in nxt:
                    best[m] = min(best.get(m, 99), d)
                front = nxt
        for m, d in best.items():
            depth[m] = min(depth.get(m, 99), d)
            gain[m] = gain[m] + decay(d) if accumulate else max(gain[m], decay(d))
    region = set(depth) | set(anchors)
    share = lambda m: len(ix.nb[m] & region) / max(len(ix.nb[m]), 1)
    ranked = sorted(depth, key=lambda m: (-(gain[m] + 0.5 * share(m)), depth[m], ix.sym[m]["q"]))
    return merge(anchors, ranked, fallback)


def merge(*lists):
    return list(dict.fromkeys(x for lst in lists for x in lst))


def metrics(names, gold):
    return {"r5": len(set(names[:5]) & gold) / len(gold), "r10": len(set(names[:10]) & gold) / len(gold)}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("repo", type=Path)
    ap.add_argument("manifest", type=Path)
    ap.add_argument("output", type=Path)
    ap.add_argument("--binary", type=Path, required=True)
    a = ap.parse_args()
    common = Path(subprocess.check_output(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
                                          cwd=a.repo, text=True).strip())
    manifest = json.loads(a.manifest.read_text())
    with tempfile.TemporaryDirectory(prefix="memo-intersection-") as tmp:
        tmp = Path(tmp)
        subprocess.run(["git", "init", "-q", str(tmp)], check=True)
        (tmp / ".git/memo").mkdir()
        src = sqlite3.connect((common / "memo/base.db").as_uri() + "?mode=ro", uri=True)
        c = sqlite3.connect(tmp / ".git/memo/base.db")
        src.backup(c)
        if (common / "memo/memo.toml").exists():
            shutil.copyfile(common / "memo/memo.toml", tmp / ".git/memo/memo.toml")
        ix = Index(c)
        report = {"meta": dict(c.execute("SELECT * FROM meta")), "cases": []}
        totals = collections.defaultdict(lambda: [0.0, 0.0])
        for case in manifest["cases"]:
            gold = {ix.by_q[q] for q in case["expected"]}
            out = subprocess.run([str(a.binary), "--repo", str(tmp), "--no-index", "--json", "brief", case["query"]],
                                 capture_output=True, text=True)
            data = json.loads(out.stdout)["data"] if out.returncode == 0 else {"symbols": []}
            base = [ix.by_q[s["addr"]["qname"]] for s in data["symbols"] if s["addr"]["qname"] in ix.by_q]
            base = [s for s in base if ix.prod(s)]
            groups = ix.match(tokenize(case["query"]))
            lists = {"brief_atual": base}
            for lvl, name in ((1, "name"), (2, "call"), (3, "file")):
                for idf in (False, True):
                    lists[name + ("_idf" if idf else "")] = merge(rank(ix, groups, lvl, idf), base)
            top = lists["name"][0] if lists["name"] else None
            lists["name_rel"] = merge([top] if top else [], two_hops(ix, top) if top else [], lists["name"])
            lists["anchors_rel"] = anchors_rel(ix, groups, lists["name"])
            lists["harm3"] = anchors_rel(ix, groups, lists["name"], 3)
            lists["harm4"] = anchors_rel(ix, groups, lists["name"], 4)
            lists["geo4"] = anchors_rel(ix, groups, lists["name"], 4, lambda d: 0.5 ** (d - 1))
            lists["harm4_sum"] = anchors_rel(ix, groups, lists["name"], 4, accumulate=True)
            lists["harm2_sum"] = anchors_rel(ix, groups, lists["name"], 2, accumulate=True)
            lists["harm5"] = anchors_rel(ix, groups, lists["name"], 5)
            lists["geo5"] = anchors_rel(ix, groups, lists["name"], 5, lambda d: 0.5 ** (d - 1))
            row = {"query": case["query"], "words": [[ix.text[w] for w in sorted(g)][:4] for g in groups],
                   "brief_exit": out.returncode, "strategies": {}}
            for k, lst in lists.items():
                names = [s for s in lst if ix.callable(s)]
                m = metrics(names, gold)
                totals[k][0] += m["r5"]; totals[k][1] += m["r10"]
                row["strategies"][k] = {**m, "top5": [ix.sym[s]["q"] for s in names[:5]]}
            report["cases"].append(row)
        n = len(manifest["cases"])
        report["summary"] = {k: {"r5": round(v[0] / n, 3), "r10": round(v[1] / n, 3)} for k, v in totals.items()}
        a.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
        for k, v in report["summary"].items():
            print(f"{k:12} R5={v['r5']:.3f} R10={v['r10']:.3f}")


if __name__ == "__main__":
    main()
