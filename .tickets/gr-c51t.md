+++
created = "2026-11-06T13:01:46Z"
deps = ["gr-bootstrap"]
links = []
priority = 2
status = "open"
tags = ["ledger", "storage"]
type = "task"
+++
# Add a small JSONL backend for a decision-ledger wrapper

## Description

A focused decision ledger should fit in one JSONL file rather than a directory of documents. Keep ledger and TK as separate tools with separate schemas and stores. Share the Graphiga runtime without requiring cross-store relationships or a generalized orchestration system.

## Design

Write a concrete one-record-per-line example and settle explicit identity, content, schema location, and append-only behavior before implementation. Separate logical records from Markdown/filename encoding only as needed for this second backend. For the initial ledger, creation may append while mutations are rejected; do not introduce event replay or revisions by default.

## Acceptance Criteria

A small ledger wrapper can create and read/query schema-validated records in one JSONL file. Existing records remain byte-for-byte unchanged on a successful append. Duplicate IDs and unsupported mutations are rejected before writing. Specify and test behavior for malformed/truncated records and interrupted appends. The Markdown backend and TK checks remain working; SQLite and cross-store features are out of scope.
