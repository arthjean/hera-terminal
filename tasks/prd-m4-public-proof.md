[PRD]
# PRD: M4 Public Proof

## Changelog

| Version | Date | Author | Summary |
|---------|------|--------|---------|
| 1.0 | 2026-07-04 | Arthur Jean | Initial draft for Hera M4 public proof package |

## Problem Statement

1. Hera M1, M2 and M3 prove the internal spine: headless core, PTY harness and Paneflow dogfood integration. They do not yet prove Hera publicly.
2. The current evidence is useful for local development but fragmented across checked-in fixtures, dogfood reports, local audit artifacts and Paneflow logs. A future contributor or host embedder cannot evaluate compatibility, memory, replay, API shape and product integration from one reproducible package.
3. Terminal engines are trust-sensitive infrastructure. "It opens a terminal" is not enough. Hera needs public proof: compatibility matrix, deterministic replay, benchmark numbers, memory profile for long scrollback, a scrubbed Paneflow dogfood demo and clear Rust API examples.
4. Without M4, the next milestone can jump to replacement UI or publishing before the engine has earned public credibility.

**Why now:** M3 is marked DONE, Hera and Paneflow dogfood gates have passed locally, and the latest live shadow run produced no mismatch report files. The next useful step is converting local confidence into reproducible public evidence before broader integration or crate release.

## Overview

This PRD defines Hera M4: create a public proof package for Hera as a Rust terminal engine core.

M4 is not a terminal application milestone. It does not replace Paneflow's authoritative terminal path, publish crates to crates.io, or add a new renderer. It makes Hera inspectable by someone outside Arthur's local loop. The package must answer six questions:

1. What terminal behavior is covered, partially covered or known missing?
2. Can captured sessions replay deterministically?
3. What are the performance and memory characteristics at 10k, 100k and 1M logical lines?
4. Can Paneflow dogfood be demonstrated without leaking private terminal content?
5. Is the Rust API small, documented and plausible for embedders?
6. What must happen before M5 moves toward wider adoption or replacement experiments?

The main outputs are `docs/m4-public-proof-report.md`, machine-readable evidence artifacts, compatibility matrix data, benchmark and memory reports, replay demos, scrubbed Paneflow dogfood instructions, public API examples and package readiness notes.

## Goals

| Goal | M4 Target | M6 Target |
|------|-----------|-----------|
| Public evidence | One checked-in M4 report links compatibility, replay, memory, benchmark, Paneflow and API proof | Public docs become the stable entrypoint for contributors and embedders |
| Compatibility clarity | Matrix covers core VT categories, implemented status, fixture source and known gaps | Matrix is backed by broader VTTEST/esctest2-style corpus |
| Replay determinism | At least 3 scrubbed sessions replay twice with identical final snapshots and event counts | Replay suite includes long Paneflow, Codex, Claude Code and shell sessions |
| Memory proof | 10k, 100k and 1M logical-line profiles record peak RSS or equivalent process memory plus Hera-owned counters | Bounded scrollback policy is benchmarked across realistic daily sessions |
| Performance proof | Benchmarks report ingest, snapshot, replay and comparison throughput with regression thresholds | CI blocks meaningful regression in hot paths |
| Public API readiness | API examples compile and docs render without private Paneflow assumptions | Hera crates are publish-ready or published with stable semver policy |

## Target Users

### Terminal Engine Evaluator

- **Role:** Rust developer, OSS contributor or host engineer evaluating whether Hera is technically serious.
- **Behaviors:** Reads README, opens the M4 report, runs commands locally, checks API examples and known gaps.
- **Pain points:** Terminal projects often claim compatibility without reproducible tests, long-session proof or documented failure modes.
- **Current workaround:** Read source and infer maturity from scattered fixtures.
- **Success looks like:** The evaluator can run a small command set and understand exactly what Hera can and cannot claim.

### Paneflow Maintainer

