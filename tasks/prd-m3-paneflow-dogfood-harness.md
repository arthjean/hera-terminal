[PRD]
# PRD: M3 Paneflow Dogfood Harness

## Changelog

| Version | Date | Author | Summary |
|---------|------|--------|---------|
| 1.0 | 2026-07-03 | Arthur Jean | Initial draft for Hera M3 Paneflow side-by-side dogfood |

## Problem Statement

1. Hera M2 can execute real commands through a PTY harness and replay checked-in recordings, but it has not been exercised inside a real host terminal application with GPUI rendering, user input, pane lifecycle, search, scrollback and agent workflows.
2. Paneflow already has a mature terminal surface built on `alacritty_terminal`, GPUI custom rendering, PTY I/O threads, agent lifecycle hooks and MCP pane reads. Replacing or extending that surface without side-by-side evidence would risk regressions in the product Arthur actually uses.
3. Hera's long-term thesis depends on huge scrollback, deterministic replay and agent-aware session inspection. Those claims need long-session dogfood from Codex CLI and Claude Code before public API or renderer commitments.
4. Without M3, future work can jump straight to a new renderer or app surface and blur the line between terminal correctness, Paneflow product behavior and experimental Hera integration.

**Why now:** M2 is DONE and verified: `terminal-cli run <command>` executes through PTY, produces snapshots, writes `hera.pty_recording` files and replays recordings offline. Paneflow is the natural dogfood host, and its current architecture already confines `alacritty_terminal` behind neutral types and a custom `TerminalElement`, which creates a measurable comparison seam.

## Overview

This PRD defines Hera M3: integrate Hera into Paneflow behind an explicit dogfood feature flag, without replacing Paneflow's current terminal path by default. The first version is a shadow integration: Paneflow continues to render and drive panes through its existing `alacritty_terminal` path, while a Hera shadow core consumes the same PTY output, resize and input metadata, emits snapshots, and records differences.

The renderer work is deliberately constrained. M3 must not build a new terminal app. It must map Hera's renderer-neutral `RenderSnapshot` into Paneflow's existing window-free layout and golden-test seams, then expose an opt-in diagnostic side-by-side surface only after the shadow comparator and recordings are useful.

M3 ends with a go/no-go report. The output is not "Hera replaces Alacritty". The output is evidence: compatibility gaps, replay recordings, memory numbers, latency numbers, rendering parity failures, and a concrete recommendation for M4.

## Goals

| Goal | Month-1 Target | Month-6 Target |
|------|---------------|----------------|
| Host integration safety | `hera-dogfood` feature is off by default and Paneflow behavior is unchanged when disabled | Hera can be enabled per-pane without changing global Paneflow startup behavior |
| Shadow correctness | 10 deterministic recordings replay with zero Hera panics and first-difference reports for mismatches | 50 recordings across Codex, Claude Code, shell and build-tool sessions replay through Hera |
| Render adapter proof | 12 window-free layout goldens map Hera snapshots to Paneflow layout inputs | Side-by-side GPUI diagnostic rendering covers cursor, colors, hyperlinks and scrollback for 80x24 and 120x40 |
| Long-session evidence | 2 long sessions captured: one Codex CLI and one Claude Code, each >=10,000 logical output lines | 100,000-line and 1,000,000-line scenarios have memory profiles and scrollback policy decisions |
| Performance budget | Dogfood shadow path adds <=2 ms P95 per PTY event batch on Arthur's current dev machine for 80x24, 10k scrollback | Dogfood path supports 100k-line sessions with <=256 MB additional process memory while enabled |

## Target Users

### Hera Implementer

- **Role:** Arthur or an implementation agent proving Hera inside a real host.
- **Behaviors:** Works across `C:\dev\hera-terminal` and `C:\dev\paneflow`, implements story-scoped integration, validates with both workspaces.
- **Pain points:** Hera can pass headless tests while still failing under real pane resize, rendering, scrollback, keyboard input and agent output pressure.
- **Current workaround:** Use `terminal-cli run` recordings and manually infer whether the behavior would survive inside Paneflow.
- **Success looks like:** A feature-flagged dogfood path produces snapshot diffs, recordings and metrics without destabilizing Paneflow.

### Paneflow Maintainer

- **Role:** Arthur maintaining a production-grade Rust/GPUI app for agent terminals.
- **Behaviors:** Uses Paneflow as a daily tool, cares about terminal reliability, cross-platform support, UI latency and no hidden regressions.
- **Pain points:** Terminal regressions are expensive because Paneflow is a coordination surface for real coding agents, not a demo app.
- **Current workaround:** Keep `alacritty_terminal` as the authoritative path and avoid swapping terminal internals without evidence.
- **Success looks like:** The existing terminal remains authoritative until Hera proves parity through recordings, layout tests and long-session metrics.

