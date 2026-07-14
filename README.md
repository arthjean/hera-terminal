# Hera

[Version francaise](docs/README_FR.md)

Hera is a headless terminal engine written in Rust. It turns a VT byte stream
into deterministic terminal state, renderer-neutral snapshots, bounded
scrollback, and replays that a GUI, TUI, test harness, or remote host can use.

The project targets long-running CLI and agent sessions without tying terminal
correctness to a renderer, PTY runtime, or specific application. Paneflow is its
first dogfood environment.

## Current Status

Hera is a six-crate Rust 2024 workspace. M1 through M5 are complete. M6 tested
a controlled rendering-authority experiment in Paneflow for explicitly selected
panes. The experiment is blocked after its first visible Windows canary and does
not replace the PTY, input handling, or default terminal path.

| Milestone | Status | Primary evidence |
|---|---|---|
| M0: research map | DONE | Architecture decisions and reference engine inventories |
| M1: headless core | DONE | VT ingestion, screens, scrollback, resize, snapshots, and fixtures |
| M2: PTY runtime | DONE | Direct and shell commands, resize, IO, lifecycle, and recordings |
| M3: Paneflow shadow dogfood | DONE | Side-by-side integration without changing the authoritative renderer |
| M4: public proof | DONE | Replays, benchmarks, memory profiles, API examples, and evidence package |
| M5: compatibility and release hardening | DONE | 18 passing compatibility rows and a targeted Windows dogfood pass |
| M6: controlled host replacement | BLOCKED | 60-minute Windows canary recorded P0 mismatches, dropped output and fallback |

The latest verified M5 run exercised two Paneflow panes for 45 minutes in
shadow mode and produced no mismatch reports. That result unlocks a limited
rendering-authority experiment. It does not yet prove cross-platform replacement
or public release readiness.

The M6 direction was selected after the final M5 report. The visible canary then
ran for 3,719 seconds with two panes. It recorded 219 P0 mismatches, one
unsupported checkpoint, 1,526,241 dropped Hera bytes and one safe fallback.
That evidence selects `replacement_experiment_blocked`; the default path remains
Alacritty.

Open limitations:

- Hera cannot yet keep render authority during the 100,000-line Paneflow burst.
- The Windows comparison fields behind the 219 P0 mismatches need diagnosis.
- Linux and macOS do not yet have runtime measurements equivalent to M5 on
  Windows.
- Dry runs for dependent crates remain blocked by unpublished Hera crates or
  the lack of an accepted staging strategy.
- The semver baseline and part of the supply-chain posture remain incomplete.
- Hera is not a desktop terminal application, a GPU renderer, or Paneflow's
  default terminal engine.

## Implemented Capabilities

`terminal-core` wraps `alacritty/vte` behind Hera-owned types. The parser
tokenizes the stream; Hera owns the semantics and observable state.

The core currently covers:

- incremental byte ingestion and structured VT actions
- primary and alternate screens, cursor, tabs, and modes
- C0 controls, CUP/HVP, ED/EL/ECH, and SGR attributes
- DEC modes 47, 1047, 1048, and 1049
- scrollback bounded by line and byte limits with stable row handles
- predictable resize and reflow without polluting primary scrollback
- viewport, cell, style, cursor, damage, and scrollback snapshots
- bracketed paste state exposed through the render model
- unknown or advanced payloads preserved as safe metadata

The runtime and tooling add:

- a cross-platform PTY behind a Hera-owned boundary
- direct argv execution or explicit shell execution
- resize, input/output, exit, timeout, backpressure, and recordings
- golden fixtures, deterministic replay, and snapshot comparison
- machine-readable milestone evidence generation and validation

Sixel support intentionally stops at parsing and metadata. Hera does not render
image protocols yet.

## Workspace Architecture

| Crate | Responsibility |
|---|---|
| `terminal-core` | Parser integration, terminal state, scrollback, resize, and snapshots |
| `terminal-protocol` | Structured VT actions and payloads without leaking `vte` types |
| `terminal-render-model` | Renderer-neutral viewport, cells, styles, damage, cursor, and placeholders |
| `terminal-pty` | Process IO, resize, lifecycle, and platform transport |
| `terminal-fixtures` | Fixtures, replays, snapshots, and evidence schemas |
| `terminal-cli` | Local debugging, PTY execution, replay, benchmarks, and evidence validation |

The central dependency flow stays one-way:

