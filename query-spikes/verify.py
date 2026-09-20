#!/usr/bin/env python3
"""Shared black-box acceptance runner, not either query implementation."""
import argparse
import copy
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent
STORE = ROOT / "store"


def read_nodes(store):
    nodes = {}
    for path in sorted(store.glob("*.md")):
        text = path.read_text()
        assert text.startswith("+++\n"), path
        front, body = text[4:].split("\n+++\n", 1)
        fields = tomllib.loads(front)
        titles = re.findall(r"^# (.+)$", body, re.M)
        assert len(titles) == 1, path
        nodes[path.stem] = {"fields": fields, "title": titles[0]}
    return nodes


def oracle(nodes):
    # An explicit independent account of the three domain questions.
    def reaches_root(start):
        current, seen = start, set()
        while current not in seen:
            seen.add(current)
            parent = nodes[current]["fields"].get("parent")
            if parent is None:
                return False
            assert parent in nodes
            if parent == "demo-epic":
                return True
            current = parent
        return False

    return {
        "open_backups": sorted(
            id for id, n in nodes.items()
            if n["fields"]["status"] == "open"
            and "backups" in n["fields"].get("tags", [])
        ),
        "ready": sorted(
            id for id, n in nodes.items()
            if n["fields"]["status"] in ("open", "in_progress")
            and all(nodes[d]["fields"]["status"] == "closed"
                    for d in n["fields"].get("deps", []))
        ),
        "epic_descendants": sorted(
            id for id in nodes if id != "demo-epic" and reaches_root(id)
        ),
    }


def check_fixture():
    schema = tomllib.loads((STORE / "schema.toml").read_text())
    reference_schema = tomllib.loads((ROOT.parent / "examples/tko/schema.toml").read_text())
    assert schema == reference_schema, "comparison must retain the tko base schema"
    nodes = read_nodes(STORE)
    assert len(nodes) == 9
    for id, n in nodes.items():
        f = n["fields"]
        assert f["type"] in schema["types"]
        for name in ("status", "priority", "created", "deps", "links"):
            assert name in f, (id, name)
        for target in f.get("deps", []) + f.get("links", []):
            assert target in nodes, (id, target)
        if "parent" in f:
            assert f["parent"] in nodes
        for target in f.get("links", []):
            assert id in nodes[target]["fields"]["links"]
    expected = json.loads((ROOT / "expected.json").read_text())
    for language in ("predicates", "datalog", "datafrog"):
        definitions = tomllib.loads((ROOT / f"{language}.toml").read_text())["queries"]
        assert set(definitions) == set(expected["baseline"])
        assert all(isinstance(value, str) and value.strip() for value in definitions.values())
    assert oracle(nodes) == expected["baseline"]
    reopened = copy.deepcopy(nodes)
    reopened["demo-decision"]["fields"]["status"] = "open"
    for query, wanted in expected["decision_reopened"].items():
        assert oracle(reopened)[query] == wanted
    closed = copy.deepcopy(nodes)
    closed["demo-task"]["fields"]["status"] = "closed"
    for query, wanted in expected["task_closed"].items():
        assert oracle(closed)[query] == wanted
    cycle = copy.deepcopy(nodes)
    cycle["demo-epic"]["fields"]["parent"] = "demo-phase"
    assert oracle(cycle)["epic_descendants"] == expected["baseline"]["epic_descendants"]
    print("PASS: common TOML schema, nine-node fixture, all three declaration tables, independent baseline/mutation/cycle expected results")
    return expected


