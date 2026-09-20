+++
assignee = "example-agent"
created = "2026-01-01T00:00:00Z"
deps = ["demo-decision"]
external_ref = "https://example.invalid/issues/restore-example"
links = []
parent = "demo-epic"
priority = 2
status = "open"
tags = ["backups", "example"]
type = "task"
+++
# Example: implement selective restore

## Description

Implement a command that restores one selected file in this fictional project.

## Scope

One file and an explicit destination; no automatic overwrite.

## Design

Read the selected archive member and write it to the requested destination.

## Acceptance Criteria

- Restored bytes equal archived bytes.
- Missing members fail without creating a destination.
