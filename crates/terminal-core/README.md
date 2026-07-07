# terminal-core

Headless terminal state engine for Hera.

This crate owns terminal correctness: byte ingestion, parser integration, screen
state, cursor state, scrollback policy, resizing and renderer-neutral snapshots.
It intentionally does not depend on PTY, GPUI, Paneflow, windowing or platform
runtime APIs.