- **Role:** Arthur maintaining Paneflow while proving Hera inside a real product host.
- **Behaviors:** Uses dogfood mode, captures local reports, compares Hera against the authoritative Paneflow terminal path.
- **Pain points:** Local dogfood can become anecdotal unless evidence is scrubbed, summarized and repeatable.
- **Current workaround:** Inspect `.paneflow-audit` artifacts and terminal logs manually.
- **Success looks like:** Paneflow dogfood has a public-safe demo path, a private raw artifact policy and a written M5 recommendation.

### Future Hera Embedder

- **Role:** Developer embedding Hera into a GUI, TUI, remote terminal, replay tool or agent workspace.
- **Behaviors:** Looks for byte ingestion, resize, render snapshot, replay, PTY boundary and memory policy examples.
- **Pain points:** Existing engines often expose app-specific internals or require renderer coupling too early.
- **Current workaround:** Adapt Alacritty-like internals or write a small emulator from scratch.
- **Success looks like:** API examples compile and show the intended host boundary without importing Paneflow, GPUI or platform details into `terminal-core`.

## Research Findings

Key findings that informed this PRD:

### Compatibility Evidence

- VTTEST remains a durable terminal compatibility reference because it exercises VT100, VT220 and related emulator behavior rather than one local shell session: https://invisible-island.net/vttest/.
- esctest2 provides an automated xterm-style compatibility test reference. Hera should not copy it blindly in M4, but its categories are useful for naming matrix rows and future fixture expansion: https://github.com/ThomasDickey/esctest2.
- The M4 compatibility matrix should separate "implemented", "fixture-backed", "manual-only", "not implemented" and "out of scope" so public claims stay honest.

### Replay And Recording

- asciinema asciicast v2 uses newline-delimited JSON with a header followed by event records. That supports Hera's direction: public replay demos should prefer simple event streams over private raw shell transcripts: https://docs.asciinema.org/manual/asciicast/v2/.
- Hera already has fixture and recording concepts from M1 to M3. M4 should stabilize the public subset: scrubbed, deterministic, versioned and documented.

### Benchmarks And Memory

- Criterion.rs is the right Rust benchmark baseline for repeatable local microbenchmarks and regression visibility: https://bheisler.github.io/criterion.rs/book/.
- Long-session proof must include memory, not only latency. The README target explicitly calls out 10k, 100k and 1M line scenarios, so M4 must record memory counters or process-level peak measurements for those sizes.

### Public Rust Readiness

- The Rust API Guidelines remain the best checklist for public crate design quality before wider adoption: https://rust-lang.github.io/api-guidelines/.
- docs.rs automatically builds documentation for crates published to crates.io, and its metadata can configure feature selection. Hera should make `cargo doc` clean before any publish decision: https://docs.rs/about and https://docs.rs/about/metadata.
- Cargo's publishing guidance recommends packaging and dry-run validation before publish. M4 should perform package-readiness checks but must not publish crates: https://doc.rust-lang.org/cargo/reference/publishing.html.

### OSS Trust

- OpenSSF Scorecard is a useful external frame for open-source security health. M4 should not overfit to a score, but should document baseline checks and obvious gaps before public proof: https://scorecard.dev/.

## Assumptions & Constraints

### Assumptions To Validate

- M3 dogfood artifacts can be summarized publicly without committing raw terminal content, prompts, local paths or private agent transcripts.
- Hera can generate 10k, 100k and 1M logical-line workloads deterministically without requiring a real shell for every benchmark.
- `terminal-core`, `terminal-render-model`, `terminal-protocol`, `terminal-fixtures`, `terminal-pty` and `terminal-cli` can expose enough documentation for a public API audit without semver commitments.
- Paneflow dogfood can produce a demo script or screenshot sequence that proves integration while leaving Paneflow's default terminal path authoritative.
- Windows-local measurements can be marked as local baseline while matrix rows leave Linux and macOS as "not measured" when they are not actually run.

### Hard Constraints

