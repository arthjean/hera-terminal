[PRD]
# PRD: M6 Paneflow Controlled Host Replacement

## Changelog

| Version | Date | Author | Summary |
|---------|------|--------|---------|
| 1.0 | 2026-07-10 | Arthur Jean | Initial M6 controlled host replacement experiment PRD |
| 1.1 | 2026-07-10 | Arthur Jean | Add EP-007 targeted host hardening after the failed Windows canary |

## Problem Statement

1. M5 is complete with 18 of 18 stories `DONE`, and its isolated Windows GPUI shadow run completed 45 minutes with zero mismatch reports, but Hera still does not own any user-visible Paneflow terminal surface.
2. Paneflow's current `TerminalView`, `TerminalState`, input handling and `TerminalElement` are coupled to `alacritty_terminal` types. The existing Hera integration mirrors PTY output into a shadow `Terminal` and can adapt a Hera snapshot for diagnostics, but Alacritty remains the source painted in normal panes.
3. A direct global replacement would overstate the current evidence. M5 still records Linux and macOS as unmeasured, dependent package dry-runs as blocked, semver tooling as unavailable and security policy work as incomplete.
4. Hera cannot yet replace every Alacritty responsibility in one bounded milestone. Its public API provides byte ingestion, resize, cursor, screen identity, scrollback and render snapshots, but Paneflow still relies on Alacritty state for input mode encoding, PTY lifecycle and several host interaction paths.
5. The first M6 Windows canary completed 3,719 seconds but recorded 219 P0 mismatches, one unsupported checkpoint, 1,526,241 dropped Hera bytes and one fallback. The current 64-batch non-blocking tap drops output when render-coupled draining falls behind, and comparison checkpoints do not prove that Hera and Alacritty have applied the same PTY batch sequence and resize generation.

**Why now:** The M5 Windows shadow gate is closed with zero P0 mismatches, the Hera-to-Paneflow render adapter and golden corpus already exist, and an isolated Paneflow worktree now provides a safe place to make Hera authoritative for a visible pane without changing the normal checkout or default user path.

## Overview

M6 delivers the first real Paneflow terminal surface whose visible grid, styles, cursor and scrollback are sourced from Hera. The experiment runs only from `C:\dev\paneflow-hera-m6` on branch `feat/hera-m6-host-replacement`. It is compiled behind a new additive `hera-host` Cargo feature and selected for newly created panes with `PANEFLOW_TERMINAL_ENGINE=hera`. The default build and an absent or invalid selector continue to use Alacritty and must not require a Hera checkout.

M6 is a controlled render-authority replacement, not removal of the legacy engine. One PTY process feeds both states in identical order. A single Hera session is the authoritative source painted by the existing GPUI `TerminalElement`; the synchronized Alacritty state remains available for input mode handling, differential checks and immediate per-pane fallback. A Hera initialization, queue, resize or snapshot adaptation failure automatically returns that pane to Alacritty without restarting the child process.

The milestone ends with a visible Windows canary, Linux and macOS feature-gate evidence, a scrubbed M6 evidence package and a go/no-go report. Passing M6 may authorize a broader Hera canary. It does not authorize making Hera the Paneflow default, deleting Alacritty, publishing Hera crates or claiming that the full terminal runtime has been replaced.

EP-007 is the bounded recovery epic selected by the failed canary. It first classifies the observed mismatch fields, then makes comparison checkpoints sequence-coherent, moves Hera ingestion off the GPUI render cadence into a bounded lossless worker, proves the repair with deterministic burst and race fixtures, and repeats the Windows canary. It does not absorb Linux/macOS qualification from US-017 or widen the replacement boundary.

## Goals

| Goal | Month-1 Target | Month-6 Target |
|------|---------------|----------------|
| Make Hera visibly authoritative in Paneflow | One opt-in pane renders exclusively from a Hera snapshot while accepting keyboard input and resize | Three consecutive 60-minute canaries across at least five workload classes complete with zero P0 mismatch or fallback events |
| Preserve rollback safety | 100% of injected Hera init, resize, drain and snapshot failures return the affected pane to synchronized Alacritty within 100 ms | No user-visible session loss attributable to engine fallback across all recorded canaries |
| Bound runtime regression | PTY batch ingestion and adaptation P95 remains at or below 2.0 ms and paired-process RSS stays within 20% of the M5 shadow baseline | Same thresholds hold on Windows, Linux and macOS evidence runs before any broader rollout |
| Protect the default Paneflow path | Default workspace check and test commands pass without `PANEFLOW_HERA_TERMINAL_ROOT` | Hera remains opt-in until a later PRD explicitly changes the default |

## Target Users

### Paneflow Maintainer
- **Role:** Arthur or another maintainer validating Hera inside the real Paneflow host.
- **Behaviors:** Builds Rust workspaces locally, runs feature-gated GPUI builds, opens multiple terminal panes and inspects structured evidence before changing defaults.
- **Pain points:** Shadow evidence proves equivalence but does not prove that users can work through a Hera-painted terminal; a broad replacement would be hard to diagnose and reverse.
- **Current workaround:** Run `hera-dogfood` in shadow or side-by-side mode while Alacritty remains visible.
- **Success looks like:** A Hera-authoritative pane can run real interactive workloads, failures fall back locally, and a machine-readable report explains whether the next rollout stage is allowed.

### Paneflow Experimental User
- **Role:** A technical Paneflow user explicitly running the M6 worktree build.
- **Behaviors:** Uses PowerShell or another shell, Codex CLI, Claude Code, TUIs, long test output, selection, copy, search and pane resizing.
- **Pain points:** A terminal engine experiment that loses input, cursor state, scrollback or session continuity is unusable even if its static snapshots match.
- **Current workaround:** Use the default Alacritty-backed Paneflow terminal.
- **Success looks like:** The opt-in terminal behaves like the normal pane for the defined M6 workload set and silently retains a safe synchronized fallback.

## Research Findings

Key findings that informed this PRD:

### Competitive Context
- **GPUI:** GPUI separates retained `Entity<Render>` views from low-level `Element` layout, prepaint and paint phases. Paneflow already follows this shape, so Hera should feed the existing `TerminalElement` rather than introduce a second renderer. Source: https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md
- **xterm.js:** Its embedded terminal API exposes explicit resize, render, scroll, selection, focus and disposal lifecycles. These are the minimum host interactions an engine switch must preserve. Source: https://xtermjs.org/docs/api/terminal/classes/terminal/
- **Alacritty:** Alacritty remains a cross-platform terminal baseline and explicitly recommends measuring performance with the real workload. M6 therefore keeps it as the paired control instead of relying only on synthetic throughput. Source: https://github.com/alacritty/alacritty
- **Windows Terminal:** Its user-facing renderer can switch full repaint and software-rendering behavior independently of shell hosting, reinforcing the separation between terminal state, renderer authority and PTY process ownership. Source: https://learn.microsoft.com/en-us/windows/terminal/customize-settings/rendering
- **Market gap:** Hera already has a renderer-neutral snapshot and Paneflow already has a mature GPUI paint path, but no milestone has made the Hera snapshot authoritative in a real visible pane with rollback.

### Best Practices Applied
- Cargo features should be additive, optional dependencies should remain disabled by default, and runtime configuration is preferred when coexisting implementations must be selected without mutually exclusive features. Source: https://doc.rust-lang.org/cargo/reference/features.html
- Canary evaluation should be partial, time-limited, metric-driven and paired with explicit rollback targets. A control path must remain available while the canary receives real workloads. Source: https://sre.google/workbook/canarying-releases/
- Configuration changes that can remove operator control need automatic rollback or at least an immediate stop path. Source: https://sre.google/workbook/configuration-design/
- The local M5 policy requires zero failed or unimplemented P0 compatibility rows, zero P0 Paneflow shadow mismatches and explicit platform evidence before a host replacement experiment can expand.

*Full local evidence is recorded in `docs/m5-compatibility-release-hardening-report.md`, `evidence/m5/m5-go-no-go-thresholds.json` and `evidence/m5/dogfood/live-gpui-summary-2026-07-10.json`.*

## Assumptions & Constraints

