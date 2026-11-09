#!/usr/bin/env python3
"""Offline ablation of exact neighbors, entrypoint paths and local connectivity.

The production index is opened read-only. CLI baselines and experimental entrypoint
flags use a disposable SQLite backup. A separate manifest supplies source-reviewed
expected symbols; these are never used to build or sort candidates.
"""

import argparse
import ast
import collections
import hashlib
import json
import math
import itertools
import shutil
import sqlite3
import subprocess
import tempfile
from pathlib import Path


def readonly(path):
    return sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)


def http_entries(repo, symbols):
    """Recognize literal HTTP decorators on a locally constructed FastAPI router."""
    by_name = {s["qname"]: sid for sid, s in symbols.items()}
    found = set()
    evidence = []
    for path in sorted({s["path"] for s in symbols.values() if not s["test"]}):
        try:
            tree = ast.parse((repo / path).read_text())
        except (OSError, SyntaxError, UnicodeError):
            continue
        constructors = set()
        for n in tree.body:
            if isinstance(n, ast.ImportFrom) and n.module == "fastapi":
                constructors.update(a.asname or a.name for a in n.names if a.name == "APIRouter")
            elif isinstance(n, ast.Import):
                constructors.update((a.asname or a.name) + ".APIRouter" for a in n.names if a.name == "fastapi")
        routers = set()
        for n in tree.body:
            if isinstance(n, ast.Assign) and isinstance(n.value, ast.Call):
                if ast.unparse(n.value.func) in constructors:
                    routers.update(t.id for t in n.targets if isinstance(t, ast.Name))
        module = path.removesuffix(".py").removesuffix("/__init__").replace("/", ".")
        for n in tree.body:
            if not isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)):
                continue
            for d in n.decorator_list:
                if not (isinstance(d, ast.Call) and isinstance(d.func, ast.Attribute)):
                    continue
                if not (isinstance(d.func.value, ast.Name) and d.func.value.id in routers):
                    continue
                if d.func.attr not in {"get", "post", "put", "patch", "delete", "head", "options"}:
                    continue
                sid = by_name.get(module + "." + n.name)
                if sid is not None and d.args and isinstance(d.args[0], ast.Constant) and isinstance(d.args[0].value, str):
                    found.add(sid)
                    evidence.append({"symbol": module + "." + n.name, "path": path,
                                     "line": d.lineno, "method": d.func.attr.upper(), "route": d.args[0].value})
    return found, evidence


def distances(start, adjacency, depth):
    dist = {start: 0}
    queue = collections.deque([start])
    while queue:
        node = queue.popleft()
        if dist[node] >= depth:
            continue
        for other in adjacency.get(node, ()):
            if other not in dist:
                dist[other] = dist[node] + 1
                queue.append(other)
    return dist