- M4 must not publish crates to crates.io.
- M4 must not replace Paneflow's default terminal path.
- M4 must not require raw private terminal recordings to be checked in.
- `terminal-core` must remain free of PTY, GPUI, Paneflow, renderer, windowing and platform dependencies.
- Public artifacts must distinguish measured evidence from planned coverage.
- Benchmark outputs must include machine/context metadata so numbers are not presented as universal.
- New docs and data should use ASCII text unless modifying an existing file that already intentionally uses non-ASCII.

## Non-Goals

- Building a standalone Hera desktop terminal app.
- Implementing a GPU renderer.
- Achieving full VTTEST or esctest2 pass coverage.
- Rendering Kitty, iTerm2 or Sixel image protocols.
- Publishing crates or making semver stability promises.
- Moving Paneflow from shadow dogfood to Hera-authoritative rendering.
- Sending dogfood artifacts to telemetry or external services.

## Quality Gates

These commands must pass for every Hera story unless the story is documentation-only and explicitly records why code gates are unchanged:

- `cargo fmt --all -- --check` - Hera workspace formatting is stable.
- `cargo check --workspace` - Hera workspace typechecks.
- `cargo clippy --workspace --all-targets -- -D warnings` - Hera lints are blocking.
- `cargo test --workspace` - Hera unit, fixture and replay tests pass.
- `cargo doc --workspace --no-deps` - Public docs render without warnings that block publication readiness.

Additional gates for performance and proof stories:

- `cargo bench --workspace` or the narrower checked-in M4 benchmark command - benchmark harness runs and writes M4 evidence artifacts.
- M4 memory profile command for 10k, 100k and 1M logical-line scenarios - memory evidence is generated with machine metadata.
- Replay verification command - each public replay fixture runs twice and produces identical final snapshot hashes.

Additional gates for Paneflow dogfood stories:

- `cargo check --workspace --features hera-dogfood` from `C:\dev\paneflow` - Paneflow dogfood feature typechecks.
- `cargo test --workspace --features hera-dogfood` from `C:\dev\paneflow` - Paneflow dogfood tests pass.
- Live dogfood smoke command from `C:\dev\paneflow` - no mismatch report files are generated for the documented M4 demo scenario.

Additional gates for package-readiness stories:

- `cargo package -p terminal-core --allow-dirty --no-verify`
- `cargo package -p terminal-protocol --allow-dirty --no-verify`
- `cargo package -p terminal-render-model --allow-dirty --no-verify`
- `cargo package -p terminal-fixtures --allow-dirty --no-verify`
- `cargo package -p terminal-pty --allow-dirty --no-verify`
- `cargo package -p terminal-cli --allow-dirty --no-verify`

The package commands are readiness checks only. They must not be followed by `cargo publish`.

## Epics & User Stories

### EP-001: Evidence Baseline And Artifact Contract

Convert local M3 confidence into a public-proof evidence structure with redaction, provenance and repeatable commands.

**Definition of Done:** The repo has a documented M4 evidence layout, redaction policy and baseline report structure that future stories can populate.

#### US-001: Refresh M3 Baseline For M4

**Description:** As a Hera maintainer, I want M3's final dogfood status summarized accurately so that M4 starts from current evidence instead of stale reports.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** None

**Acceptance Criteria:**

- [ ] Given `tasks/prd-m3-paneflow-dogfood-harness-status.json`, when it is read, then M3 is marked `DONE` and the M4 baseline references that status.
- [ ] Given `docs/m3-paneflow-dogfood-report.md` contains stale blocked or local-only notes, when M4 baseline is written, then it distinguishes historical blockers from current verified green gates.
- [ ] Given the latest Paneflow dogfood smoke is rerun, when no mismatch artifacts are produced, then the M4 baseline records command, date and artifact directory.
- [ ] Given the smoke run fails or writes mismatch reports, when M4 baseline is generated, then the PRD status remains implementable but the report marks Paneflow proof as blocked instead of silently passing.

#### US-002: Define Public Evidence Artifact Layout

**Description:** As a future contributor, I want M4 artifacts in a predictable layout so that public proof can be inspected without hunting through local audit folders.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**