### Assumptions (to validate)
- The existing Hera snapshot adapter can be promoted from diagnostic output to the normal `TerminalElement` input without creating a second GPUI renderer.
- The existing PTY tee can keep Hera and Alacritty ordered closely enough that Alacritty is a valid immediate fallback after Hera has been visible for an arbitrary period.
- Paneflow interactions that currently read Alacritty coordinates can be routed through an engine-neutral pane snapshot for the M6 workload set.
- A single Hera session can be shared by ingestion, comparison and rendering without duplicate parsing or unbounded lock contention.
- Linux and macOS runners can compile and execute the feature-gated non-visual and GPUI test set before M6 closes.

### Hard Constraints
- All Paneflow implementation work must occur in `C:\dev\paneflow-hera-m6` on `feat/hera-m6-host-replacement`. The checkout at `C:\dev\paneflow` is user-owned and must not be modified by M6 stories.
- The default Paneflow feature set remains empty. A default build must not read `PANEFLOW_HERA_TERMINAL_ROOT` or require local Hera source.
- The `hera-host` feature is additive and may coexist with existing `hera-dogfood` diagnostics. It must not disable normal Paneflow behavior at compile time.
- `PANEFLOW_TERMINAL_ENGINE=hera` applies only to panes created after the selector is read. Existing panes are never hot-switched between state models.
- Each pane owns exactly one PTY child and, in Hera mode, exactly one Hera `Terminal`. Alacritty may remain synchronized as control, but output must never be written twice to the child process.
- Raw terminal bytes, prompts, commands, local paths, usernames, hostnames and tokens must not enter checked-in M6 evidence.
- Windows is the first visible canary platform. Linux and macOS require measured feature-gate evidence before any claim of cross-platform host replacement.
- Public package publication, default engine rollout and deletion of Alacritty are prohibited in this milestone.

## Quality Gates

These commands must pass for every user story that touches the corresponding repository:
- `cargo fmt --all -- --check` - formatting in `C:\dev\hera-terminal` and `C:\dev\paneflow-hera-m6`
- `cargo check --workspace` - default Hera and default Paneflow workspaces compile without the host feature
- `cargo clippy --workspace --all-targets -- -D warnings` - default Hera and Paneflow targets have no Clippy warnings
- `cargo test --workspace` - default Hera and Paneflow tests pass
- `cargo doc --workspace --no-deps` - Hera public documentation builds when a Hera public API changes
- `$env:PANEFLOW_HERA_TERMINAL_ROOT='C:\dev\hera-terminal'; cargo check --workspace --features hera-host` - feature-gated Paneflow workspace compiles against the local Hera source
- `$env:PANEFLOW_HERA_TERMINAL_ROOT='C:\dev\hera-terminal'; cargo clippy --workspace --all-targets --features hera-host -- -D warnings` - feature-gated Paneflow targets have no Clippy warnings
- `$env:PANEFLOW_HERA_TERMINAL_ROOT='C:\dev\hera-terminal'; cargo test --workspace --features hera-host` - feature-gated unit, golden and GPUI tests pass
- `git diff --check` - no whitespace errors in either repository

EP-007 adds these focused gates before another 60-minute canary is allowed:
- `$env:PANEFLOW_HERA_TERMINAL_ROOT='C:\dev\hera-terminal'; cargo test -p paneflow-app --features hera-host terminal::hera_dogfood::comparison` - sequence and resize-coherent comparison tests pass
- `$env:PANEFLOW_HERA_TERMINAL_ROOT='C:\dev\hera-terminal'; cargo test -p paneflow-app --features hera-host terminal::hera_dogfood::authoritative` - bounded ingestion, burst, shutdown and snapshot publication tests pass
- `$env:PANEFLOW_HERA_TERMINAL_ROOT='C:\dev\hera-terminal'; cargo test -p paneflow-app --features hera-host terminal::pty_session` - PTY order, lifecycle and fallback tests pass
- `cargo run -p terminal-cli -- validate-m6-host-evidence evidence/m6/host-manifest.json` - public host evidence remains run-bound, digest-bound and scrubbed
- `cargo run -p terminal-cli -- validate-m6-exit-evidence evidence/m6/m6-exit-evidence.json` - no hardening or canary result can be relabeled manually

For visible UI stories, launch from `C:\dev\paneflow-hera-m6` with `PANEFLOW_HERA_TERMINAL_ROOT=C:\dev\hera-terminal`, `PANEFLOW_TERMINAL_ENGINE=hera` and `--features hera-host`; record a scrubbed structured result under `C:\dev\hera-terminal\evidence\m6`.

## Epics & User Stories

### EP-001: Experiment Contract And Activation

Establish the isolated repository boundary, additive build gate, runtime selector and immutable per-pane ownership model before making Hera visible.

**Definition of Done:** The worktree baseline is recorded, default Paneflow remains Hera-independent, `hera-host` compiles only when requested, and every new pane has an explicit immutable engine identity with a documented fallback state.

#### US-001: Freeze The M6 Baseline And Decision Policy
**Description:** As a maintainer, I want a machine-readable M6 baseline so that implementation and review use the same M5 evidence, worktree commit and go/no-go thresholds.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** None

**Acceptance Criteria:**
- [ ] Given the M6 baseline generator or checked-in artifact, when it runs from `C:\dev\hera-terminal`, then it records M5 status `DONE`, 18 completed stories, the M5 zero-mismatch live summary, Paneflow base commit `4129f8ac`, worktree label `paneflow-hera-m6` and branch `feat/hera-m6-host-replacement`.
- [ ] Given the M5 report still blocks global replacement, when M6 thresholds are written, then they distinguish `windows_visible_canary`, `cross_platform_canary` and `default_replacement` outcomes instead of treating them as one decision.
- [ ] Given a missing M5 artifact, wrong worktree branch or changed base commit, when the baseline is validated, then validation fails with the exact missing or mismatched field and does not infer readiness.
- [ ] Given the baseline is public, when it is scanned, then it contains zero absolute host paths and zero terminal transcript content.

#### US-002: Add The Additive Hera Host Gate And Runtime Selector
**Description:** As a maintainer, I want Hera host code disabled by default and selected explicitly so that normal Paneflow builds and users are unaffected.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**
- [ ] Given `src-app/Cargo.toml`, when `hera-host` is enabled, then it activates only the optional Hera proxy dependencies and any required host modules without changing the default feature set.
- [ ] Given no Hera feature and no `PANEFLOW_HERA_TERMINAL_ROOT`, when `cargo check --workspace` and `cargo test --workspace` run, then both pass without reading sibling Hera source.
- [ ] Given `PANEFLOW_TERMINAL_ENGINE=hera` and a build without `hera-host`, when Paneflow starts, then it logs one bounded warning and uses Alacritty.
- [ ] Given an absent selector, `alacritty`, or an unknown value, when a pane is created, then its engine is Alacritty; an unknown value never panics or silently selects Hera.
- [ ] Given `hera-host` and a valid Hera root, when `PANEFLOW_TERMINAL_ENGINE=hera` is set, then newly created panes are eligible for Hera authority.

#### US-003: Define Immutable Per-Pane Engine Ownership
**Description:** As a maintainer, I want each pane to own an explicit engine state so that rendering, comparison and fallback cannot disagree about which source is authoritative.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**
- [ ] Given a new terminal pane, when it is constructed, then it stores an engine identity equivalent to `Alacritty` or `HeraAuthoritative` and exposes that identity to render and diagnostics code.
- [ ] Given a pane already exists, when the process environment changes, then the pane keeps its original engine identity until it is closed.
- [ ] Given a Hera-authoritative pane, when its state graph is inspected in tests, then it owns one PTY child, one Hera terminal and one synchronized Alacritty control state.
- [ ] Given an impossible or partially initialized engine state, when construction completes, then the pane resolves to Alacritty with a structured fallback reason instead of retaining mixed authority.
- [ ] Given engine-specific behavior, when it is added, then the boundary remains inside Paneflow terminal modules and no Paneflow, GPUI or Alacritty type is added to Hera public APIs.

---

### EP-002: Hera Authoritative Render Path

Promote the existing Hera shadow state and snapshot adapter into the source painted by Paneflow's existing GPUI terminal element.

**Definition of Done:** An opted-in pane drains PTY bytes into one Hera terminal, converts its latest render snapshot into engine-neutral Paneflow layout content and paints that content without reading the Alacritty grid for visible cells.