### Future Embedder

- **Role:** A future GUI, TUI, remote-session or product host consuming Hera.
- **Behaviors:** Needs a small API surface: byte ingestion, resize, render snapshot, replay, semantic events and bounded history.
- **Pain points:** Existing terminal engines tend to entangle renderer, PTY, multiplexer, product state and platform concerns.
- **Current workaround:** Adapt a product terminal's internals or write an isolated emulator.
- **Success looks like:** M3 documents the smallest host API that served Paneflow without adding Paneflow types to Hera core.

## Research Findings

Key findings that informed this PRD:

### Competitive Context

- Paneflow: Uses Rust, GPUI, upstream `alacritty_terminal`, custom `TerminalElement`, PTY I/O threads and neutral wrapper types. This makes it the best dogfood host because it already has a renderer and an engine boundary.
- xterm.js: Exposes parser hooks and embeddable terminal APIs, but hook execution can affect parsing flow. Hera should keep extension points outside terminal-correct hot paths: https://xtermjs.org/docs/guides/hooks/.
- VS Code integrated terminal: Shell integration powers command decorations, command navigation and sticky scroll. This validates agent/session metadata as a product direction, but also shows semantics should be derived on top of terminal state: https://code.visualstudio.com/docs/terminal/shell-integration.
- GPUI/Zed: GPUI is a Rust framework built around `Render`, app/window contexts and explicit updates. Paneflow already follows that model, so Hera should adapt into Paneflow's existing view/element seams instead of importing GPUI into Hera: https://gpui.rs/ and https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md.
- **Market gap:** A terminal engine dogfood loop that keeps a production terminal path alive while collecting deterministic cross-engine evidence.

### Best Practices Applied

- Keep ConPTY and PTY lifecycle concerns isolated. Microsoft warns that pseudoconsole channels need separate servicing to avoid deadlocks, which supports keeping M3 on Paneflow's proven PTY path first: https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session.
- Keep PTY APIs behind stable domain types. `portable-pty` exposes open, spawn, reader, writer, resize and child-wait capabilities, matching M2 and Paneflow's current platform strategy: https://docs.rs/portable-pty/latest/portable_pty/.
- Use compatibility fixtures and deterministic tests before trusting local behavior. VTTEST and esctest2 remain relevant external references for terminal correctness beyond one shell: https://invisible-island.net/vttest/ and https://github.com/ThomasDickey/esctest2.
- Use window-free render goldens before visual rollout. Paneflow already has `TerminalElement::layout_from_snapshot` and `golden_frame` tests, which is the correct seam for Hera render-model mapping.