- [ ] Given the repo root, when M4 files are listed, then public artifacts live under documented paths such as `docs/`, `fixtures/`, `benches/` or a dedicated evidence directory.
- [ ] Given an artifact is generated by a command, when its metadata is read, then it records command, Hera commit, OS, shell, UTC timestamp and relevant feature flags.
- [ ] Given a private local artifact path exists, when public evidence is generated, then raw prompts, local usernames, home paths and terminal bytes are not copied into checked-in artifacts.
- [ ] Given an artifact cannot be regenerated, when the manifest is validated, then it is marked as non-reproducible with a reason instead of being presented as proof.

#### US-003: Add Evidence Manifest And Redaction Policy

**Description:** As a maintainer, I want machine-readable evidence metadata and redaction rules so that public proof remains trustworthy and safe.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given generated M4 artifacts, when the evidence manifest is read, then every artifact has a type, path, source command, generated-at value and privacy classification.
- [ ] Given a public replay or dogfood demo artifact, when redaction validation runs, then known private tokens, local home paths and raw transcript fields are rejected.
- [ ] Given a developer attempts to commit a `raw_local` artifact, when validation runs, then it fails with the offending file path and classification.
- [ ] Given redaction rules are updated, when old artifacts are checked, then stale artifacts that no longer pass are flagged for regeneration.

### EP-002: Compatibility Matrix

Make terminal behavior claims explicit, source-backed and honest about gaps.

**Definition of Done:** Hera has a public compatibility matrix with fixture links, status fields, gap notes and external reference mapping.

#### US-004: Define Compatibility Matrix Schema

**Description:** As a terminal engine evaluator, I want a structured compatibility matrix so that I can see which terminal behaviors are implemented, tested and missing.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given the matrix file, when it is parsed, then every row has category, behavior, status, fixture coverage, source reference, notes and owner fields.
- [ ] Given a row claims `implemented`, when fixtures are inspected, then at least one fixture or replay artifact is linked.
- [ ] Given a behavior is not implemented, when the matrix is rendered in docs, then it is shown as a known gap and not hidden behind broad category language.
- [ ] Given a malformed matrix row is introduced, when validation runs, then the validation fails with row identifier and missing fields.

#### US-005: Map VTTEST And Esctest2 Categories To Hera Fixtures

**Description:** As a compatibility reviewer, I want Hera's matrix tied to known terminal test categories so that public claims are not invented locally.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-004

**Acceptance Criteria:**

- [ ] Given VTTEST categories, when the matrix is reviewed, then cursor movement, screen clearing, scrolling, character attributes, alternate screen and resize-related behavior are represented.
- [ ] Given esctest2-inspired categories, when the matrix is reviewed, then xterm-style escape handling gaps are recorded even when fixtures are deferred.
- [ ] Given a category has no Hera fixture, when docs are generated, then the row is marked `manual-only`, `not measured` or `not implemented` instead of `passing`.
- [ ] Given a public reader follows a source link, when the link target is opened, then the reference points to VTTEST, esctest2, xterm control sequence docs or a checked-in local fixture.

#### US-006: Add Cross-Platform Measurement Fields

**Description:** As a public evaluator, I want Windows, Linux and macOS measurement status separated so that local Windows results are not mistaken for universal support.

**Priority:** P1
**Size:** S (2 pts)
**Dependencies:** Blocked by US-004

**Acceptance Criteria:**

- [ ] Given the compatibility matrix, when a row is rendered, then Windows, Linux and macOS measurement fields are visible or included in machine-readable data.
- [ ] Given only Arthur's Windows machine was used, when the row is rendered, then Linux and macOS say `not measured` rather than `pass`.
- [ ] Given a behavior is platform-neutral by code structure, when no platform run exists, then the evidence still remains `not measured` for runtime proof.
- [ ] Given a platform field is omitted, when validation runs, then the matrix fails schema validation.

### EP-003: Benchmarks And Memory Proof

Produce repeatable performance and memory evidence for Hera's core claim: bounded long-session terminal state.