#### US-004: Promote One Hera Session To The Host Render Source
**Description:** As a maintainer, I want the existing Hera session to provide render snapshots so that shadow ingestion, comparison and visible rendering do not create duplicate terminal cores.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-003

**Acceptance Criteria:**
- [ ] Given a Hera-authoritative pane, when PTY output is drained, then the same `terminal_core::Terminal` instance used for differential evidence produces the render snapshot.
- [ ] Given output batches arrive before a frame, when a snapshot is requested, then all successfully queued prior batches are applied in FIFO order before the snapshot is returned.
- [ ] Given two consecutive frames with no new output, when snapshots are requested, then no second Hera terminal is constructed and snapshot damage remains deterministic.
- [ ] Given the Hera state lock is poisoned, unavailable or disabled, when render requests a snapshot, then the pane records a bounded failure and enters the fallback path without panicking.
- [ ] Given a snapshot is returned, when its dimensions are checked, then they match the pane's last accepted terminal dimensions or the snapshot is rejected before paint.

#### US-005: Feed Hera Content Into The Existing Terminal Element
**Description:** As a Paneflow user, I want Hera cells painted through the mature GPUI terminal renderer so that fonts, themes and pixel geometry remain consistent with normal Paneflow.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-004

**Acceptance Criteria:**
- [ ] Given a valid Hera `RenderSnapshot`, when the host adapter runs, then it produces the engine-neutral content required by `TerminalElement` for visible rows, styles, cursor, active screen and scrollback offset.
- [ ] Given a Hera-authoritative pane, when `TerminalView::render` builds its terminal element, then visible cell layout is sourced from Hera content and not from the Alacritty grid.
- [ ] Given normal Paneflow themes, integrated glyphs and color emoji settings, when Hera content is painted, then it uses the existing font, geometry, background, text, cursor and overlay paint stages.
- [ ] Given invalid row width, unsupported style metadata or a conversion diagnostic, when adaptation runs, then it returns a typed error or diagnostic and never indexes outside the snapshot.
- [ ] Given this story changes the adapter boundary, when Hera workspace tests run, then `terminal-core` and `terminal-render-model` remain free of GPUI, Paneflow and Alacritty imports.

#### US-006: Make Hera Visible For Selected Panes
**Description:** As an experimental user, I want an opted-in pane to visibly render from Hera so that M6 proves a real terminal window rather than another diagnostic surface.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-005

**Acceptance Criteria:**
- [ ] Given `hera-host` and `PANEFLOW_TERMINAL_ENGINE=hera`, when Paneflow opens a new terminal pane, then typed shell output is visibly painted from the Hera snapshot in the normal pane body.
- [ ] Given the pane is Hera-authoritative, when it renders normally, then no side-by-side diagnostic panel or duplicate terminal surface is displayed.
- [ ] Given plain text, ANSI indexed color, truecolor, bold, italic, underline, inverse, cursor-hidden, wide-cell, image-placeholder and scrollback golden cases, when rendered through the host path, then every checked-in golden matches.
- [ ] Given alternate-screen enter and exit sequences, when the pane renders each state, then the visible screen identity and restored primary content match the Hera fixture result.
- [ ] Given `PANEFLOW_TERMINAL_ENGINE=alacritty`, when the same golden harness runs, then existing Alacritty goldens remain byte-for-byte unchanged.
- [ ] Given Hera adaptation fails before the first successful frame, when the pane paints, then it shows the synchronized Alacritty surface and a structured fallback reason rather than a blank pane.

---

### EP-003: PTY, Input, Resize And Lifecycle

Keep one real process and one user input stream while Hera becomes visually authoritative, with deterministic ordering and safe lifecycle behavior.

**Definition of Done:** PTY output, resize, focus, keyboard, paste, spawn, exit and close behavior work in a Hera-authoritative pane without duplicate child input, dropped output, orphan processes or state divergence hidden from evidence.

#### US-007: Preserve Ordered PTY Output And Bounded Backpressure
**Description:** As an experimental user, I want every PTY output byte applied in order so that the visible Hera terminal and fallback control remain coherent under rapid output.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-004

**Acceptance Criteria:**
- [ ] Given one PTY read batch, when it enters the host bridge, then identical bytes are delivered once to Alacritty and once to the single Hera session in the same observed order.
- [ ] Given output faster than frame paint, when the existing 64-batch Hera queue fills, then dropped byte and batch counters are recorded and the pane falls back instead of presenting an incomplete Hera screen as valid.
- [ ] Given split UTF-8, CSI or OSC sequences across batches, when the full stream is applied, then the authoritative result matches concatenated ingestion fixtures.
- [ ] Given a 100,000-line rapid-output workload, when it completes, then no child output is written back to the PTY and no second PTY reader is spawned.
- [ ] Given queue sender or drain failure, when the next checkpoint runs, then the failure is observable, bounded and cannot loop indefinitely.

#### US-008: Synchronize Resize, Focus, Keyboard And Paste
**Description:** As an experimental user, I want normal interaction events to reach the running process exactly once while both terminal states stay dimensionally coherent.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-003, US-004

**Acceptance Criteria:**
- [ ] Given a valid pane resize, when GPUI resolves new dimensions, then the PTY, Hera state and Alacritty control receive the same rows and columns before the next authoritative comparison checkpoint.
- [ ] Given an invalid zero or overflow dimension, when resize is requested, then the last valid dimensions remain active and Hera falls back or reports the exact validation error without panicking.
- [ ] Given keyboard input, IME commit or paste, when it is sent, then the child process receives one byte stream and both engines observe resulting PTY output without duplicating input.
- [ ] Given a pending PTY spawn, when user input arrives, then buffering remains capped at the existing 64 KiB limit; overflow is rejected or reported without unbounded allocation.
- [ ] Given DEC focus reporting is active, when pane focus changes, then focus in/out sequences are sent once and the visible Hera cursor/focus state remains coherent.

#### US-009: Preserve Spawn, Exit, Close And Host Metadata
**Description:** As a Paneflow user, I want Hera-authoritative panes to follow normal terminal lifecycle behavior so that failed spawns, exits and closed panes do not leak processes or lose host metadata.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-007, US-008

**Acceptance Criteria:**
- [ ] Given asynchronous PTY creation, when the child is pending, succeeds or fails, then the existing placeholder, promotion and in-pane error behavior remains visible through the selected engine path.
- [ ] Given normal exit, signal exit or explicit pane close, when lifecycle completes, then the child is reaped or killed once, the exit overlay remains accurate and no Hera drain task survives the pane.
- [ ] Given OSC title, CWD or clipboard metadata handled by Paneflow's host listener, when output arrives in Hera mode, then the same host events occur once and are not inferred from public evidence.
- [ ] Given the pane falls back after the process has started, when output continues, then the same child session remains interactive and its exit state is preserved.
- [ ] Given close races with output or resize, when tests repeat the race 100 times, then no panic, deadlock, orphan process or unbounded task remains.

---

### EP-004: Essential Terminal Interactions

Preserve the user interactions that make the visible terminal usable rather than limiting M6 to static output.

**Definition of Done:** Scrollback, selection, copy, search, hyperlinks, IME and cursor/theme behavior operate against engine-neutral visible state for the M6 workload set or trigger an explicit safe fallback.

#### US-010: Support Hera Scrollback And Viewport Navigation
**Description:** As an experimental user, I want to navigate bounded Hera scrollback so that long agent and build sessions remain inspectable.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-005, US-007

**Acceptance Criteria:**
- [ ] Given primary-screen output exceeding the viewport, when the user scrolls by wheel, page or scrollbar, then the visible Hera rows move predictably and `scroll_to_bottom` returns to live output.
- [ ] Given the configured Hera line and byte budgets, when a 100,000-line workload runs, then retained scrollback stays within the configured limits and evicted rows cannot be selected by stale coordinates.
- [ ] Given alternate screen is active, when the user scrolls, then primary scrollback is not exposed inside the alternate screen and is restored after exit.
- [ ] Given a resize reflows wrapped rows while scrolled back, when the next frame paints, then viewport position is clamped to valid stable row handles without panic or negative offsets.
- [ ] Given the host cannot map Hera scrollback to a valid viewport, when navigation is requested, then the pane falls back or disables the action with a diagnostic instead of showing unrelated rows.

