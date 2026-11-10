+++
created = "2026-11-06T13:01:45Z"
deps = ["gr-bootstrap"]
links = []
priority = 1
status = "open"
tags = ["macos", "packaging"]
type = "task"
+++
# Package and validate Graphiga and TK on macOS

## Description

The concrete destination is the work Mac, where the original ticket tool was chosen to avoid compilation/setup friction. Current Nix outputs and CI support only x86_64-linux; the local wrapper has only been exercised on Linux.

## Design

Establish the work Mac's architecture and allowed installation route, then provide a reproducible native build and a simple documented installation for the engine plus wrapper/schema. Retain Linux support and the separate bootstrap executable. Do not assume cross-compilation is required.

## Acceptance Criteria

The supported Mac architecture and prerequisites are explicit. Build the executable and run the engine and TK workflow checks on macOS, or clearly record the outstanding runtime validation if no Mac runner is available. Document installation, creation of a fresh store, and version replacement. A smoke workflow creates tickets, blocks/unblocks a dependency, and closes work.
