#!/usr/bin/env python3
"""Gera um repositório Python sintético, com git, para medir o `memo`.

Uso: gen_repo.py <destino> [--files 5000]

Cada arquivo define uma classe `Calculate<tok>UseCase` com `execute`, `_total` e `_limit` e duas
funções auxiliares, todas chamadas entre si no mesmo módulo (chamadas exatas). As palavras
`calculate`, `total` e `limit` aparecem em quase todo arquivo, de propósito: é o pior caso para
`query word`. A estrutura é repetitiva e tem poucas arestas entre arquivos; um repositório real
pode se comportar diferente. Este gerador substitui o de `/tmp`, que se perdeu: os números
absolutos podem diferir dos registrados nas TASKs, e a ordem de grandeza é o que importa.
"""
import argparse
import subprocess
from pathlib import Path

CONTEXTS = ["billing", "orders", "booking", "inventory", "accounts"]


def token(i: int) -> str:
    return "w" + chr(97 + (i // 26) % 26) + chr(97 + i % 26) + chr(97 + (i // 676) % 26)


def module_source(i: int) -> str:
    t = token(i)
    return (
        f"class Calculate{t.capitalize()}UseCase:\n"
        f"    def execute(self, amount):\n"
        f"        total = self._total(amount)\n"
        f"        limit = self._limit(total)\n"
        f"        return helper_total_{t}(limit) + helper_limit_{t}(total)\n"
        f"\n"
        f"    def _total(self, amount):\n"
        f"        return helper_total_{t}(amount)\n"
        f"\n"
        f"    def _limit(self, total):\n"
        f"        return helper_limit_{t}(total)\n"
        f"\n\n"
        f"def helper_total_{t}(value):\n"
        f"    return value\n"
        f"\n\n"
        f"def helper_limit_{t}(value):\n"
        f"    return value\n"
    )


def test_source(i: int) -> str:
    t = token(i)
    return (
        f"def test_given_amount_when_calculate_{t}_then_returns_total():\n"
        f'    """GIVEN amount WHEN calculate THEN total"""\n'
        f"    assert True\n"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("dest")
    parser.add_argument("--files", type=int, default=5000)
    args = parser.parse_args()
    root = Path(args.dest)
    root.mkdir(parents=True, exist_ok=True)
    for i in range(args.files):
        context = CONTEXTS[i % len(CONTEXTS)]
        if i % 10 == 9:
            path = root / "app" / context / "application" / "tests" / f"test_{token(i)}.py"
            source = test_source(i)
        else:
            path = root / "app" / context / "application" / "usecases" / f"calculate_{token(i)}_usecase.py"
            source = module_source(i)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source)
    git = ["git", "-c", "user.name=gen", "-c", "user.email=gen@example.com", "-c", "commit.gpgsign=false"]
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=root, check=True)
    subprocess.run(["git", "add", "-A"], cwd=root, check=True)
    subprocess.run([*git, "commit", "-q", "-m", "synthetic"], cwd=root, check=True)
    print(f"{args.files} arquivos em {root}")


if __name__ == "__main__":
    main()