#### US-011: Preserve Selection, Copy And Search
**Description:** As an experimental user, I want selection, clipboard copy and terminal search to operate on what Hera visibly paints so that inspection workflows remain trustworthy.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-010

**Acceptance Criteria:**
- [ ] Given a linear or rectangular mouse selection, when the user drags over Hera-painted cells, then highlights follow engine-neutral row and column coordinates and copied text matches the visible cells.
- [ ] Given a selection spanning reflowed or evicted rows, when copy is requested, then stale handles are rejected or clamped and no unrelated terminal text is copied.
- [ ] Given plain-text or regex search, when results exist in retained Hera rows, then match count, active match, next/previous navigation and scrollbar ticks refer to the same visible content.
- [ ] Given an invalid regex or an empty query, when search updates, then the existing actionable error or empty state appears without mutating terminal state.
- [ ] Given the M6 adapter cannot provide truthful data for one interaction, when that interaction begins, then the pane records the capability gap and falls back rather than reading hidden Alacritty text while presenting Hera as authoritative.

#### US-012: Preserve Hyperlinks, IME And Visual Terminal State
**Description:** As an experimental user, I want links, composition input, cursor behavior and terminal styling to match normal Paneflow so that Hera mode supports real daily interaction.

**Priority:** P1
**Size:** L (5 pts)
**Dependencies:** Blocked by US-005, US-008

**Acceptance Criteria:**
- [ ] Given OSC 8 or detected URL spans represented in the visible content, when the user performs modifier hover and click, then underline and open behavior target the visible span once.
- [ ] Given a malformed, stale or non-http link target, when it is activated, then Paneflow refuses the open and does not execute terminal content as a command.
- [ ] Given IME preedit and commit, when composing text in a Hera pane, then preedit is painted at the visible cursor and committed bytes reach the PTY once.
- [ ] Given cursor visibility, shape, blink, color override, theme transparency, integrated glyph or color emoji settings, when frames update, then the existing Paneflow paint policy applies to Hera content.
- [ ] Given unsupported Hera metadata for one visual state, when adaptation occurs, then the state has a documented conservative default or typed diagnostic and never reads uninitialized data.

---

### EP-005: Observability, Fallback And Non-Regression

Make every engine decision and failure measurable while proving that the default Alacritty path remains unchanged.

**Definition of Done:** Structured scrubbed evidence identifies engine authority and fallback, all injected failures recover locally, and automated tests prove default, opted-in Alacritty and Hera-authoritative modes without leaking private terminal content.

#### US-013: Add The M6 Evidence Contract And Runtime Metrics
**Description:** As a maintainer, I want scrubbed structured host metrics so that canary decisions are reproducible without retaining private terminal sessions.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-001, US-003

**Acceptance Criteria:**
- [ ] Given a pane lifecycle, when metrics are emitted, then they include schema version, run ID, pane pseudonym, selected engine, authoritative engine, fallback count, fallback reason class, output batches, dropped bytes, resize count, duration and exit class.
- [ ] Given latency probing, when at least 100 output batches are measured, then P50, P95 and P99 ingestion-plus-adaptation latency are recorded in milliseconds.
- [ ] Given memory measurement, when paired Alacritty-control and Hera-authoritative runs complete, then process RSS source, baseline, peak and delta are recorded with the same workload definition.
- [ ] Given a public M6 artifact, when the evidence validator scans it, then raw bytes, terminal lines, commands, CWDs, usernames, hostnames, tokens and absolute private artifact paths cause validation failure.
- [ ] Given an unknown schema, duplicate artifact path or missing source command, when the manifest is validated, then validation fails with a field-specific message.

#### US-014: Implement Automatic Per-Pane Fallback
**Description:** As an experimental user, I want a failing Hera pane to return to its synchronized Alacritty control so that the running terminal session remains usable.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-004, US-007, US-013

**Acceptance Criteria:**
- [ ] Given injected Hera initialization, output-drain, resize and snapshot-adaptation failures, when each failure is detected, then the affected pane selects Alacritty authority within 100 ms and remains attached to the same PTY child.
- [ ] Given bytes already consumed before fallback, when Alacritty becomes visible, then its synchronized screen contains the same observed PTY stream and accepts further input without replaying user input.
- [ ] Given fallback occurs once, when subsequent frames render, then the pane does not oscillate back to Hera during that session and emits at most one user-visible warning plus one structured fallback event.
- [ ] Given fallback cannot recover a valid Alacritty control, when the pane renders, then it shows a bounded in-pane terminal error and closes or kills the child through the existing lifecycle path.
- [ ] Given fault-injection tests run repeatedly, when 100 cases complete, then zero panic, deadlock, orphan child or raw terminal evidence leak occurs.

#### US-015: Prove Default And Dual-Path Non-Regression
**Description:** As a maintainer, I want automated regression coverage for every engine mode so that M6 cannot silently alter normal Paneflow behavior.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-006, US-009, US-014

**Acceptance Criteria:**
- [ ] Given the default feature set, when all Paneflow terminal tests and goldens run without a Hera root, then results match the pre-M6 baseline.
- [ ] Given `hera-host` with `PANEFLOW_TERMINAL_ENGINE=alacritty`, when the same test corpus runs, then the visible path and goldens match the default build.
- [ ] Given `hera-host` with `PANEFLOW_TERMINAL_ENGINE=hera`, when the M5 compatibility, M5 replay derivative and Paneflow host golden corpus run, then all P0 cases pass and no case is silently skipped.
- [ ] Given unknown selector values, missing Hera roots, unsupported platforms or injected adapter failures, when tests run, then expected fallback states are asserted and no test passes only because output was blank.
- [ ] Given feature combinations are inspected with Cargo, when `cargo tree -e features` runs for default and `hera-host`, then optional Hera dependencies appear only in the feature-gated graph.

---

### EP-006: Visible Canary And Exit Decision

Run the real M6 experiment on Windows, collect Linux and macOS evidence and publish a final decision without widening claims beyond measured behavior.

**Definition of Done:** A 60-minute visible Windows canary and cross-platform feature-gate runs produce validated scrubbed evidence, then the M6 report selects exactly one next outcome with blockers and owners.

#### US-016: Run The Visible Windows Hera Canary
**Description:** As a maintainer, I want a time-bounded visible Hera canary in Paneflow so that the first real window is proven under representative interactive workloads.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-012, US-015

**Acceptance Criteria:**
- [ ] Given `C:\dev\paneflow-hera-m6`, `hera-host`, a valid Hera root and `PANEFLOW_TERMINAL_ENGINE=hera`, when Paneflow starts, then a normal GPUI window opens and every newly created canary pane reports Hera as authoritative.
- [ ] Given a 60-minute run with at least two concurrent panes, when PowerShell echo/Unicode, rapid output, 100,000-line output, alternate-screen TUI, Codex CLI and Claude Code workload classes are exercised, then the run completes without crash, blank frame, lost input or orphan process.
- [ ] Given canary metrics, when the run ends, then P0 mismatch count is 0, fallback count is 0, dropped Hera output bytes are 0 and ingestion-plus-adaptation P95 is at or below 2.0 ms.
- [ ] Given a paired Alacritty-control run with the same workload definition, when peak RSS is compared, then Hera-authoritative process RSS is no more than 20% above the M5 shadow or paired control baseline.
- [ ] Given any P0 mismatch, fallback, queue drop, blank frame or incomplete 60-minute duration, when the report is generated, then the canary is `failed` or `blocked` and cannot be relabeled as pass manually.
- [ ] Given public evidence generation, when artifacts are saved under `evidence/m6`, then they are scrubbed and contain no screenshots or text that expose private terminal content.

#### US-017: Measure Linux And macOS Feature-Gate Behavior
**Description:** As a maintainer, I want the M6 host feature exercised on Linux and macOS so that Windows success is not presented as cross-platform readiness.

**Priority:** P1
**Size:** L (5 pts)
**Dependencies:** Blocked by US-015

**Acceptance Criteria:**
- [ ] Given Linux and macOS runners with the Hera repository available, when default Paneflow checks run, then they pass without the host feature or Hera root environment variable.
- [ ] Given the same runners and local or CI Hera source, when feature-gated check, Clippy and test commands run, then each platform records command, target triple, toolchain, exit code and duration.
- [ ] Given GPUI visual automation is unavailable on one runner, when evidence is produced, then headless host goldens and lifecycle tests still run and the missing visible result is recorded as `blocked`, never inferred as pass.
- [ ] Given a platform feature-gate command fails, when the M6 report is generated, then cross-platform canary and default replacement outcomes remain blocked with the exact failing command and owner.
- [ ] Given all rows pass, when the platform artifact is validated, then Windows, Linux and macOS each have measured rather than inferred status.