**Definition of Done:** M4 includes benchmark and memory artifacts for ingest, replay, snapshot, comparison and long logical-line scenarios.

#### US-007: Add M4 Benchmark Harness

**Description:** As a Hera implementer, I want repeatable benchmarks for hot paths so that future changes can be judged against measurable baselines.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given benchmark fixtures, when the M4 benchmark command runs, then it measures byte ingest, snapshot generation, replay and comparison.
- [ ] Given benchmark output, when the report is generated, then it includes throughput, latency summary, input size and machine metadata.
- [ ] Given Criterion.rs is used, when benchmark artifacts are stored, then generated bulky target output is not committed unless explicitly summarized.
- [ ] Given a benchmark panics or produces no measurements, when the report command runs, then M4 benchmark status is marked failed instead of emitting empty numbers.

#### US-008: Add 10k, 100k And 1M Logical-Line Memory Profiles

**Description:** As a terminal engine evaluator, I want memory profiles for large scrollback sizes so that Hera's bounded-history claim is measurable.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-007

**Acceptance Criteria:**

- [ ] Given deterministic generated output, when the memory profile command runs for 10k, 100k and 1M logical lines, then it records peak process memory or documented equivalent metrics.
- [ ] Given Hera-owned counters exist, when memory evidence is generated, then row count, visible rows, scrollback rows, byte budget and discarded-row counts are included.
- [ ] Given a run exceeds configured memory budget, when the report is generated, then it marks the scenario as failed and includes the observed value.
- [ ] Given the machine cannot run 1M lines within a documented timeout, when evidence is generated, then the report marks 1M as blocked with timeout and partial metrics instead of inventing a pass.

#### US-009: Define Regression Thresholds And Report Generator

**Description:** As a maintainer, I want benchmark thresholds and report generation so that numbers become an actionable gate rather than a screenshot.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-007, US-008

**Acceptance Criteria:**

- [ ] Given M4 benchmark output, when the report generator runs, then it creates a stable Markdown and machine-readable summary.
- [ ] Given thresholds are configured, when a metric regresses beyond the accepted threshold, then the command exits non-zero or marks the metric failed.
- [ ] Given a benchmark is intentionally unstable on one platform, when the report is generated, then the instability is called out in notes and excluded from hard pass claims.
- [ ] Given a metric has no prior baseline, when thresholds are evaluated, then it is marked as baseline-created rather than pass or fail.

### EP-004: Replay And Demo Artifacts

Make deterministic replay and public-safe dogfood demonstration part of the product evidence.

**Definition of Done:** Hera can ship scrubbed replay artifacts, replay verification output and a Paneflow dogfood demo procedure without exposing private terminal contents.

#### US-010: Create Scrubbed Replay Demo Corpus

**Description:** As a public evaluator, I want replay demos that can be checked into the repo so that deterministic behavior is visible without private sessions.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-003

**Acceptance Criteria:**

- [ ] Given public replay fixtures, when replay verification runs twice, then each fixture produces identical final snapshot hashes and event counts.
- [ ] Given a fixture includes generated command output, when it is inspected, then it contains no private prompt, local username, home path or proprietary transcript text.
- [ ] Given a replay fixture was captured from a local session, when redaction validation runs, then unsafe fields are removed or the fixture is rejected.
- [ ] Given a replay fixture has nondeterministic timestamps or process IDs, when replay verification runs, then those fields are normalized or excluded from the hash.

#### US-011: Add Public Event Stream Export

**Description:** As a replay tool builder, I want a simple event stream export so that Hera sessions can be inspected outside the Rust test harness.

**Priority:** P1
**Size:** M (3 pts)
**Dependencies:** Blocked by US-010

**Acceptance Criteria:**

- [ ] Given a replay fixture, when export runs, then it writes a documented newline-delimited JSON event stream with a versioned header.
- [ ] Given the exported stream is compared to asciicast v2 concepts, when docs are read, then Hera-specific fields and incompatibilities are clearly named.
- [ ] Given an event contains private raw bytes, when export is requested for public mode, then the command refuses or writes a redacted representation.
- [ ] Given an unknown future event kind appears, when the exporter runs, then it preserves versioned metadata or fails with a clear unsupported-event error.

