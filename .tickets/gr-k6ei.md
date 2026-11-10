+++
created = "2026-11-06T13:01:44Z"
deps = ["gr-bootstrap"]
links = []
priority = 1
status = "open"
tags = ["reliability", "storage"]
type = "task"
+++
# Harden ticket-store writes for everyday use

## Description

Graphiga now maintains its own work tickets. Store::persist directly truncates files, and separate commands can load the same state and overwrite each other's changes. Protect ordinary use without expanding this task into a transactional database.

## Design

Add atomic replacement for each changed file and a store lock covering load, calculation, and persistence for cooperating Graphiga processes. Preserve Markdown body bytes. State precisely what remains unsupported: external editors do not acquire the lock, and multiple file replacements do not form a transaction.

## Acceptance Criteria

Concurrent cooperating writers cannot silently lose updates. A failed per-file replacement preserves the previous complete file. Reciprocal multi-file operations report partial progress accurately if a later replacement fails. Focused failure/concurrency checks exercise these behaviors. Existing engine and TK tests pass; documentation states the remaining boundaries.