#### US-018: Publish The M6 Go/No-Go Report
**Description:** As a maintainer, I want one final M6 report and evidence manifest so that the next milestone is selected from verified host behavior.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-016, US-017

**Acceptance Criteria:**
- [ ] Given all M6 artifacts, when `docs/m6-paneflow-controlled-host-replacement-report.md` is written, then it reports scope, worktree commit, default-path status, Windows canary, platform rows, interaction coverage, performance, memory, fallbacks and unresolved gaps.
- [ ] Given the report decision, when it is finalized, then it selects exactly one outcome: `broader_hera_canary`, `targeted_host_hardening` or `replacement_experiment_blocked`.
- [ ] Given any P0 mismatch, fallback, dropped output, failed required platform command, privacy validation failure or missing artifact, when the decision is computed, then `broader_hera_canary` is prohibited.
- [ ] Given a successful report, when README and `docs/research-map.md` milestone text are updated, then both describe M6 as a controlled render-authority experiment and do not claim default or full runtime replacement.
- [ ] Given the evidence manifest is validated, when it references an absolute path, stale source, raw local artifact or missing file, then validation fails.
- [ ] Given all stories are `DONE` and all required gates pass, when the status tracker rolls up, then M6 becomes `DONE`; otherwise the exact story remains `BLOCKED` or `IN_REVIEW`.

---

### EP-007: Targeted Host Hardening And Canary Recovery

Remove the two demonstrated blockers without weakening bounded-memory, fallback, privacy or default-path guarantees. The epic treats comparison coherence and lossless ingestion as separate correctness problems, proves each with deterministic fixtures, then repeats the exact Windows authority gate.

**Definition of Done:** Every historical mismatch is assigned a supported class or explicitly marked unrecoverable from the scrubbed evidence, comparable checkpoints carry a shared PTY sequence and resize generation, the Hera ingest path remains byte-bounded without dropping accepted PTY output, a 10-minute qualification run passes, and the full 60-minute Windows canary plus paired Alacritty memory control passes. Linux/macOS qualification remains owned by US-017.

#### US-019: Classify And Reproduce The M6 Mismatches
**Description:** As a maintainer, I want field-level mismatch evidence and deterministic reproducers so that hardening fixes the actual divergence instead of suppressing counters.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-018

**Acceptance Criteria:**
- [ ] Given the failed canary artifacts and any required local-only diagnostic rerun of at most 10 minutes, when P0 mismatches are analyzed, then each of the 219 observed mismatches is classified as `stale_checkpoint`, `terminal_semantic_divergence`, `adapter_divergence`, `resize_generation_divergence` or `unclassified_from_public_evidence`, with those class counts summing exactly to 219.
- [ ] Given the separate unsupported baseline, when classification completes, then the one observed unsupported checkpoint retains its own field path or `unclassified_from_public_evidence` result and is never folded into, removed from or counted among the 219 P0 mismatches.
- [ ] Given the existing scrubbed artifacts cannot recover a field path, when the classification is published, then that count remains `unclassified_from_public_evidence` and is never inferred from aggregate counters.
- [ ] Given comparison instrumentation, when a checkpoint is recorded locally, then it includes only pane pseudonym, field path, outcome class, PTY batch sequence, resize generation, active-screen class and bounded timing metadata; raw cells, lines, bytes, commands, prompts, paths and identities remain forbidden from public evidence.
- [ ] Given the highest-frequency actionable mismatch class, when its deterministic fixture runs repeatedly, then it reproduces before the fix with a stable first differing field and does not depend on a live shell or GPUI timing accident.
- [ ] Given the classification report, when a proposed fix merely reclassifies a real semantic mismatch as unsupported or removes a compared field, then review rejects it.

#### US-020: Make Comparison Checkpoints Sequence-Coherent
**Description:** As a maintainer, I want Hera and Alacritty compared only at a shared applied-output boundary so that scheduler lag cannot become a false P0 mismatch or false pass.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-019

**Acceptance Criteria:**
- [ ] Given each PTY read batch, when it enters the dual-engine path, then it receives one monotonic `PtyBatchSequence` before fan-out and both engines expose the last fully applied sequence without duplicating or reordering the bytes.
- [ ] Given resize events, when they enter the host path, then they carry a monotonic resize generation ordered with output ingestion, and a checkpoint records the generation applied by both engines.
- [ ] Given unequal applied sequences or resize generations, when comparison is requested, then the outcome is `pending_coherence`, increments a dedicated counter and cannot increment equal, mismatch or unsupported counters.
- [ ] Given equal sequence and resize generations, when comparison runs, then a real differing dimensions, active-screen, viewport, style, scrollback or cursor field remains a P0 mismatch with the first field path preserved.
- [ ] Given delayed Hera application, delayed Alacritty application, split escape sequences, alternate-screen transitions and resize/output races, when deterministic tests run, then comparison waits for coherence and produces exactly one final stable outcome per requested checkpoint.
- [ ] Given pane exit or fallback, when metrics finalize, then all accepted sequences are either applied by both controls or the evidence records an incomplete terminal sequence and prohibits a passing canary.

#### US-021: Move Hera Ingestion To A Bounded Lossless Worker
**Description:** As a maintainer, I want Hera ingestion independent from GPUI frame cadence so that normal PTY bursts cannot drop bytes while memory remains bounded.

**Priority:** P0
**Size:** XL (8 pts)
**Dependencies:** Blocked by US-019, US-020

**Acceptance Criteria:**
- [ ] Given a Hera-authoritative pane, when its PTY starts, then exactly one pane-owned ingest worker owns mutation of the Hera terminal and consumes ordered output and resize messages independently from `sync_channels`, prepaint and paint cadence.
- [ ] Given any successful non-empty PTY read, when the dual-engine path receives it, then the entire read is accepted exactly once for Hera ingestion before it is exposed to either comparison or evidence roll-up.
- [ ] Given the ingest queue, when messages are enqueued, then accounting is byte-based with a documented hard memory budget of 1 MiB per pane rather than a count of 64 arbitrarily sized batches.
- [ ] Given the worker temporarily falls behind, when queued bytes reach the budget, then the producer applies cancellation-aware backpressure until capacity is available; accepted PTY bytes are never dropped, overwritten or silently skipped.
- [ ] Given a 100,000-line fragmented stream, when the real PTY batch shape is replayed while rendering is intentionally delayed, then the final marker is present, applied sequences are contiguous, dropped bytes and batches remain zero, no fallback occurs and observed queued bytes never exceed the declared budget.
- [ ] Given snapshot consumers, when they request content, then they receive the latest immutable snapshot plus applied sequence and resize generation without mutating or draining the Hera core on the GPUI thread.
- [ ] Given worker shutdown, pane close, PTY exit, fallback or a blocked producer, when lifecycle cleanup runs, then waiters are released, accepted messages drain or receive an explicit incomplete status, the worker joins, and no child, thread or queue remains orphaned.
- [ ] Given worker panic, poisoned state, impossible sequence gap or snapshot publication failure, when the host detects it, then existing one-way Alacritty fallback remains within 100 ms and the exact failure class is recorded.
- [ ] Given the default build or an Alacritty-selected pane, when Paneflow runs, then no Hera worker, queue or snapshot publisher is created and the dependency graph remains unchanged.

#### US-022: Prove The Hardening With Deterministic And Short-Run Gates
**Description:** As a maintainer, I want a cheap qualification stage so that a broken build cannot consume another 60-minute canary.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-020, US-021