#### US-012: Add Paneflow Dogfood Demo Procedure

**Description:** As a Paneflow maintainer, I want a repeatable public-safe dogfood procedure so that Hera's real-host proof is not anecdotal.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-001, US-003

**Acceptance Criteria:**

- [ ] Given `C:\dev\paneflow`, when the documented dogfood command sequence is run with `PANEFLOW_HERA_DOGFOOD=shadow`, then the demo scenario completes without mismatch report files.
- [ ] Given the artifact directory is inspected after a passing demo, when no mismatch exists, then the report records empty mismatch artifacts as a pass condition.
- [ ] Given mismatch files are produced, when the demo report is generated, then it links sanitized summaries and marks Paneflow proof as failed or partial.
- [ ] Given raw local retention is enabled, when public docs are generated, then they warn that raw artifacts stay local and must not be committed.

### EP-005: Public API And Package Readiness

Show the intended Rust host boundary and package health without prematurely publishing Hera.

**Definition of Done:** Public examples compile, docs render, package dry-runs pass where expected and release blockers are documented.

#### US-013: Add Public API Examples

**Description:** As a future embedder, I want compiling API examples so that I can see the intended Hera integration shape.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given examples are listed in README or docs, when `cargo test --workspace` runs, then code snippets or example binaries compile.
- [ ] Given the headless example is opened, when it is read, then it shows byte ingestion, resize and render snapshot without PTY or Paneflow dependencies.
- [ ] Given the PTY example is opened, when it is read, then it shows the PTY boundary outside `terminal-core`.
- [ ] Given an example needs Paneflow, GPUI or platform internals, when API docs are reviewed, then it is rejected or moved to a host-specific dogfood doc.

#### US-014: Audit API Against Rust API Guidelines

**Description:** As a Rust crate maintainer, I want a public API audit so that obvious naming, docs and error-shape problems are caught before publish pressure.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-013

**Acceptance Criteria:**

- [ ] Given public Hera crates, when the API audit runs, then exported types, errors, feature flags and docs are reviewed against the Rust API Guidelines.
- [ ] Given a public type lacks a clear purpose or docs, when the audit report is written, then it is listed as a blocker or accepted gap.
- [ ] Given a public API leaks parser, PTY, GPUI, Paneflow or platform internals across the wrong crate boundary, when the audit runs, then it is marked P0.
- [ ] Given a breaking rename is recommended, when M4 closes, then the change is either implemented before publish-readiness or recorded as an M5 blocker.

#### US-015: Run Package And Docs Readiness Checks

**Description:** As a maintainer, I want package dry-runs and docs generation so that publishing blockers are visible before any release decision.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-014

**Acceptance Criteria:**

- [ ] Given each Hera crate, when the package-readiness command runs, then package contents are generated or documented failures are recorded.
- [ ] Given `cargo doc --workspace --no-deps` runs, when docs are generated, then public crate docs render without broken intra-doc links.
- [ ] Given workspace metadata still has `publish = false`, when readiness is reported, then the report says publishing is intentionally disabled.
- [ ] Given a developer attempts `cargo publish`, when M4 instructions are followed, then the action is out of scope and must not be performed.

#### US-016: Add OSS Security Health Baseline

**Description:** As an OSS maintainer, I want a lightweight security and supply-chain baseline so that public proof does not ignore trust signals.