```text
bytes -> terminal-core -> RenderSnapshot -> host renderer
             ^
             |
       terminal-pty events
```

`terminal-core` does not depend on PTY, GPUI, Paneflow, windowing, or platform
APIs. `terminal-render-model` does not depend on a concrete renderer.

## Quick Start

Prerequisite: Rust 1.85 or newer.

```powershell
cargo check --workspace
cargo test --workspace
cargo run -p terminal-core --example headless_embedder
cargo run -p terminal-cli -- replay crates/terminal-fixtures/fixtures/m1-golden.json
```

Run a real command through a PTY on Windows:

```powershell
cargo run -p terminal-cli -- run -- cmd.exe /D /C "echo Hera"
```

On Linux or macOS:

```sh
cargo run -p terminal-cli -- run -- /bin/sh -lc "printf 'Hera\n'"
```

With no arguments, `terminal-cli` prints the full set of debugging, replay,
benchmark, and validation commands:

```powershell
cargo run -p terminal-cli --
```

## Headless Embedding

The minimal API consumes bytes and produces a neutral snapshot:

```rust
use terminal_core::{ScrollbackConfig, Terminal, TerminalConfig};

let config = TerminalConfig::with_scrollback(
    80,
    24,
    ScrollbackConfig::new(10_000, 8 * 1024 * 1024),
)?;
let mut terminal = Terminal::with_config(config);

terminal.advance_bytes(b"cargo test\r\nrunning 1 test\r\n");
terminal.resize(100, 30)?;

let snapshot = terminal.render_snapshot();
println!("rows={}", snapshot.viewport_rows().len());
```

Complete examples:

- [`crates/terminal-core/examples/headless_embedder.rs`](crates/terminal-core/examples/headless_embedder.rs)
- [`crates/terminal-pty/examples/pty_boundary.rs`](crates/terminal-pty/examples/pty_boundary.rs)

## Validation and Evidence

Terminal correctness is backed by raw inputs and golden snapshots, not trust in
one local shell. The M5 matrix covers C0 controls, CUP/HVP, ED/EL/ECH,
scrollback, SGR, alternate screen behavior, resize/reflow, and bracketed paste.

Primary validation commands:

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo doc --workspace --no-deps
cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json
cargo run -p terminal-cli -- validate-m5-evidence evidence/m5/evidence-manifest.json
```

Live PTY scenarios are separate from the normal workspace test suite:

```powershell
cargo test -p terminal-pty --features live-pty-tests --test live_pty -- --ignored
```

## M6 Direction

M6 measures a precise boundary: Hera becomes the authoritative visual source for
selected Paneflow panes while the existing PTY, input-mode authority, and
default path remain controlled. Before any expansion, the canary must prove zero
P0 mismatches, zero fallbacks, zero dropped bytes, bounded latency, and memory
usage comparable to the Alacritty control.

The first Windows run did not pass. Work stays inside targeted host hardening
until the comparison mismatches and bounded-queue overflow are fixed and the
same canary passes again. Default replacement remains prohibited, and Linux and
macOS behavior is still unmeasured.

## Documentation

- [`docs/research-map.md`](docs/research-map.md): decision register and reference architecture
- [`docs/m5-compatibility-release-hardening-report.md`](docs/m5-compatibility-release-hardening-report.md): historical M5 closeout
- [`docs/m6-paneflow-controlled-host-replacement-report.md`](docs/m6-paneflow-controlled-host-replacement-report.md): M6 canary and exit decision
- [`docs/m4-public-proof-report.md`](docs/m4-public-proof-report.md): M4 public proof
- [`docs/reference-inventory/`](docs/reference-inventory/): per-engine inventories
- [`evidence/m5/`](evidence/m5/): machine-readable M5 evidence
- [`evidence/m6/`](evidence/m6/): scrubbed M6 host metrics and exit evidence
- [`tasks/`](tasks/): milestone PRDs and status trackers

## Non-Negotiable Principles

- Rust-first and Rust-public.
- Terminal correctness before product surface area.
- Small, stable, renderer-neutral public APIs.
- Explicitly bounded scrollback, never "infinite" scrollback.
- Snapshots and replay treated as foundational capabilities.
- Optional, non-authoritative agent-aware features.
- No Paneflow, GPUI, PTY, or platform types in `terminal-core`.
- Privileged protocols are parsed as data and never executed by the host.