**Acceptance Criteria:**
- [ ] Given the focused EP-007 comparison, authoritative-ingest and PTY lifecycle suites, when their manifest is validated and they run 25 consecutive times, then every declared test executes on every iteration; zero selected tests or any skipped declared test fails the gate, and the runs produce zero flaky failures, zero sequence gaps, zero dropped bytes and zero leaked worker threads.
- [ ] Given burst fixtures of one large write, realistic fragmented PTY reads and adversarial one-byte chunks, when each emits at least 100,000 lines, then all variants converge to the same final Hera snapshot and sequence.
- [ ] Given a visible 10-minute Windows qualification run with two panes, when Unicode/styled output, repeated resize, alternate-screen enter/exit, one 100,000-line burst and ordinary interactive input execute, then comparable mismatches, unsupported checkpoints, fallbacks, dropped bytes, blank frames, lost inputs and orphan processes are all zero.
- [ ] Given pending-coherence samples during the short run, when the run ends, then the pending count has returned to zero and every requested comparison has a terminal outcome.
- [ ] Given performance metrics, when at least 1,000 ingest batches and 100 snapshot publications are measured, then ingestion-plus-publication P95 is at or below 2.0 ms, queue high-water bytes remain within 1 MiB and the GPUI thread performs no terminal parsing.
- [ ] Given any short-run threshold fails, when status rolls up, then US-022 and EP-007 become `BLOCKED`, US-023 remains `TODO`, and no full canary is launched.

#### US-023: Repeat The Full Windows Canary And Paired Control
**Description:** As a maintainer, I want the original authority experiment repeated unchanged after hardening so that recovery is proven against the gate that failed.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-012, US-022

**Acceptance Criteria:**
- [ ] Given the exact US-016 two-pane workload contract, when a new Hera-authoritative Windows run executes for at least 60 consecutive minutes, then PowerShell Unicode/style, rapid output, 100,000 lines, alternate screen, Codex CLI and Claude Code are exercised without weakening or removing a workload because it failed previously.
- [ ] Given the final host metrics, when the run ends, then P0 mismatches, unsupported checkpoints, unresolved pending-coherence checkpoints, fallbacks, dropped bytes, crashes, blank frames, lost inputs and orphan processes are all zero.
- [ ] Given latency evidence, when at least 100 batches are measured, then ingestion-plus-snapshot-publication P95 remains at or below 2.0 ms and every pane stays within the declared queue byte budget.
- [ ] Given an Alacritty-control run with the same pane count and workload order, when both Hera and control run for at least 60 consecutive minutes and their measured durations differ by no more than 5%, then Hera-authoritative peak RSS is no more than 20% above control and the pairing metadata is present in scrubbed evidence.
- [ ] Given host and exit artifacts, when validators run, then hashes, run ID, commits, pane count and all threshold roll-ups match the referenced files and no public artifact contains terminal content or private identity.
- [ ] Given every Windows threshold passes, when status rolls up, then US-016 and US-023 become `DONE`, EP-007 becomes `DONE`, and the decision advances only to `targeted_host_hardening`; `broader_hera_canary` remains prohibited until US-017 has measured Linux and macOS.
- [ ] Given any full-run threshold fails, when status rolls up, then US-023 and EP-007 become `BLOCKED`, US-016 remains `BLOCKED`, and the report preserves the new failure counters without manual relabeling.

## Functional Requirements

- FR-01: M6 Paneflow changes must be made only in `C:\dev\paneflow-hera-m6` on `feat/hera-m6-host-replacement`.
- FR-02: Paneflow must expose an additive `hera-host` feature that is disabled by default.
- FR-03: Paneflow must select Hera only for new panes when `PANEFLOW_TERMINAL_ENGINE=hera` and `hera-host` are both active.
- FR-04: An absent, invalid or unsupported engine selector must resolve to Alacritty with at most one warning.
- FR-05: Engine identity must be immutable for the lifetime of a pane.
- FR-06: A Hera-authoritative pane must own one PTY child, one Hera terminal and one synchronized Alacritty control state.
- FR-07: Each PTY output batch must be delivered to both terminal states in the same order without writing output back to the child.
- FR-08: Hera output buffering must be bounded by a 1 MiB per-pane byte budget, apply cancellation-aware backpressure at the boundary and expose queue high-water and wait metrics without dropping accepted PTY bytes.
- FR-09: The latest valid Hera render snapshot must be the source for visible cells in a Hera-authoritative pane.
- FR-10: Hera content must be painted through the existing Paneflow `TerminalElement` and paint modules.
- FR-11: A Hera-authoritative normal pane must not render a side-by-side diagnostic surface.
- FR-12: The PTY, Hera state and Alacritty control must receive coherent resize dimensions.
- FR-13: Keyboard, focus, paste and IME commit input must reach the child process exactly once.
- FR-14: PTY input buffered during pending spawn must retain the existing 64 KiB cap.
- FR-15: Spawn, promotion, exit, signal, close and kill behavior must preserve the existing Paneflow lifecycle contract.
- FR-16: Hera-authoritative panes must support the M6 scrollback, selection, copy, search, hyperlink, IME and cursor/theme acceptance criteria.
- FR-17: Hera init, drain, resize or snapshot failure must trigger per-pane Alacritty fallback within 100 ms.
- FR-18: A pane that falls back must never automatically switch back to Hera during the same session.
- FR-19: The default Paneflow workspace must compile and test without local Hera source.
- FR-20: Feature-gated Paneflow must compile and test against the local Hera checkout on Windows, Linux and macOS runners.
- FR-21: M6 evidence must be versioned, machine-readable, reproducible and scrubbed.
- FR-22: Checked-in evidence must never contain raw terminal bytes, commands, prompts, local paths, usernames, hostnames or tokens.
- FR-23: The M6 final report must distinguish visible canary success from broader canary and default replacement authorization.
- FR-24: M6 must not publish crates, remove Alacritty, replace the Paneflow PTY transport or make Hera the default.
- FR-25: Every dual-engine PTY output batch must receive one monotonic sequence before fan-out.
- FR-26: Every comparison must bind to equal applied PTY sequence and resize generation values from Hera and Alacritty.
- FR-27: An incoherent checkpoint must be recorded as pending and must never increment pass, mismatch or unsupported counters.
- FR-28: Hera terminal mutation must occur in one pane-owned ingest worker rather than on the GPUI synchronization or paint path.
- FR-29: Snapshot publication must expose immutable content plus the applied PTY sequence and resize generation.
- FR-30: Queue capacity must be accounted in bytes, never only in batch count.
- FR-31: Backpressure, shutdown and fallback must release all blocked producers and join the ingest worker.
- FR-32: Field-level mismatch diagnostics may remain local/private, but public artifacts may contain only bounded classes, counters, pseudonyms, sequences, generations and timings.
- FR-33: A passing short qualification is mandatory before another 60-minute canary.
- FR-34: EP-007 may advance the Windows decision only to `targeted_host_hardening`; it cannot bypass US-017 platform evidence.

## Non-Functional Requirements

- **Performance:** Hera PTY ingestion plus snapshot adaptation P95 must be at or below 2.0 ms over at least 100 measured batches in the Windows canary.
- **Frame Recovery:** Detection-to-Alacritty fallback must complete within 100 ms in 100% of injected init, drain, resize and adaptation failures.
- **Memory:** Peak process RSS for the paired Hera-authoritative canary must be no more than 20% above the equivalent M5 shadow or paired Alacritty-control workload.
- **Reliability:** The Windows canary must run for 60 consecutive minutes with at least two panes, zero P0 mismatches, zero fallbacks, zero dropped Hera bytes, zero crashes and zero orphan children.
- **Backpressure:** Hera output buffering must remain at or below 1 MiB per pane, never drop accepted PTY bytes and unblock cleanly on shutdown; pending PTY input remains capped at 65,536 bytes.
- **Checkpoint Integrity:** 100% of equal and mismatch outcomes must reference identical Hera/Alacritty PTY sequences and resize generations; incoherent requests remain pending and cannot count as pass.
- **Threading:** Terminal parsing and queue draining must not execute on the GPUI paint path; worker lifecycle tests must leave zero joined-thread or blocked-producer leaks.
- **Compatibility:** 100% of P0 M5 compatibility fixtures, M5 replay derivatives and M6 host goldens selected by the manifest must execute and pass; skipped P0 tests count as failure.
- **Platform:** Windows, Linux and macOS must each have explicit measured default and `hera-host` command rows before M6 can authorize a cross-platform canary.
- **Privacy:** Public M6 artifacts must contain zero raw terminal lines, zero raw byte fields and zero unredacted local identity or path fields.
- **Default Isolation:** 100% of default Paneflow check and test commands must pass in an environment where `PANEFLOW_HERA_TERMINAL_ROOT` is absent and the Hera checkout is unavailable.
- **Interaction:** Keyboard, paste, focus, IME commit, selection, copy and link activation tests must assert exactly one resulting host or PTY action per user action.