**Priority:** P1
**Size:** M (3 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given dependency and repo health checks are available locally or in CI, when the baseline is run, then results are summarized in the M4 report.
- [ ] Given OpenSSF Scorecard is referenced, when the report is read, then it distinguishes external score from Hera's internal security posture.
- [ ] Given a dependency advisory or secret-like value is found, when the baseline runs, then the report marks M4 public proof blocked until triaged.
- [ ] Given a check cannot run on the local machine, when the report is generated, then it records the exact command, failure and next action instead of omitting the check.

### EP-006: Public Proof Report And M5 Recommendation

Assemble the public proof package into a concise, honest report and make the next milestone decision explicit.

**Definition of Done:** `docs/m4-public-proof-report.md` exists, links all evidence, states pass/fail/partial status and recommends the next M5 scope.

#### US-017: Write M4 Public Proof Report

**Description:** As a reader, I want one report that summarizes Hera's public proof so that I can evaluate the project without reconstructing it from raw artifacts.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-003, US-004, US-007, US-008, US-010, US-012, US-013, US-015

**Acceptance Criteria:**

- [ ] Given all M4 evidence artifacts, when the report is written, then it includes compatibility, replay, benchmarks, memory, Paneflow demo, API examples, package readiness and known gaps.
- [ ] Given a section has partial or failed evidence, when the report is read, then the failure is visible in the summary and not buried in appendix text.
- [ ] Given public artifacts link to local-only paths, when report validation runs, then those links are rejected or marked local-only.
- [ ] Given the report makes a claim, when reviewers inspect it, then the claim is backed by a command, artifact, fixture or explicit source reference.

#### US-018: Update Milestone Docs And M5 Recommendation

**Description:** As Arthur, I want README and research-map milestone text updated so that future agents know whether to move toward M5 replacement, compatibility expansion or API hardening.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-017

**Acceptance Criteria:**

- [ ] Given README is opened after M4, when milestone status is read, then it no longer says the next step is M1, M2 or M3 work.
- [ ] Given `docs/research-map.md` is opened after M4, when current status is read, then it reflects M4 evidence and the recommended M5 path.
- [ ] Given M4 fails a major proof category, when M5 is recommended, then it stays on evidence or compatibility hardening instead of replacement.
- [ ] Given M4 passes public proof, when M5 is recommended, then it names the next concrete chantier and the evidence threshold that unlocked it.

## Release Criteria

M4 is complete only when:

- `docs/m4-public-proof-report.md` exists and links every public evidence artifact.
- Compatibility matrix rows are machine-readable and include measured versus unmeasured status.
- Replay demo fixtures are scrubbed, deterministic and verified twice.
- 10k, 100k and 1M logical-line memory profiles are present or the missing scenario is explicitly blocked with evidence.
- Benchmark results are generated with machine metadata and thresholds.
- Paneflow dogfood has a public-safe demo procedure and current pass/fail status.
- Public API examples compile.
- Package/docs readiness is checked without publishing.
- README and `docs/research-map.md` point to the correct post-M4 direction.
- `tasks/prd-m4-public-proof-status.json` is updated to `DONE` with every story completed and reviewed.

## Risk Register

| Risk | Severity | Mitigation |
|------|----------|------------|
| Public proof leaks private terminal data | High | Redaction policy, manifest classification and rejection of `raw_local` artifacts |
| 1M-line profile is too slow for local runs | Medium | Record timeout, partial metrics and budget gap instead of claiming pass |
| Benchmarks become noisy screenshots | Medium | Generate machine-readable summaries and thresholds |
| Compatibility matrix overclaims support | High | Require fixture/source link for `implemented` status and explicit `not measured` fields |
| Package readiness creates publish pressure | Medium | Keep `publish = false`, run dry-runs only and state publish is out of scope |
| Paneflow dogfood remains local-only | Medium | Provide a scrubbed demo procedure and summarize empty mismatch artifacts as evidence |

## Implementation Notes

- Prefer generated machine-readable evidence plus a human Markdown report. Do not make screenshots the primary proof.
- Keep raw dogfood captures local by default. Public artifacts should be summaries, scrubbed replays or deterministic generated fixtures.
- Treat Windows results as Arthur's local baseline unless Linux/macOS are actually run.
- If M4 uncovers major correctness gaps, the correct M5 recommendation is compatibility hardening, not replacement.
- If M4 passes with strong evidence, the likely M5 direction is a broader host-integration or renderer-adapter milestone, still behind feature flags.
[/PRD]
