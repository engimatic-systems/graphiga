"""Exercise the TK wrapper against a real Graphiga executable and temporary store."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("GRAPHIGA_BIN", ROOT / "target/debug/graphiga")).resolve()


class TicketWorkflow(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.store = Path(self.temporary.name)
        shutil.copyfile(ROOT / ".tickets/schema.toml", self.store / "schema.toml")
        self.env = {**os.environ, "GRAPHIGA_BIN": str(BINARY), "TICKETS_DIR": str(self.store)}

    def run_tk(self, *arguments, ok=True):
        # Run outside the checkout, with spaces in arguments preserved by argv.
        result = subprocess.run(
            [str(ROOT / "tk"), *arguments], cwd=self.store, env=self.env,
            capture_output=True, text=True,
        )
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def create(self, title, *arguments):
        node_id = self.run_tk("create", title, *arguments).stdout.strip()
        self.assertRegex(node_id, r"^gr-[a-z0-9]{4}$")
        return node_id

    def fields(self, node_id):
        text = (self.store / f"{node_id}.md").read_text()
        return tomllib.loads(text.split("+++\n", 2)[1])

    def body(self, node_id):
        return (self.store / f"{node_id}.md").read_bytes().split(b"+++\n", 2)[2]

    def ids(self, command):
        return [row["id"] for row in json.loads(self.run_tk(command, "--json").stdout)]

    def snapshot(self):
        return {path.name: path.read_bytes() for path in self.store.iterdir()}

    def test_creation_types_defaults_content_and_parent(self):
        description = "First line\n\nSecond line with café and **emphasis**."
        acceptance = "- One condition\n- Another condition"
        parent = self.create("Parent", "-t", "epic")
        node_id = self.create(
            "Ticket with spaces", "-d", description, "--acceptance", acceptance,
            "--design", "Small wrapper", "--parent", parent[3:],
            "--tags", "self-hosting,cli", "--external-ref", "work-123",
        )
        fields = self.fields(node_id)
        self.assertEqual(fields["status"], "open")
        self.assertEqual(fields["priority"], 2)
        self.assertEqual(fields["type"], "task")
        self.assertEqual(fields["deps"], [])
        self.assertEqual(fields["links"], [])
        self.assertEqual(fields["parent"], parent)
        self.assertEqual(fields["external-ref"], "work-123")
        self.assertEqual(set(fields["tags"]), {"self-hosting", "cli"})
        self.assertRegex(fields["created"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        original = self.body(node_id)
        shown = self.run_tk("show", node_id[3:]).stdout
        self.assertIn(description, shown)
        self.assertIn(acceptance, shown)
        for command, status in (("start", "in_progress"), ("close", "closed"), ("reopen", "open")):
            self.run_tk(command, node_id)
            self.assertEqual(self.fields(node_id)["status"], status)
            self.assertEqual(self.body(node_id), original)
        for kind in ("bug", "feature", "task", "epic", "chore"):
            created = self.create(kind, "-t", kind, "-p", "0")
            self.assertEqual(self.fields(created)["type"], kind)
            self.assertEqual(self.fields(created)["priority"], 0)
        self.assertEqual(self.ids("ls"), sorted(self.ids("list")))
        self.assertIn(node_id, self.ids("ready"))  # An open parent does not block.

    def test_dependencies_readiness_cycles_and_reciprocal_links(self):
        a = self.create("Dependent")
        b = self.create("Prerequisite")
        self.run_tk("dep", a, b)
        self.assertEqual(self.ids("blocked"), [a])
        self.assertEqual(self.ids("ready"), [b])
        self.run_tk("start", a)
        self.run_tk("close", b)
        self.assertIn(a, self.ids("ready"))
        self.assertEqual(self.ids("blocked"), [])
        self.run_tk("reopen", b)
        self.run_tk("dep", b, a)
        self.assertEqual(self.ids("blocked"), sorted([a, b]))
        self.run_tk("close", a)  # Closing is allowed even with unresolved deps.
        self.assertNotIn(a, self.ids("ready"))
        self.assertNotIn(a, self.ids("blocked"))
        self.assertIn(a, self.ids("closed"))
        self.run_tk("reopen", a)
        self.run_tk("undep", a, b)
        self.assertIn(a, self.ids("ready"))
        self.run_tk("link", a, b)
        self.assertEqual(self.fields(a)["links"], [b])
        self.assertEqual(self.fields(b)["links"], [a])
        self.run_tk("unlink", b, a)
        self.assertEqual(self.fields(a)["links"], [])
        self.assertEqual(self.fields(b)["links"], [])
        self.run_tk("status", a, "in_progress")
        self.assertEqual(self.fields(a)["status"], "in_progress")

    def test_invalid_input_never_changes_store(self):
        a = self.create("One")
        self.create("Two")
        before = self.snapshot()
        for arguments in (
            ("create", "Bad type", "-t", "research"),
            ("create", "Bad priority", "-p", "5"),
            ("create", "Bad priority", "-p", "-1"),
            ("create", "Missing parent", "--parent", "missing"),
            ("create", "Unsupported assignee", "-a", "Full Name"),
            ("status", a, "blocked"),
            ("close", "gr-"),
            ("close", ""),
            ("close", "missing"),
            ("dep", a, "missing"),
            ("dep", a, a),
            ("link", a, "missing"),
            ("link", a, a, a),
        ):
            with self.subTest(arguments=arguments):
                self.run_tk(*arguments, ok=False)
                self.assertEqual(self.snapshot(), before)

    def test_exact_id_wins_over_partial_match(self):
        for node_id in ("gr-exact", "gr-exact-long"):
            subprocess.run(
                [str(BINARY), "--store", str(self.store), "create", "task", node_id,
                 "--title", node_id, "--set", "created", "2026-11-06T00:00:00Z"],
                check=True, capture_output=True,
            )
        self.run_tk("close", "gr-exact")
        self.assertEqual(self.fields("gr-exact")["status"], "closed")
        self.assertEqual(self.fields("gr-exact-long")["status"], "open")


if __name__ == "__main__":
    if not BINARY.is_file():
        raise SystemExit("Build Graphiga first or set GRAPHIGA_BIN to its executable")
    unittest.main()