## Edge Cases & Error States

| # | Scenario | Trigger | Expected Behavior | User Message |
|---|----------|---------|-------------------|--------------|
| 1 | Host feature absent | Selector requests Hera in a default build | Use Alacritty, warn once, keep startup successful | `Hera host engine is unavailable in this build; using Alacritty.` |
| 2 | Unknown selector | `PANEFLOW_TERMINAL_ENGINE` has an unsupported value | Use Alacritty and record the rejected value class without echoing arbitrary content | `Unknown terminal engine; using Alacritty.` |
| 3 | Missing Hera root | `hera-host` build cannot locate required Hera source | Fail the feature-gated build with the required path contract; default build remains unaffected | Build error names `PANEFLOW_HERA_TERMINAL_ROOT` |
| 4 | Hera initialization failure | Invalid dimensions or core construction error | Create the pane with Alacritty authority and a structured fallback reason | `Hera could not initialize; this pane is using Alacritty.` |
| 5 | Legacy output drop path reached | Any successful non-empty PTY read is rejected, overwritten or skipped after US-021 | Treat as a hard invariant failure, record the incomplete sequence, fail the canary and fall back once; ordinary queue pressure must use row 16 backpressure instead | `Hera ingestion failed; this pane is using Alacritty.` |
| 6 | Invalid resize | Zero, overflow or stale dimensions | Keep last valid dimensions; reject resize and fall back if coherence is lost | `Terminal resize failed; this pane is using Alacritty.` |
| 7 | Pane closes during spawn | User closes before background PTY promotion | Cancel or finish cleanup, reap child if created, leave no drain task | None |
| 8 | Mid-session adapter failure | Invalid snapshot width, style or row handle | Switch visible authority to synchronized Alacritty within 100 ms | `Hera rendering stopped; your session continues with Alacritty.` |
| 9 | Alternate-screen scroll | User scrolls while TUI alternate screen is active | Do not expose primary scrollback; restore it after alternate exit | None |
| 10 | Scrollback eviction | Selection or search points to evicted rows | Reject stale handles, clear or clamp UI state, never copy unrelated text | `Selection is no longer available after scrollback trimming.` |
| 11 | Split or invalid UTF-8 | PTY batch ends inside a code point | Preserve parser streaming semantics and deterministic replacement policy | None |
| 12 | Invalid regex | Search query does not compile | Keep terminal state untouched and show existing regex error | Existing Paneflow search error |
| 13 | Platform runner unavailable | Linux or macOS feature run cannot execute | Record `blocked` with owner and command; prohibit cross-platform claim | Report-only blocker |
| 14 | Evidence privacy violation | Validator finds raw text, token or private path | Reject artifact and final report roll-up | Validator names the violating field |
| 15 | Incoherent checkpoint | Hera and Alacritty applied sequences or resize generations differ | Record pending coherence, schedule a later comparison, do not emit pass or mismatch | None |
| 16 | Queue budget reached | Hera ingest queue reaches 1 MiB before the worker drains it | Apply cancellation-aware backpressure, preserve order and bytes, expose wait/high-water metrics | None unless the worker fails |
| 17 | Worker exits with accepted messages pending | Panic, poison, shutdown race or explicit fallback | Record incomplete final sequence, unblock producers, fall back once and prohibit canary pass | `Hera ingestion stopped; this pane is using Alacritty.` |
| 18 | Snapshot publication lags ingestion | UI requests a frame before the latest applied sequence is published | Render the latest valid snapshot, mark comparison pending, request another frame | None |
| 19 | Short qualification fails | Any mismatch, unsupported, unresolved pending, fallback, drop or lifecycle failure | Block the full canary and retain exact counters | Report-only blocker |

## Risks & Mitigations

| # | Risk | Probability | Impact | Mitigation |
|---|------|-------------|--------|------------|
| 1 | M6 is misrepresented as full engine replacement even though PTY and input modes still use legacy state | High | High | Name the boundary `HeraAuthoritative`, document retained responsibilities and prohibit default/full replacement claims in FR-23 and FR-24 |
| 2 | Hera and Alacritty diverge before a fallback | Med | High | Feed identical ordered bytes, compare checkpoints, block on dropped bytes and keep fallback tests with injected faults |
| 3 | Dual state increases memory or lock contention | Med | High | Reuse one Hera session, retain queue caps, measure P95 and paired RSS, block broader canary above thresholds |
| 4 | Existing Paneflow interactions read hidden Alacritty text while Hera is presented as authoritative | High | High | Route visible interactions through engine-neutral content; fall back when truthful Hera data is unavailable |
| 5 | Windows success hides Linux or macOS compile/runtime failures | High | High | Require explicit platform command rows and prohibit cross-platform rollout when any required row fails or is missing |
| 6 | Local source proxy behavior becomes brittle across two repositories | Med | Med | Record exact worktree and Hera commits, validate root files at build time and keep the default graph independent |
| 7 | Automatic fallback masks recurring Hera defects | Med | High | Count every fallback as a failed canary condition, emit one structured reason and never label a fallback run as pass |
| 8 | Cross-repository status and evidence drift | Med | Med | Keep the canonical PRD, tracker, evidence manifest and final report in Hera; record Paneflow branch and commit in every run |
| 9 | The M6 scope expands into release hardening or full PTY replacement | Med | High | Enforce Non-Goals and defer package, semver, security release policy, default rollout and legacy deletion to later PRDs |
| 10 | Sequence metadata hides a real semantic divergence | Med | High | Compare every existing P0 field after coherence, preserve first-difference paths and reject reduced comparison coverage |
| 11 | Blocking backpressure freezes shutdown or the GPUI thread | Med | High | Keep producers cancellation-aware, parse only on the worker and test full queues during close, exit and fallback |
| 12 | A 1 MiB queue becomes an arbitrary memory increase instead of a flow-control boundary | Med | Med | Verify byte high-water, force delayed-render burst tests and require backpressure behavior above the budget |
| 13 | Local mismatch diagnostics leak terminal content | Med | High | Keep raw diagnostics untracked and local, publish only bounded field classes and run the existing privacy validators |
| 14 | A cheap short run is mistaken for canary success | High | High | Treat US-022 only as admission to US-023; preserve the full 60-minute, paired-memory and platform gates |

## Non-Goals

Explicit boundaries for M6:

- Do not make Hera the default Paneflow terminal engine.
- Do not delete `alacritty_terminal`, the Alacritty control state or the existing default `TerminalElement` path.
- Do not replace Paneflow's PTY spawn, process lifecycle or input mode authority with `terminal-pty`.
- Do not promise full terminal engine replacement; M6 replaces visible render authority for explicitly selected panes.
- Do not add an in-app terminal engine setting, migration UI or end-user rollout control.
- Do not publish any Hera crate to crates.io or perform public pre-release packaging.
- Do not resolve the separate M5 package staging, semver baseline, cargo-deny policy, cargo-audit or OpenSSF Scorecard release work unless a host story is directly blocked by it.
- Do not build a standalone Hera desktop app, a new GPU renderer, a theme system, a multiplexer or image-protocol rendering.
- Do not check in raw Codex, Claude Code, shell or user terminal recordings.
- Do not fix the observed overflow by only increasing `OUTPUT_QUEUE_CAPACITY` or by making the queue unbounded.
- Do not suppress mismatches, remove compared fields or treat incoherent checkpoints as equal.
- Do not replace Paneflow's PTY transport or move Alacritty input-mode authority into Hera during EP-007.
- Do not mark US-016 complete from deterministic tests or the 10-minute qualification run.

## Files NOT to Modify

- `C:\dev\paneflow\**` - the original Paneflow checkout is user-owned and contains M5 work; all M6 implementation belongs in the dedicated worktree.
- `C:\dev\paneflow-codex-native-chat\**` - unrelated Paneflow worktree.
- `C:\dev\paneflow-hera-m6\src-app\src\agents\**` - agent product behavior is outside terminal engine authority.
- `C:\dev\paneflow-hera-m6\src-app\src\ipc\**` - IPC and scripting protocols are not part of M6.
- `C:\dev\paneflow-hera-m6\packaging\**` - installers and distribution are outside the experiment.
- `C:\dev\hera-terminal\evidence\m1\**` through `C:\dev\hera-terminal\evidence\m5\**` - prior milestone evidence is immutable input; M6 writes only under `evidence/m6`.
- `C:\dev\hera-terminal\docs\reference-inventory\**` - reference inventories change only if new research changes a durable architecture decision, not for implementation bookkeeping.
- Hera parser and terminal semantics outside an explicitly demonstrated M6 API or compatibility gap - host convenience must not weaken fixture-proven behavior.