def run_acceptance(binary, language, expected):
    definitions = tomllib.loads((ROOT / f"{language}.toml").read_text())["queries"]
    with tempfile.TemporaryDirectory(prefix=f"graphiga-query-{language}-") as temporary:
        store = Path(temporary) / "store"
        shutil.copytree(STORE, store)
        base_schema = (store / "schema.toml").read_text()

        def install_queries(queries):
            declarations = "\n[queries]\n" + "\n".join(
                f"{name} = {json.dumps(text)}" for name, text in queries.items()
            ) + "\n"
            # JSON string literals here are compatible TOML basic strings;
            # this is schema construction, not Graphiga JSON CLI input.
            content = base_schema + declarations
            tomllib.loads(content)
            (store / "schema.toml").write_text(content)

        def contents():
            return {p.name: p.read_bytes() for p in store.iterdir() if p.is_file()}

        def invoke(*args, ok=True, readonly=False):
            before = contents() if readonly else None
            result = subprocess.run(
                [str(binary), "--store", str(store), *args],
                text=True, capture_output=True, timeout=30,
            )
            if readonly:
                assert contents() == before, f"query changed store: {args}"
            if ok:
                assert result.returncode == 0, (args, result.returncode, result.stderr)
            else:
                assert result.returncode != 0, (args, "unexpected success", result.stdout)
                assert result.stderr.strip(), (args, "missing diagnostic")
            return result.stdout

        def check(query, wanted, human=False):
            nodes = read_nodes(store)
            rows = json.loads(invoke("query", query, "--json", readonly=True))
            expected_rows = [
                {"id": id, "type": nodes[id]["fields"]["type"], "title": nodes[id]["title"]}
                for id in wanted
            ]
            assert rows == expected_rows, (query, rows, expected_rows)
            if human:
                lines = invoke("query", query, readonly=True).splitlines()
                assert [line.split()[0] for line in lines if line.strip()] == wanted

        install_queries(definitions)
        for query, wanted in expected["baseline"].items():
            check(query, wanted, human=True)
        print("PASS: same baseline IDs, ordered compact/JSON results, read-only behavior")

        invoke("set", "demo-decision", "status", "open")
        for query, wanted in expected["decision_reopened"].items():
            check(query, wanted)
        invoke("set", "demo-decision", "status", "closed")
        invoke("set", "demo-task", "status", "closed")
        for query, wanted in expected["task_closed"].items():
            check(query, wanted)
        invoke("set", "demo-task", "status", "open")
        invoke("set", "demo-epic", "parent", "demo-phase")
        check("epic_descendants", expected["baseline"]["epic_descendants"])
        invoke("unset", "demo-epic", "parent")
        print("PASS: current-state dependency queries, mixed dependency satisfaction, cyclic parent traversal")

        renamed = dict(definitions)
        renamed["renamed_filter"] = renamed.pop("open_backups")
        install_queries(renamed)
        check("renamed_filter", expected["baseline"]["open_backups"])
        renamed["renamed_filter"] = renamed["renamed_filter"].replace("backups", "not_a_fixture_tag")
        install_queries(renamed)
        check("renamed_filter", [])
        invoke("query", "missing_query", "--json", ok=False, readonly=True)
        install_queries(definitions)
        for path in store.glob("*.md"):
            path.unlink()
        for query in definitions:
            check(query, [])
        invoke("query", "missing_query", "--json", ok=False, readonly=True)
        invalid = "status =" if language == "predicates" else "result(N) :-"
        install_queries({"broken": invalid})
        invoke("query", "broken", "--json", ok=False, readonly=True)
        print("PASS: declaration-driven names/constants, empty results/store, unknown names, syntax errors before scanning")
    print(f"PASS: shared query acceptance for {language}; temporary stores removed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--language", choices=("predicates", "datalog", "datafrog"))
    args = parser.parse_args()
    expected = check_fixture()
    if args.check_fixture:
        if args.binary or args.language:
            parser.error("--check-fixture cannot be combined with implementation arguments")
        print("Preparation only: no query engine was executed.")
        return
    if args.binary is None or args.language is None:
        parser.error("provide --binary and --language, or --check-fixture")
    binary = args.binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error(f"not an executable: {binary}")
    run_acceptance(binary, args.language, expected)


if __name__ == "__main__":
    main()
