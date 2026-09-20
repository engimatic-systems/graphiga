+++
created = "2026-01-01T00:00:00Z"
deps = ["demo-decision", "demo-task"]
links = []
parent = "demo-phase"
priority = 1
status = "open"
tags = ["backups", "example"]
type = "bug"
+++
# Example: reject restoration onto the source

## Description

Hypothetical regression: a restore destination may name the original source. This is an example, not a reported Graphiga defect.

## Scope

Add an explicit source/destination identity guard to the fictional restore operation.

## Design

## Acceptance Criteria

An attempted self-overwrite is rejected before modifying either file.
