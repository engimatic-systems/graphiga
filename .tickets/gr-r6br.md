+++
created = "2026-11-06T13:01:45Z"
deps = ["gr-bootstrap"]
links = []
priority = 2
status = "open"
tags = ["performance"]
type = "task"
+++
# Measure ticket-store latency at realistic sizes

## Description

Growing-store latency motivated the move away from shell-based ticket. Graphiga currently loads and parses every file for every command; mutations clone the document map and titles can trigger repeated Markdown parsing. Measure the actual experience before assuming Rust solves the problem.

## Design

Use reproducibly generated temporary stores at several sizes, including a representative working set and larger cases. Measure engine commands and wrapper commands separately so list-based ID resolution overhead is visible. Include realistic body lengths and dependency shapes; record hardware, toolchain, and dataset parameters.

## Acceptance Criteria

Report repeatable timings for show, list, ready, create, and status changes, with exact dataset sizes and commands. Identify where time is spent and propose the smallest justified optimization. Agree an interactive latency target based on the intended workload before claiming success. No production store is modified and no index/backend redesign is assumed.
