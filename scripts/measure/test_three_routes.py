"""Controls for the measurement algorithm, independent of a real repository."""

import sqlite3
import tempfile
import unittest
from pathlib import Path

from three_routes import Graph, http_entries, metrics


def graph_with_hub():
    connection = sqlite3.connect(":memory:")
    connection.executescript("""
        CREATE TABLE file(id INTEGER, path TEXT);
        CREATE TABLE symbol(id INTEGER, qname TEXT, kind TEXT, is_test INTEGER,
                            file_id INTEGER, is_entrypoint INTEGER);
        CREATE TABLE edge(src INTEGER, dst INTEGER, kind TEXT, approx INTEGER);
        INSERT INTO file VALUES (1, 'flow.py');
    """)
    names = {1: "repo.Target.run", 2: "service.handle", 3: "api.route", 4: "logging.emit"}
    names.update({n: f"unrelated.f{n}" for n in range(10, 40)})
    connection.executemany("INSERT INTO symbol VALUES (?,?,'function',0,1,0)", names.items())
    edges = [(3, 2), (2, 1), (2, 4), (1, 4)] + [(n, 4) for n in range(10, 40)]
    connection.executemany("INSERT INTO edge VALUES (?,?,'CHAMA',0)", edges)
    return connection, Graph(connection)


class ExperimentControls(unittest.TestCase):
    """GIVEN controlled graphs WHEN strategies run THEN evidence boundaries remain explicit."""

    def test_paths_recover_entrypoint_before_unrelated_hub(self):
        connection, graph = graph_with_hub()
        self.addCleanup(connection.close)
        ranked, detail = graph.rank(1, {3}, True, True)
        self.assertEqual(ranked[:2], [2, 3])
        self.assertLess(ranked.index(3), ranked.index(4))
        self.assertEqual(detail["entrypoints_reaching_target"], ["api.route"])
        self.assertEqual(len(ranked), len(set(ranked)))

    def test_zero_entrypoints_cannot_create_a_path_signal(self):
        connection, graph = graph_with_hub()
        self.addCleanup(connection.close)
        near, _ = graph.rank(1, set(), False, False)
        paths, detail = graph.rank(1, set(), True, False)
        self.assertEqual(near, paths)
        self.assertEqual(detail["path_count"], 0)

    def test_approximate_call_does_not_prove_entrypoint_reachability(self):
        connection, _ = graph_with_hub()
        self.addCleanup(connection.close)
        connection.execute("UPDATE edge SET approx=1 WHERE src=3 AND dst=2")
        graph = Graph(connection)
        _, detail = graph.rank(1, {3}, True, True)
        self.assertEqual(detail["entrypoints_reaching_target"], [])

    def test_http_recognizer_requires_imported_router_and_literal_route(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "routes.py").write_text(
                "from fastapi import APIRouter as Router\nrouter=Router()\n"
                "@router.get('/items')\ndef list_items(): pass\n"
                "@other.get('/noise')\ndef unrelated(): pass\n"
                "@router.get(dynamic_path)\ndef dynamic(): pass\n"
            )
            symbols = {n: {"qname": "routes." + name, "test": False, "path": "routes.py"}
                       for n, name in enumerate(["list_items", "unrelated", "dynamic"], 1)}
            detected, evidence = http_entries(root, symbols)
            self.assertEqual(detected, {1})
            self.assertEqual(evidence[0]["route"], "/items")

    def test_reference_metrics_do_not_call_unlabelled_results_false_positives(self):
        actual = metrics(["unknown", "expected"], {"expected"}, set(), {"known_wrong"})
        self.assertEqual(actual["expected_recall_at_5"], 1)
        self.assertEqual(actual["known_negatives_at_5"], 0)


if __name__ == "__main__":
    unittest.main()