class Graph:
    def __init__(self, connection):
        self.symbols = {
            sid: {"qname": q, "kind": kind, "test": bool(test) or "/tests/" in "/" + path,
                  "path": path, "entry": bool(entry)}
            for sid, q, kind, test, path, entry in connection.execute(
                "SELECT s.id,s.qname,s.kind,s.is_test,coalesce(f.path,''),s.is_entrypoint "
                "FROM symbol s LEFT JOIN file f ON f.id=s.file_id"
            )
        }
        self.by_name = {s["qname"]: sid for sid, s in self.symbols.items()}
        self.forward = collections.defaultdict(set)
        self.reverse = collections.defaultdict(set)
        self.exact = collections.defaultdict(set)
        self.weights = collections.defaultdict(dict)
        weights = {"CHAMA": 1, "CRIA": .8, "RECEBE": .6, "RETORNA": .6,
                   "LE": .5, "ESCREVE": .5, "ESPECIALIZA": .3}
        for src, dst, kind, approx in connection.execute("SELECT src,dst,kind,approx FROM edge WHERE dst IS NOT NULL"):
            if kind not in weights or self.symbols[src]["test"] or self.symbols[dst]["test"]:
                continue
            weight = weights[kind] * (.25 if approx else 1)
            # Duplicate call sites cannot manufacture extra votes or connectivity.
            self.weights[src][dst] = max(self.weights[src].get(dst, 0), weight)
            self.weights[dst][src] = max(self.weights[dst].get(src, 0), weight)
            if not approx:
                self.exact[src].add(dst)
                self.exact[dst].add(src)
                if kind == "CHAMA":
                    self.forward[src].add(dst)
                    self.reverse[dst].add(src)

    def callable(self, sid):
        return not self.symbols[sid]["test"] and self.symbols[sid]["kind"] in {"function", "method"}

    def rank(self, target, entrypoints, use_paths, use_connections, global_degree=False, rank_connections=True):
        near = set(self.exact.get(target, ()))
        ancestors = distances(target, self.reverse, 6)
        entries = set(entrypoints) & ancestors.keys()
        paths = set()
        if use_paths:
            queue = list(entries)
            while queue:
                node = queue.pop()
                if node in paths:
                    continue
                paths.add(node)
                queue.extend(n for n in self.forward.get(node, ())
                             if ancestors.get(n) == ancestors[node] - 1)
        region = near | paths | {target}
        local = collections.defaultdict(float)
        for node in region:
            for other, weight in self.weights.get(node, {}).items():
                local[other] += weight
        def connectivity(node):
            degree = sum(self.weights.get(node, {}).values())
            return degree if global_degree else local[node] / math.sqrt(max(degree, 1))
        candidates = region | (set(local) if use_connections else set())
        candidates = {n for n in candidates if n != target and self.callable(n)}
        connected = set(sorted(candidates, key=lambda n: (-connectivity(n), self.symbols[n]["qname"]))[:5]) if use_connections and rank_connections else set()
        distance = distances(target, self.exact, 8)
        def key(node):
            votes = (node in near) + (node in paths) + (node in connected)
            return (-votes, node not in paths, node not in near, distance.get(node, 99),
                    -connectivity(node) if use_connections and rank_connections else 0, self.symbols[node]["qname"])
        ranked = sorted(candidates, key=key)
        return ranked, {"entrypoints_reaching_target": [self.symbols[n]["qname"] for n in sorted(entries)],
                        "near_count": len(near), "path_count": len(paths), "candidate_count": len(candidates)}


def metrics(names, gold, entries, negatives):
    first = next((i + 1 for i, q in enumerate(names) if q in entries), None)
    return {"expected_recall_at_5": len(set(names[:5]) & gold) / len(gold),
            "expected_recall_at_10": len(set(names[:10]) & gold) / len(gold),
            "first_entrypoint_rank": first,
            "entrypoint_recall_at_5": len(set(names[:5]) & entries) / len(entries) if entries else None,
            "known_negatives_at_5": len(set(names[:5]) & negatives), "returned": len(names)}