*Full local research sources are in `docs/research-map.md`, `docs/reference-inventory/`, `C:\dev\paneflow\ARCHITECTURE.md` and `C:\dev\paneflow\src-app\src\terminal\`.* Context7 lookup confirmed `/websites/rs_gpui_gpui` for GPUI and `/websites/rs_portable-pty` for PTY design.

## Assumptions & Constraints

### Assumptions (to validate)

- Paneflow can add Hera as a local path dependency behind a feature flag without disturbing default release builds.
- Existing Paneflow PTY output can be tapped or mirrored without changing command execution semantics.
- Hera `RenderSnapshot` has enough data to drive a first Paneflow window-free layout adapter for text, style, cursor, dimensions and active screen.
- The first side-by-side comparison can use text/cell/cursor/screen/scrollback summaries before byte-for-byte visual parity.
- Long Codex/Claude recordings can be kept local or checked in only when they are small, deterministic and scrubbed of private paths or prompts.

### Hard Constraints

- `terminal-core`, `terminal-protocol` and `terminal-render-model` must not depend on GPUI, Paneflow, `alacritty_terminal`, PTY crates or platform APIs.
- Paneflow's existing `alacritty_terminal` path remains authoritative until M3 produces a written go/no-go report.
- M3 must be disabled by default in Paneflow release builds and enabled only by explicit feature flag plus runtime opt-in.
- No terminal content, prompts, paths or agent transcripts may be sent to telemetry or external services.
- M3 must not require replacing Paneflow's IPC, MCP bridge, updater, telemetry, packaging or agent hook architecture.
- New code must preserve Paneflow's Linux, macOS and Windows compatibility policy.

## Quality Gates

These commands must pass for every user story:

- `cargo fmt --all -- --check` - Hera workspace formatting is stable.
- `cargo check --workspace` - Hera workspace typechecks.
- `cargo clippy --workspace --all-targets -- -D warnings` - Hera lints are blocking.
- `cargo test --workspace` - Hera unit, fixture and replay tests pass.
- `cargo fmt --check` from `C:\dev\paneflow` - Paneflow formatting is stable.
- `cargo check --workspace --features hera-dogfood` from `C:\dev\paneflow` - Paneflow dogfood feature typechecks.
- `cargo clippy --workspace --features hera-dogfood -- -D warnings` from `C:\dev\paneflow` - Paneflow lints are blocking.
- `cargo test --workspace --features hera-dogfood` from `C:\dev\paneflow` - Paneflow dogfood tests pass.

For stories touching GPUI rendering:

- `cargo test -p paneflow-app --features hera-dogfood golden_frame` from `C:\dev\paneflow` - window-free terminal layout goldens pass or intentionally regenerate with a documented diff.

## Epics & User Stories

### EP-001: Feature-Flagged Integration Boundary

Establish a cross-repo boundary where Paneflow can consume Hera for dogfood without changing default product behavior.

**Definition of Done:** Paneflow can compile with `hera-dogfood`, default builds remain unchanged, and no Hera crate imports Paneflow or GPUI.

#### US-001: Add Paneflow `hera-dogfood` Feature And Local Hera Dependencies

**Description:** As a Paneflow maintainer, I want Hera dependencies behind an explicit feature so that dogfood can be compiled without affecting default releases.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** None

**Acceptance Criteria:**

- [ ] Given `C:\dev\paneflow\src-app\Cargo.toml`, when `hera-dogfood` is disabled, then no Hera crate is compiled into the default `paneflow-app` dependency graph.
- [ ] Given `hera-dogfood` is enabled, when `cargo check --workspace --features hera-dogfood` runs from `C:\dev\paneflow`, then Paneflow compiles with local path dependencies to Hera crates.
- [ ] Given Hera manifests are inspected, when dependencies are read, then no Hera crate depends on Paneflow, GPUI or `alacritty_terminal`.
- [ ] Given the local Hera path is missing, when the feature is enabled, then the build fails with a clear dependency error and default Paneflow builds still compile.

#### US-002: Define Paneflow Hera Adapter Module Boundary

**Description:** As a host-adapter implementer, I want a dedicated Paneflow module boundary so that Hera dogfood code cannot leak into the rest of the app.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**

- [ ] Given `hera-dogfood` is enabled, when source files are inspected, then Hera imports are confined to `src-app/src/terminal/hera_dogfood/` and an explicit allowlist test.
- [ ] Given `hera-dogfood` is disabled, when any Paneflow terminal module is compiled, then references to Hera adapter types are cfg-gated out.
- [ ] Given a developer imports Hera from an unapproved Paneflow file, when tests run, then a confinement guard fails with the offending path.
- [ ] Given the adapter receives an unsupported Hera snapshot field, when mapping runs, then it records an adapter diagnostic instead of panicking.

#### US-003: Add Runtime Opt-In And Diagnostics Gate

**Description:** As a Paneflow user, I want Hera dogfood disabled unless I explicitly opt in so that normal terminal sessions are not affected.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** Blocked by US-001, US-002

**Acceptance Criteria:**

- [ ] Given `hera-dogfood` is compiled but `PANEFLOW_HERA_DOGFOOD` is unset, when Paneflow starts, then no Hera shadow session is created.
- [ ] Given `PANEFLOW_HERA_DOGFOOD=shadow`, when a new terminal pane opens, then a Hera shadow session is attached and diagnostics are logged locally.
- [ ] Given `PANEFLOW_HERA_DOGFOOD` has an unknown value, when Paneflow starts, then it logs one warning and falls back to disabled mode.
- [ ] Given dogfood diagnostics are enabled, when terminal content is recorded, then prompts, paths and raw terminal bytes are written only to the configured local artifact path.

### EP-002: Shadow Core From Paneflow PTY Events

Feed Hera with the same terminal activity Paneflow already receives, while preserving Paneflow's current PTY and renderer path.

**Definition of Done:** A Paneflow terminal pane can mirror output, resize and input metadata into Hera and produce deterministic snapshots without affecting the authoritative terminal.

#### US-004: Tap PTY Output Into Hera Shadow Session

**Description:** As a Hera implementer, I want Paneflow PTY output chunks mirrored into a shadow `terminal_core::Terminal` so that real panes produce Hera snapshots.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-002, US-003

**Acceptance Criteria:**

- [ ] Given a Paneflow pane prints a known token, when dogfood shadow mode is enabled, then Hera receives the same output bytes and its final snapshot contains the token.
- [ ] Given a PTY chunk arrives while Hera shadow processing is still busy, when the next batch is queued, then the adapter applies bounded backpressure and records byte counts.
- [ ] Given Hera `advance_bytes` sees malformed or split UTF-8, when the pane continues outputting, then Paneflow's authoritative terminal remains unaffected.
- [ ] Given dogfood shadow mode is disabled, when PTY output flows, then no Hera byte tap code executes.

#### US-005: Mirror Resize And Dimension Policy

**Description:** As a terminal user, I want Hera dimensions to track Paneflow pane dimensions so that snapshot comparisons are meaningful.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-004

**Acceptance Criteria:**

- [ ] Given Paneflow creates a terminal with N columns and M rows, when Hera shadow session starts, then Hera initializes with N columns and M rows.
- [ ] Given Paneflow resizes a pane, when the authoritative terminal resize succeeds, then Hera receives the same character dimensions before the next comparison checkpoint.
- [ ] Given Hera rejects a resize as invalid, when Paneflow continues running, then the adapter records a resize diagnostic and disables only the affected shadow session.
- [ ] Given pixel dimensions change without character-cell dimensions changing, when Paneflow repaints, then Hera does not receive a spurious resize event.

#### US-006: Mirror Input Metadata Without Double-Writing PTY

**Description:** As a Paneflow maintainer, I want user input and scripted input observed by Hera without writing twice to the child process.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-004

**Acceptance Criteria:**

- [ ] Given a user types into a Paneflow pane, when dogfood shadow mode is enabled, then Paneflow writes input once to the PTY and Hera records input metadata separately.
- [ ] Given paste or auto-submit sends more than 64 KB, when input metadata is recorded, then the adapter stores a bounded summary and byte count instead of unbounded text.
- [ ] Given input recording fails, when the key event continues, then the authoritative Paneflow PTY write still occurs.
- [ ] Given dogfood diagnostics are exported, when raw input bytes contain control characters, then any human-readable summary escapes them.

#### US-007: Isolate Shadow Lifecycle Failures

**Description:** As a Paneflow user, I want Hera failures to be isolated so that dogfood cannot crash active work.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-004, US-005, US-006

**Acceptance Criteria:**

- [ ] Given Hera shadow session returns a typed error, when Paneflow processes the event, then only that shadow session is disabled and the pane remains usable.
- [ ] Given the dogfood event queue exceeds its configured capacity, when overflow occurs, then Paneflow records dropped byte counts and keeps the authoritative terminal alive.
- [ ] Given a pane is closed, when dogfood mode is active, then Hera shadow state is dropped within 1 second and no background writer remains.
- [ ] Given a panic would occur inside dogfood-only code, when tests run, then the scenario is covered by a non-panicking error path or explicit test guard.

### EP-003: Snapshot Comparator And Replay Evidence

Build the evidence loop: compare Paneflow's current terminal surface with Hera snapshots, then preserve mismatches as replayable artifacts.

**Definition of Done:** M3 can produce local mismatch reports and replay recordings that identify the first differing field between Hera and Paneflow summaries.

#### US-008: Define Neutral Comparison Summary

**Description:** As a dogfood reviewer, I want a neutral comparison model so that Alacritty and Hera snapshots can be compared without leaking either engine's internals.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-004, US-005

**Acceptance Criteria:**

- [ ] Given Paneflow has a renderable terminal content snapshot, when comparison summary is built, then it includes dimensions, active screen if available, cursor, viewport lines, style buckets and scrollback line count.
- [ ] Given Hera has a `RenderSnapshot`, when comparison summary is built, then it maps to the same field names as the Paneflow summary.
- [ ] Given a field is unsupported by one engine, when comparison runs, then the field is marked `unsupported` instead of being treated as equal.
- [ ] Given two summaries differ, when diffing runs, then the first differing field path and compact values are reported.

#### US-009: Add Runtime Diff Counters And Local Report Files

**Description:** As a maintainer, I want per-pane diff counters and local reports so that dogfood produces actionable evidence without exposing terminal content externally.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-008

**Acceptance Criteria:**

- [ ] Given shadow mode is enabled, when a comparison checkpoint runs, then Paneflow increments per-pane counters for equal, mismatch, unsupported and shadow-disabled states.
- [ ] Given a mismatch occurs, when artifact output is configured, then a local JSON report stores pane id, timestamp, field path, dimensions, redacted command metadata and bounded line excerpts.
- [ ] Given artifact output path is missing or unwritable, when a mismatch occurs, then Paneflow logs the filesystem error once per pane and continues without dogfood artifacts.
- [ ] Given terminal output contains private-looking paths, when report excerpts are enabled, then report generation applies the configured redaction policy or omits excerpts.

#### US-010: Capture M3 Dogfood Recordings

**Description:** As a Hera implementer, I want dogfood recordings from Paneflow panes so that real sessions can be replayed outside the GUI.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-004, US-005, US-006, US-009

**Acceptance Criteria:**

- [ ] Given recording mode is enabled for a pane, when PTY output, input metadata, resize and lifecycle events occur, then a bounded recording file is written locally.
- [ ] Given recording output exceeds 64 MB, when the cap is reached, then recording truncates future raw output, preserves metadata and marks `output_truncated=true`.
- [ ] Given a pane exits non-zero, when recording finalizes, then exit metadata and the last Hera snapshot are still written.
- [ ] Given recording contains prompts or paths, when artifact retention is configured as `scrubbed`, then the checked-in artifact omits raw sensitive text and keeps only deterministic synthetic fixtures.

#### US-011: Add Replay Tests For Captured Dogfood Fixtures

**Description:** As a regression tester, I want captured dogfood fixtures replayed through Hera so that M3 evidence survives outside Paneflow.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-010

**Acceptance Criteria:**

- [ ] Given a checked-in M3 fixture, when Hera replay tests run, then it replays output and resize events into `terminal-core` and compares the final snapshot.
- [ ] Given a fixture was captured from Paneflow and scrubbed, when schema validation runs, then required metadata still identifies source, dimensions, event counts and redaction status.
- [ ] Given a fixture final snapshot is wrong, when tests run, then failure output identifies the first differing field path.
- [ ] Given no private dogfood recordings are checked in, when the test suite runs, then it still passes using synthetic or scrubbed fixtures.

### EP-004: Minimal GPUI Render Adapter

Prove that Hera's render model can feed Paneflow's GPUI rendering seams without replacing the app's terminal.

**Definition of Done:** Hera snapshots can be converted into Paneflow window-free layout inputs, tested with goldens, and optionally rendered in a diagnostic side-by-side surface.

#### US-012: Map Hera RenderSnapshot To Paneflow Layout Inputs

**Description:** As a renderer implementer, I want Hera snapshots converted into Paneflow layout inputs so that GPUI rendering can be tested without a window.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-008

**Acceptance Criteria:**

- [ ] Given a Hera snapshot with plain text, colors and cursor, when converted, then Paneflow `layout_from_snapshot` receives equivalent rows, styles and cursor data.
- [ ] Given Hera snapshot contains an image placeholder or unsupported style field, when converted, then the adapter emits a deterministic placeholder diagnostic.
- [ ] Given a snapshot has dimensions over Paneflow's rendering caps, when conversion runs, then it returns a typed adapter error before allocating full layout state.
- [ ] Given `hera-dogfood` is disabled, when Paneflow render code compiles, then no Hera layout adapter code is linked into default builds.

#### US-013: Add Hera Window-Free Golden Corpus

**Description:** As a Paneflow maintainer, I want deterministic goldens for Hera layout mapping so that visual drift is caught before live rendering.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-012

**Acceptance Criteria:**

- [ ] Given `cargo test -p paneflow-app --features hera-dogfood golden_frame` runs, then at least 12 Hera-sourced layout goldens pass.
- [ ] Given a golden drifts, when tests fail, then failure output names the fixture and regeneration command.
- [ ] Given OS-sensitive font metrics differ, when the golden corpus is run on unsupported OS lanes, then the test is ignored with a documented reason rather than silently passing.
- [ ] Given an unsupported Hera field appears in a fixture, when the golden is rendered, then the placeholder diagnostic is present in the golden text.

#### US-014: Add Diagnostic Side-By-Side Render Surface

**Description:** As a dogfood reviewer, I want an opt-in diagnostic surface so that Hera and Paneflow rendering can be visually compared during real sessions.

**Priority:** P1
**Size:** L (5 pts)
**Dependencies:** Blocked by US-012, US-013

**Acceptance Criteria:**

- [ ] Given `PANEFLOW_HERA_DOGFOOD=side_by_side`, when a terminal pane opens, then the existing Paneflow render remains primary and Hera render appears only in the diagnostic surface.
- [ ] Given the Hera render adapter fails, when the diagnostic surface is active, then Paneflow hides the Hera surface for that pane and shows a local diagnostic counter.
- [ ] Given the user disables dogfood mode and restarts, when the same workspace opens, then no side-by-side UI is visible.
- [ ] Given side-by-side render is active, when a pane receives rapid output, then the existing Paneflow render remains interactive and Hera rendering may skip frames with a counted reason.

#### US-015: Route Resize, Focus And Basic Input Through Existing Paneflow Controls

**Description:** As a Paneflow user, I want the diagnostic Hera surface to follow existing terminal focus and resize behavior without creating a separate input model.

**Priority:** P1
**Size:** M (3 pts)
**Dependencies:** Blocked by US-014

**Acceptance Criteria:**

- [ ] Given a pane is focused, when the side-by-side surface is visible, then keyboard input still routes only through the existing `TerminalView` path.
- [ ] Given a pane is resized, when the side-by-side surface is visible, then Hera render dimensions update from the same character-cell size as the authoritative terminal.
- [ ] Given IME preedit, mouse selection or copy mode is active, when side-by-side is visible, then unsupported Hera interactions are displayed as diagnostics and do not intercept input.
- [ ] Given focus changes between panes, when dogfood mode is active, then dogfood diagnostics follow the active pane without mutating saved workspace layout.

### EP-005: Long-Session Dogfood And Go/No-Go Report

Use real agent sessions to decide what Hera must fix before deeper Paneflow integration or public proof.

**Definition of Done:** M3 ends with captured sessions, memory and latency numbers, a compatibility gap list and a written recommendation for the next milestone.

#### US-016: Capture Codex And Claude Code Long Sessions

**Description:** As a Hera maintainer, I want long sessions from real coding agents so that scrollback and replay pressure are based on actual workflows.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-010, US-011

**Acceptance Criteria:**

- [ ] Given dogfood recording mode is enabled, when a Codex CLI session produces >=10,000 logical output lines, then a local recording and summary metrics file are produced.
- [ ] Given dogfood recording mode is enabled, when a Claude Code session produces >=10,000 logical output lines, then a local recording and summary metrics file are produced.
- [ ] Given a session includes private prompts, file paths or secrets, when preparing checked-in artifacts, then raw recordings are not committed and a scrubbed synthetic derivative is used instead.
- [ ] Given a long session exceeds recording caps, when finalizing metrics, then truncation is explicit and line/byte counts are still reported.

#### US-017: Measure Memory, Latency And Diff Rate

**Description:** As a decision maker, I want measured dogfood overhead so that M4 scope is based on data.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-009, US-016

**Acceptance Criteria:**

- [ ] Given a 10k-line dogfood session, when metrics are collected, then the report includes Paneflow RSS baseline, dogfood RSS delta, PTY batch P50/P95/P99 and total mismatch count.
- [ ] Given `PANEFLOW_LATENCY_PROBE=1` is enabled, when dogfood side-by-side mode runs, then time-to-pixel samples include whether Hera rendered, skipped or errored for each sampled batch.
- [ ] Given metrics collection fails on an OS, when the report is generated, then that OS is marked `not measured` with the failing command and no fabricated number.
- [ ] Given dogfood overhead exceeds <=2 ms P95 batch cost or <=64 MB memory delta for 10k lines, when the report is generated, then M4 replacement work is marked blocked.

#### US-018: Write M3 Go/No-Go Report

**Description:** As Arthur, I want a concise M3 decision report so that the next milestone is selected from evidence instead of momentum.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-011, US-013, US-017

**Acceptance Criteria:**

- [ ] Given M3 stories are implemented, when the report is written, then it lists compatibility gaps, replay gaps, render gaps, memory numbers, latency numbers and required M4 work.
- [ ] Given any P0 mismatch remains unexplained, when the report is written, then it recommends "continue dogfood" rather than "replace Paneflow terminal path".
- [ ] Given all M3 target metrics pass, when the report is written, then it recommends the next PRD scope with explicit files and risk areas.
- [ ] Given dogfood artifacts include private data, when the report references them, then it points to local-only paths and includes scrubbed summaries only.

## Functional Requirements

- FR-01: Paneflow must expose a compile-time `hera-dogfood` feature that is disabled by default.
- FR-02: Dogfood runtime must require an explicit runtime opt-in such as `PANEFLOW_HERA_DOGFOOD`.
- FR-03: Paneflow must keep its current `alacritty_terminal` path authoritative during M3.
- FR-04: Hera shadow sessions must consume mirrored PTY output and resize events from Paneflow panes.
- FR-05: Hera shadow failures must not crash Paneflow or block input to the authoritative terminal.
- FR-06: M3 must produce comparison summaries for Hera and Paneflow terminal state.
- FR-07: M3 must produce local mismatch reports with bounded, redacted excerpts.
- FR-08: M3 must support local dogfood recordings with byte, event and file-size caps.
- FR-09: M3 must include replay tests for checked-in scrubbed or synthetic recordings.
- FR-10: M3 must map Hera `RenderSnapshot` data into Paneflow window-free layout tests.
- FR-11: M3 must include at least one opt-in diagnostic side-by-side render mode.
- FR-12: M3 must end with a go/no-go report before any PRD replaces Paneflow's terminal path.

## Non-Functional Requirements

- **Performance:** With dogfood shadow enabled on Arthur's current dev machine, PTY batch processing overhead must be <=2 ms P95 for an 80x24 pane with 10,000 scrollback lines.
- **Memory:** With dogfood shadow enabled, additional process memory must be <=64 MB for 10,000-line sessions and <=256 MB for 100,000-line replay scenarios.
- **Reliability:** A Hera shadow error must disable only the affected shadow session within 1 event batch and leave the Paneflow pane usable.
- **Recording Bounds:** Dogfood recording files must default to <=64 MB raw output per pane and mark truncation explicitly.
- **Security:** No dogfood artifact may be uploaded or sent through telemetry; raw terminal bytes stay local, and checked-in fixtures must be scrubbed or synthetic.
- **Portability:** Paneflow dogfood code must typecheck on Linux, macOS and Windows target cfgs; OS-specific behavior must be behind explicit `cfg(target_os)` gates.
- **Compatibility:** When `hera-dogfood` is disabled, `cargo check --workspace` from Paneflow must compile without Hera dependencies.
- **Determinism:** Checked-in M3 replay fixtures must produce byte-identical Hera snapshots across 2 consecutive runs.

## Edge Cases & Error States

Systematic coverage of unhappy paths. Evidence shows earlier defect discovery significantly reduces cost.

| # | Scenario | Trigger | Expected Behavior | User Message |
|---|----------|---------|-------------------|--------------|
| 1 | Dogfood disabled | Feature compiled but env unset | No shadow session, default terminal path only | N/A |
| 2 | Unknown dogfood mode | Invalid `PANEFLOW_HERA_DOGFOOD` value | Warn once, disable dogfood | "unknown Hera dogfood mode" |
| 3 | Hera local dependency missing | `hera-dogfood` enabled without Hera path | Feature build fails, default build unaffected | Cargo dependency error |
| 4 | PTY byte tap overflow | Shadow queue exceeds cap | Drop counted bytes, keep Paneflow terminal alive | Local diagnostic only |
| 5 | Hera parser/runtime error | Shadow `advance_bytes` or resize fails | Disable affected shadow session | Local diagnostic only |
| 6 | Snapshot mismatch | Hera summary differs from Paneflow summary | Store first-difference report | "Hera mismatch recorded" in logs |
| 7 | Artifact path unwritable | Report or recording cannot be written | Log once per pane, continue session | "dogfood artifact write failed" |
| 8 | Private data in recordings | Prompt/path/secret-like content detected | Do not check in raw artifact, create scrubbed derivative | Local warning |
| 9 | Side-by-side render failure | Hera adapter cannot map snapshot | Hide Hera diagnostic surface for pane | Diagnostic counter |
| 10 | Long-session cap exceeded | Output exceeds recording cap | Truncate raw output, keep counts and final metadata | "recording truncated" |
| 11 | OS metrics unavailable | Memory or latency probe unsupported | Mark metric `not measured` with reason | Report entry |
| 12 | Paneflow pane closed | Active shadow session exists | Drop shadow state within 1 second | N/A |

## Risks & Mitigations

| # | Risk | Probability | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | Dogfood work destabilizes Paneflow's daily terminal path | Med | High | Compile-time feature plus runtime opt-in; Alacritty remains authoritative |
| 2 | Hera becomes Paneflow-specific too early | Med | High | Put adapter in Paneflow; forbid Paneflow and GPUI deps in Hera core crates |
| 3 | Snapshot comparison produces noisy false positives | High | Med | Start with neutral summaries and unsupported markers before visual parity claims |
| 4 | Long recordings capture private prompts or paths | Med | High | Local-only raw artifacts, scrubbed/synthetic checked-in fixtures, explicit redaction policy |
| 5 | Side-by-side rendering shifts focus or input behavior | Med | High | Diagnostic surface never owns input; existing `TerminalView` remains sole input path |
| 6 | Memory overhead hides scrollback design problems | High | High | Add memory delta targets and mark M4 blocked if budgets fail |
| 7 | Cross-platform cfgs drift because local dogfood is on Windows only | Med | Med | Paneflow feature checks must typecheck all target-gated paths and document unmeasured OS metrics |

## Non-Goals

Explicit boundaries: what this version does NOT include:

- No replacement of Paneflow's default `alacritty_terminal` path.
- No new standalone Hera desktop terminal app.
- No public Hera API that imports Paneflow, GPUI, `alacritty_terminal` or Paneflow-specific agent types.
- No terminal multiplexer, remote session protocol or persistent PTY daemon.
- No authoritative command detection or shell integration replacement.
- No telemetry upload of terminal content, prompts, paths, recordings or diff reports.
- No full image protocol rendering; unsupported image metadata may remain placeholders.
- No public release claim that Hera is production-ready until the M3 report says so.

## Files NOT to Modify

- `crates/terminal-core/Cargo.toml` - Do not add GPUI, Paneflow, PTY, platform or `alacritty_terminal` dependencies.
- `crates/terminal-render-model/Cargo.toml` - Keep renderer-neutral; do not add GPUI or Paneflow types.
- `crates/terminal-pty/*` - Do not replace M2 PTY runtime for M3; Paneflow dogfood taps the existing host path first.
- `docs/reference-inventory/*` - Evidence files should not be rewritten unless new research changes a durable decision.
- `C:\dev\paneflow\crates\paneflow-shim\*` - Agent shim behavior is outside M3 unless explicitly needed by a recording story.
- `C:\dev\paneflow\crates\paneflow-mcp\*` - MCP read bridge is not part of the Hera renderer dogfood.
- `C:\dev\paneflow\crates\paneflow-telemetry\*` - Dogfood artifacts must stay local and must not expand telemetry.
- `C:\dev\paneflow\packaging\*` - Packaging and updater behavior stay out of M3.
- Reference repos under `C:\dev\terminal-research`, `C:\dev\wezterm`, `C:\dev\xterm.js`, `C:\dev\terminal` and related clones - Read-only references, not vendored Hera code.

## Technical Considerations

Frame as questions for engineering input, not mandates:

- **Cross-Repo Dependency:** Should Paneflow consume Hera through local path dependencies or a temporary git dependency? Recommended: local path dependencies under `hera-dogfood` for M3, because both repos are local and unpublished.
- **Byte Tap Location:** Should the tap live at Paneflow's PTY event loop boundary or after `alacritty_terminal` processing? Recommended: tap raw PTY output before Alacritty parsing so Hera receives the same byte stream.
- **Comparator Scope:** Should M3 compare full cell/style state or start with viewport text and cursor? Recommended: start with dimensions, cursor, viewport text, scrollback count and style buckets, then expand as false positives shrink.
- **Recording Format:** Should M3 reuse `hera.pty_recording` or define a Paneflow dogfood schema? Recommended: extend through a separate M3 dogfood wrapper that can be reduced into `hera.pty_recording` for replay.
- **Render Adapter:** Should Hera map directly into `TerminalElement` paint types or into Paneflow's window-free `LayoutInputs`? Recommended: `LayoutInputs` first to keep tests deterministic and avoid a live GPUI dependency inside Hera.
- **Runtime Toggle:** Should opt-in be env var only or also config-driven? Recommended: env var for M3 to avoid writing persistent config for experimental dogfood.
- **Artifact Storage:** Where should local raw recordings live? Recommended: under a gitignored dogfood artifact directory with explicit size caps and scrubbed derivatives for checked-in fixtures.
- **M4 Decision:** Should passing M3 automatically lead to replacing Alacritty? Recommended: no. M3 must write a report, then M4 PRD decides between deeper dogfood, renderer work, scrollback storage or semantic timeline.

## Success Metrics

| Metric | Baseline (current) | Target | Timeframe | How Measured |
|--------|-------------------|--------|-----------|-------------|
| Paneflow default behavior | No Hera dependency | 0 Hera code compiled when `hera-dogfood` is disabled | End of US-001 | `cargo tree -p paneflow-app` without feature |
| Hera shadow token proof | No host integration | Known token appears in Hera snapshot from a Paneflow pane | End of US-004 | Dogfood unit/integration test |
| Snapshot comparison coverage | No comparison model | 8 summary fields compared or marked unsupported | End of US-008 | Comparator tests |
| Checked-in M3 fixtures | 0 M3 dogfood fixtures | 10 scrubbed/synthetic fixtures replay deterministically | End of US-011 | Hera replay tests |
| Hera layout goldens | 0 Hera-sourced Paneflow goldens | 12 goldens passing | End of US-013 | Paneflow `golden_frame` tests |
| Long-session captures | 0 Paneflow Hera captures | 2 sessions >=10,000 logical output lines | End of US-016 | Local metrics report |
| Dogfood overhead | N/A | <=2 ms P95 PTY batch overhead and <=64 MB memory delta at 10k lines | End of US-017 | Latency probe and memory measurement |
| M3 decision | No go/no-go report | 1 report with recommendation and blockers | End of US-018 | `docs/m3-paneflow-dogfood-report.md` or equivalent |

## Open Questions

- Should M3 artifacts live under `C:\dev\hera-terminal\artifacts\m3-dogfood\` or Paneflow's `.paneflow-audit` directory? Owner: implementing engineer. Needed before US-010.
- What exact redaction policy should be applied to private paths and prompts in checked-in derivatives? Owner: implementing engineer. Needed before US-009.
- Should the first side-by-side UI be a split pane, overlay, or debug-only hidden panel? Owner: implementing engineer. Needed before US-014.
- Which OSes will be actually measured for the first M3 report? Owner: Arthur or implementing engineer. Needed before US-017.
[/PRD]