## Technical Considerations

These are engineering questions with recommended answers, not permission to widen scope:

- **Backend boundary:** Should Paneflow use an enum or a trait for the two known engines? Recommended: a closed internal enum for M6, because only Alacritty and Hera exist and exhaustive matching makes fallback states visible. Revisit a trait when a third backend or test double needs independent substitution.
- **Hera session ownership:** Should authoritative rendering create a second Hera core? Recommended: no. Evolve the existing shadow session into one host-owned Hera state that serves ingestion, snapshot, comparison and metrics.
- **Render adapter:** Should M6 add a Hera-specific GPUI element? Recommended: no. Convert `RenderSnapshot` into engine-neutral layout content and reuse `TerminalElement`, its paint modules and its geometry tests.
- **Input mode authority:** Should Hera encode application cursor, keypad, mouse and focus sequences in M6? Recommended: no unless a small public API gap is proven and fixture-backed. Keep synchronized Alacritty mode state as the explicit M6 input authority and record this debt for the next replacement stage.
- **PTY ownership:** Should Paneflow adopt `terminal-pty` now? Recommended: no. M6 measures render authority while keeping one existing Paneflow PTY lifecycle. PTY replacement would multiply risk and deserves a separate PRD.
- **Interaction coordinates:** Should selection and search use raw row indexes? Recommended: no. Map through Hera stable row handles or a frame-scoped engine-neutral coordinate model, and reject stale handles after trim or reflow.
- **Fallback granularity:** Should a Hera failure restart the application? Recommended: no. Fall back once per pane because the synchronized control state and child process already exist.
- **Feature naming:** Should `hera-host` replace `hera-dogfood`? Recommended: no. `hera-host` expresses authority, while `hera-dogfood` remains diagnostic compatibility plumbing; shared dependencies may be grouped additively.
- **Evidence ownership:** Should host artifacts live in Paneflow? Recommended: raw private diagnostics stay local to Paneflow, while scrubbed milestone artifacts and final status remain canonical under Hera `evidence/m6` and `docs`.
- **Platform rollout:** Should a Windows visual pass enable Hera elsewhere? Recommended: no. Linux and macOS require their own measured feature rows and visual evidence remains explicitly blocked where unavailable.
- **Comparison boundary:** Assign a monotonic sequence at the PTY read boundary and compare only after both engines expose that sequence plus the same resize generation. Scheduler lag is pending evidence, not a mismatch or pass.
- **Ingestion ownership:** One pane-owned worker should mutate Hera and publish immutable versioned snapshots. GPUI reads snapshots; it does not drain or parse PTY output.
- **Queue policy:** Replace the 64-batch `try_send` queue with byte-accounted bounded flow control. At the 1 MiB boundary, block cancellably until the worker drains capacity instead of dropping output or allocating without bound.
- **Diagnostic policy:** Private local diagnostics may identify the first differing field and version metadata, but public artifacts retain only bounded classifications and aggregate counts.
- **Recovery staging:** Deterministic tests and a 10-minute run are admission gates only. The original 60-minute workload and paired control remain the authority gate.

## Success Metrics

| Metric | Baseline (current) | Target | Timeframe | How Measured |
|--------|-------------------|--------|-----------|-------------|
| User-visible Hera authority | 0 Paneflow panes | At least 2 concurrent panes in one real GPUI window sourced from Hera | M6 Windows canary | Engine identity plus scrubbed canary artifact |
| M5 status carried forward | 18 of 18 stories `DONE` | Exact baseline validated before implementation | US-001 | M6 baseline validator |
| P0 host mismatch count | 0 in the final 45-minute M5 shadow run, but Hera was not visible | 0 across the 60-minute Hera-authoritative canary | US-016 | M6 comparison counters |
| Automatic fallback recovery | No authoritative fallback path exists | 100% of four injected failure classes recover within 100 ms | US-014 | Fault-injection test report |
| Canary fallback count | N/A for visible Hera authority | 0 in a passing 60-minute canary | US-016 | Runtime evidence |
| Dropped Hera output bytes | 0 in the final M5 run | 0 in a passing M6 canary | US-016 | Bounded queue counters |
| PTY batch ingestion plus adaptation P95 | M3 policy target is 2.0 ms; checked-in M5 live summary does not include a measured value | At or below 2.0 ms over at least 100 batches | US-016 | Latency probe artifact |
| Process RSS overhead | No paired visible-authority baseline | At most 20% above paired M5 shadow or Alacritty-control workload | US-016 | Same-workload process RSS report |
| Default path isolation | Paneflow default build has no Hera feature | 100% of default check and test gates pass without Hera root | Every story, final US-015 | CI and local command evidence |
| Host golden coverage | Hera diagnostic golden corpus exists, no authoritative host corpus | 100% of declared P0 host goldens execute and pass | US-006, US-015 | Golden test manifest |
| Platform command coverage | Windows measured; Linux and macOS blocked in M5 | Windows, Linux and macOS each have explicit default and feature-gated rows | US-017 | M6 platform artifact |
| Public evidence privacy | M5 validator rejects raw transcript fields | 0 raw terminal, identity, token or private path violations | US-013, US-018 | M6 evidence validator |
| Classified canary mismatches | 219 aggregate P0 mismatches with no public field distribution | Every mismatch classified or explicitly marked unrecoverable from public evidence | US-019 | Local diagnostic summary plus scrubbed classification artifact |
| Coherent comparison coverage | No shared applied-sequence or resize-generation proof | 100% of equal/mismatch outcomes carry identical engine sequence and resize generation | US-020 | Comparison coherence counters |
| Hera queue loss | 1,526,241 dropped bytes and one fallback in the first canary | 0 dropped bytes with at most 1 MiB queued per pane under fragmented 100,000-line bursts | US-021, US-022 | Queue high-water and sequence fixtures |
| Short-run qualification | No admission gate between unit tests and the first 60-minute run | One 10-minute two-pane run with every hard-failure counter at zero | US-022 | Scrubbed qualification artifact |
| Recovered Windows authority | First 60-minute canary failed | One repeated 60-minute canary plus paired control passes the unchanged US-016 thresholds | US-023 | M6 host and exit evidence validators |

## Open Questions

- Can Hera's current snapshot and stable row handles support viewport-relative scroll, selection and search without exposing internal vectors? Owner: US-010 and US-011 implementers before interaction acceptance.
- Which Paneflow host behaviors still read `SharedTerm` directly after Hera becomes visually authoritative? Owner: US-003 implementer; every remaining dependency must be classified as control-only, input-mode debt or M6 blocker.
- Can the existing shadow output drain be called deterministically before GPUI prepaint without blocking the render thread above the 2.0 ms P95 budget? Owner: US-004 implementer before selecting lock and snapshot strategy.
- Which Linux and macOS runner can execute the pinned Paneflow GPUI fork and local Hera proxy crates? Owner: US-017 implementer before M6 final review.
- Does a paired Alacritty-control process or the M5 shadow run provide the less noisy memory baseline on each platform? Owner: US-013 and US-016 implementers; the chosen baseline must be recorded with rationale.
- Which field paths account for the 219 historical mismatches? Owner: US-019; do not infer this from aggregate public counters.
- Can cancellation-aware backpressure stay below the 2.0 ms P95 target under realistic fragmented PTY reads? Owner: US-021 and US-022; measure enqueue waits, worker ingestion and snapshot publication separately.
- Does Alacritty expose a reliable applied-batch boundary directly, or must Paneflow acknowledge application at its event-loop handoff? Owner: US-020; the chosen boundary must be fixture-proven and cannot rely only on enqueue completion.
- Is 1 MiB sufficient for every declared burst while preserving the paired RSS threshold? Owner: US-021; the budget is a hard cap, and any adjustment requires measured evidence plus a PRD update rather than a silent constant change.
[/PRD]