def cli_baseline(binary, repo, target, command):
    result = subprocess.run([str(binary), "--repo", str(repo), "--task", "three-routes",
                             "--no-index", "--full", "--json", *command, target], capture_output=True, text=True)
    if result.returncode:
        return [], {"exit_code": result.returncode, "message": (result.stdout + result.stderr).strip()}
    data = json.loads(result.stdout)["data"]
    if command == ["brief"]:
        names = [s["addr"]["qname"] for s in data["symbols"] if not s["in_tests"]]
    else:
        names = [i["addr"]["qname"] for i in data["callers"] if i.get("addr")]
        names += [i["addr"]["qname"] for key in ("consumers", "protocol", "by_word") for i in data[key]]
    return list(dict.fromkeys(names)), {"exit_code": 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repo", type=Path)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    repo = args.repo.resolve()
    common = Path(subprocess.check_output(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], cwd=repo, text=True).strip())
    source = common / "memo" / "base.db"
    manifest_bytes = args.manifest.read_bytes()
    manifest = json.loads(manifest_bytes)
    with readonly(source) as original, tempfile.TemporaryDirectory(prefix="memo-three-routes-") as temp:
        scratch = Path(temp)
        subprocess.run(["git", "init", "-q", str(scratch)], check=True)
        memo_dir = scratch / ".git" / "memo"
        memo_dir.mkdir()
        connection = sqlite3.connect(memo_dir / "base.db")
        original.backup(connection)
        config = common / "memo" / "memo.toml"
        if config.exists():
            shutil.copyfile(config, memo_dir / "memo.toml")
        graph = Graph(connection)
        indexed_entries = {n for n, s in graph.symbols.items() if s["entry"]}
        detected, evidence = http_entries(repo, graph.symbols)
        connection.executemany("UPDATE symbol SET is_entrypoint=1 WHERE id=?", [(n,) for n in detected])
        connection.commit()
        report = {"metadata": {"graph": dict(connection.execute("SELECT * FROM meta")),
                   "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
                   "graph_symbols": len(graph.symbols), "indexed_entrypoints": len(indexed_entries),
                   "detected_http_entrypoints": len(detected), "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
                   "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                   "limits": "callable production symbols; exact call paths <=6; top-5 connectivity; no weight tuning"},
                  "entrypoint_evidence": evidence, "cases": []}
        for case in manifest["cases"]:
            target = graph.by_name[case["target"]]
            gold, entries, negatives = map(set, [case["expected"], case.get("entrypoints", []), case.get("negatives", [])])
            missing = (gold | entries | negatives) - graph.by_name.keys()
            if missing:
                raise ValueError(f"Unknown reference symbols: {sorted(missing)}")
            row = {"name": case["name"], "target": case["target"], "expected": sorted(gold), "strategies": {}}
            for command in (["brief"], ["query", "impact"]):
                names, details = cli_baseline(args.binary, scratch, case["target"], command)
                names = [q for q in names if q != case["target"] and graph.callable(graph.by_name[q])]
                row["strategies"]["current_" + command[-1]] = {"top10": names[:10], "metrics": metrics(names, gold, entries, negatives), **details}
            for name, flags in {
                "neighbors": (indexed_entries, False, False, False),
                "paths_index_only": (indexed_entries, True, False, False),
                "three_index_only": (indexed_entries, True, True, False),
                "paths_http": (indexed_entries | detected, True, False, False),
                "three_http": (indexed_entries | detected, True, True, False),
                "three_global_degree": (indexed_entries | detected, True, True, True),
                "expanded_distance": (indexed_entries, False, True, False, False),
            }.items():
                ranked, details = graph.rank(target, *flags)
                names = [graph.symbols[n]["qname"] for n in ranked]
                row["strategies"][name] = {"top10": names[:10], "metrics": metrics(names, gold, entries, negatives), **details}
            # Diagnostic control: combine the two existing answers without gold-aware selection.
            lists = [row["strategies"][s]["top10"] for s in ("current_brief", "current_impact")]
            union = list(dict.fromkeys(q for pair in itertools.zip_longest(*lists) for q in pair if q))
            row["strategies"]["current_union_round_robin"] = {"top10": union[:10], "metrics": metrics(union, gold, entries, negatives)}
            call_neighbors = {n: graph.forward.get(n, set()) | graph.reverse.get(n, set()) for n in graph.symbols}
            confirmed = distances(target, call_neighbors, 2)
            filtered = [q for q in union if graph.by_name[q] in confirmed]
            row["strategies"]["current_union_exact_two_hops"] = {"top10": filtered[:10], "metrics": metrics(filtered, gold, entries, negatives)}
            report["cases"].append(row)
        report["ambiguous_controls"] = []
        for query in manifest.get("ambiguous_controls", []):
            for command in (["brief"], ["query", "impact"]):
                _, details = cli_baseline(args.binary, scratch, query, command)
                report["ambiguous_controls"].append({"query": query, "command": command, **details})
        args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
        print(json.dumps(report["metadata"], indent=2))
        for strategy in report["cases"][0]["strategies"]:
            ms = [c["strategies"][strategy]["metrics"] for c in report["cases"]]
            print(strategy, json.dumps({key: round(sum(m[key] for m in ms) / len(ms), 4)
                                       for key in ("expected_recall_at_5", "expected_recall_at_10", "known_negatives_at_5")}))


if __name__ == "__main__":
    main()
