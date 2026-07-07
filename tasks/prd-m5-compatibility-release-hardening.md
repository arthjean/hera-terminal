[PRD]
# PRD: M5 Compatibility And Release Hardening

## Changelog

| Version | Date | Author | Summary |
|---------|------|--------|---------|
| 1.0 | 2026-07-04 | Arthur Jean | Initial draft for Hera M5 compatibility and release hardening |

## Problem Statement

1. M4 produced a public proof package, but its verdict is explicitly partial. Hera now has inspectable evidence, but the remaining gaps still block public adoption and any Paneflow replacement experiment.
2. The M4 compatibility matrix names unimplemented or deferred terminal behavior: CSI cursor positioning, ED/EL/ECH erasure, DEC private modes 47/1047/1048, real-session replay breadth and Linux/macOS runtime measurements.
3. The M4 package-readiness report shows release blockers: missing package metadata, unresolved internal publish order, blocked package dry-runs for dependent crates, no external OpenSSF Scorecard result and only partial OSS security posture.
4. Without M5, the project can create a visible terminal surface or publish crates before the terminal behavior, public API surface, package artifacts and platform proof are ready enough to trust.

**Why now:** M4 is DONE and locally verified. The next decision is no longer "can Hera be demonstrated?" It is "can Hera be hardened enough that M6 can make an evidence-based choice between broader host integration, public pre-release packaging or more compatibility work?"

## Overview

M5 turns Hera's M4 public proof into compatibility and release hardening. It closes or explicitly defers the concrete terminal gaps named by M4, adds scrubbed real-session replay derivatives, measures Windows/Linux/macOS rows, makes the package metadata and publish order coherent, adds a stronger OSS security baseline and writes a final go/no-go report for M6.

The milestone is intentionally not a renderer milestone. Paneflow remains authoritative. Hera dogfood stays behind feature flags and shadow mode. M5 can unlock a future host replacement experiment only if it produces measured evidence: no P0 mismatch in a broader Paneflow shadow scenario, pass or explicitly deferred status for core compatibility gaps, measured platform rows, package dry-runs that match the release policy and a security posture report that does not hide blocked checks.

The output is a checked-in M5 evidence package under `evidence/m5/`, a human report at `docs/m5-compatibility-release-hardening-report.md`, updated compatibility and release-readiness docs, expanded fixtures and updated milestone guidance in README and `docs/research-map.md`.

## Goals

| Goal | Month-1 Target | Month-6 Target |
|------|---------------|----------------|
| Compatibility closure | CSI positioning and ED/EL/ECH either fixture-backed pass or explicitly deferred with rationale | 80 percent of core M5 matrix rows are fixture-backed pass across target platforms |
| DEC private mode policy | 47, 1047 and 1048 have pass or deferred-policy rows with tests | Private mode behavior is stable enough for Paneflow replacement experiments |
| Real-session replay | 2 scrubbed derivatives exist: one Codex and one Claude Code session | 10 scrubbed real-session derivatives cover shells, agents and build tools |
| Platform evidence | Windows, Linux and macOS rows are measured or blocked with exact command output | CI produces platform evidence on every main-branch change |
| Package readiness | All intended public crates either package successfully or have one documented release blocker | All intended public crates are pre-release publish-ready without running `cargo publish` |
| Security posture | RustSec/cargo-deny or equivalent local checks plus OpenSSF Scorecard path are recorded | Security posture checks run in CI and block high-confidence advisories/secrets |

## Target Users

### Hera Maintainer

- **Role:** Arthur or an implementation agent hardening Hera after M4.
- **Behaviors:** Implements terminal semantics, fixtures, validators, package metadata, platform evidence and report updates.
- **Pain points:** M4 shows useful proof but leaves too many `partial`, `not_measured` and `blocked` cells for adoption-grade confidence.
- **Current workaround:** Read M4 report sections manually and choose fixes ad hoc.
- **Success looks like:** M5 report has a concrete M6 recommendation backed by compatibility, replay, package, platform and security artifacts.

### Future Hera Embedder

- **Role:** Developer considering Hera for a GUI, TUI, remote terminal, replay viewer or agent workspace.
- **Behaviors:** Checks compatibility matrix, examples, docs, package artifacts and platform support before integrating.
- **Pain points:** An embeddable terminal core is risky if behavior claims are vague or packaging breaks downstream builds.
- **Current workaround:** Use mature engines directly or keep Hera as an experimental local dependency.
- **Success looks like:** The embedder can tell which crates are intended public surfaces, which terminal behaviors pass, and which gaps are intentionally deferred.

### Paneflow Maintainer

- **Role:** Arthur evaluating whether Hera can move beyond shadow dogfood.
- **Behaviors:** Runs Paneflow with `hera-dogfood`, compares mismatch evidence, watches long agent sessions and checks rapid-output behavior.
- **Pain points:** A targeted smoke pass is not enough to replace a production terminal path used by real coding agents.
- **Current workaround:** Keep Alacritty authoritative and treat Hera as a shadow comparator.
- **Success looks like:** A longer shadow run creates no P0 mismatch evidence, and any mismatch has a sanitized artifact and owner.

## Research Findings

Key findings that informed this PRD:

### Competitive Context

- Mature terminal projects use compatibility tests and reference behavior as proof, not slogans. VTTEST exercises VT100, VT220, VT420 and xterm-like behavior, and remains a practical compatibility oracle for terminal emulators: https://invisible-island.net/vttest/.
- xterm control sequences are still the practical reference for CSI, DEC private modes, screen erasure and many xterm-specific sequences Hera must classify: https://www.xfree86.org/current/ctlseqs.html.
- esctest2 provides an automated xterm-style compatibility reference. M5 should use its categories to structure coverage without claiming full parity: https://github.com/ThomasDickey/esctest2.
- The market gap is not another terminal app. It is a Rust-public, replayable, renderer-neutral terminal engine that can show measured compatibility and release readiness before UI replacement work.

### Best Practices Applied

- Cargo package metadata, include/exclude rules and publishing checks must be treated as release infrastructure, not paperwork. Cargo's manifest and publishing docs are the authority for package metadata and package dry-runs: https://doc.rust-lang.org/cargo/reference/manifest.html and https://doc.rust-lang.org/cargo/reference/publishing.html.
- docs.rs builds crate docs from crates.io packages and supports per-package metadata such as features and target arguments. M5 should make documentation policy explicit before publish pressure: https://docs.rs/about and https://docs.rs/about/metadata.
- Rust API Guidelines call out common traits, meaningful errors, `Send`/`Sync` where possible, Serde support where appropriate and documentation quality. M5 should convert M4's API audit into release-hardening action: https://rust-lang.github.io/api-guidelines/checklist.html.
- RustSec and cargo-deny cover advisories, licenses, bans, duplicate versions and sources. M5 should record which checks run, which are blocked and which findings are release blockers: https://rustsec.org/ and https://github.com/embarkstudios/cargo-deny.
- OpenSSF Scorecard can run through a GitHub Action or CLI. M5 should not invent a score; it should run the check when available or document the exact blocked state: https://scorecard.dev/.

## Assumptions & Constraints

### Assumptions To Validate

- M4's existing matrix and validators can be extended for M5 without replacing the evidence contract.
- CSI positioning, ED/EL/ECH and DEC private modes can be implemented or explicitly deferred within one milestone without pulling in a renderer.
- Public-safe derivatives of real Codex and Claude Code sessions can be produced without exposing prompts, local paths, user names, tokens or private transcript text.
- Linux and macOS measurement can be produced through CI or separate local runners; if not, blocked evidence must include exact commands and failure details.
- Package readiness can be improved without actually publishing crates to crates.io.

### Hard Constraints

- M5 must not run `cargo publish`.
- M5 must not make Paneflow use Hera as the default authoritative terminal path.
- `terminal-core` must remain free of PTY, GPUI, Paneflow, renderer, windowing and platform dependencies.
- Public artifacts must use repo-relative paths and must not contain raw private terminal bytes.
- Compatibility rows must separate implemented, fixture-backed, platform-measured, deferred and out-of-scope states.
- If a platform or security check cannot run, M5 must record a blocked state with command, exit code and reason.

## Quality Gates

These commands must pass for every Hera story unless the story is documentation-only and explicitly records why code gates are unchanged:

- `cargo fmt --all -- --check` - Hera workspace formatting is stable.
- `cargo check --workspace` - Hera workspace typechecks.
- `cargo clippy --workspace --all-targets -- -D warnings` - Hera lints are blocking.
- `cargo test --workspace` - Hera unit, fixture, replay and CLI tests pass.
- `cargo doc --workspace --no-deps` - Public docs render without broken intra-doc links.
- `git diff --check` - No whitespace or conflict-marker issues are present.

Additional gates for M5 evidence stories:

- `cargo run -p terminal-cli -- validate-m5-evidence evidence/m5/evidence-manifest.json` - M5 evidence manifest validates after the validator exists.
- `cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json` - M5 compatibility rows validate after the validator exists.
- `cargo run -p terminal-cli -- verify-m5-replay crates/terminal-fixtures/fixtures/m5-replay --json-output evidence/m5/m5-replay-verification.json` - Public M5 replay fixtures are deterministic after the command exists.

Additional gates for Paneflow dogfood stories:

- `cargo check --workspace --features hera-dogfood` from `C:\dev\paneflow` - Paneflow dogfood feature typechecks.
- `cargo test --workspace --features hera-dogfood` from `C:\dev\paneflow` - Paneflow dogfood tests pass.
- M5 Paneflow shadow dogfood command from the report - no P0 mismatch artifacts are produced.

Additional gates for release-hardening stories:

- `cargo package -p terminal-protocol --allow-dirty --no-verify`
- `cargo package -p terminal-render-model --allow-dirty --no-verify`
- `cargo package -p terminal-core --allow-dirty --no-verify`
- `cargo package -p terminal-fixtures --allow-dirty --no-verify`
- `cargo package -p terminal-pty --allow-dirty --no-verify`
- `cargo package -p terminal-cli --allow-dirty --no-verify`

Package commands are readiness checks only. `cargo publish` is explicitly out of scope.

## Epics & User Stories

### EP-001: M5 Baseline And Evidence Policy

Convert M4's partial verdict into a precise M5 baseline, evidence schema and go/no-go policy.

**Definition of Done:** M5 has a machine-readable evidence contract, a compatibility/release baseline and explicit thresholds for M6 recommendations.

#### US-001: Carry M4 Blockers Into M5 Baseline

**Description:** As a Hera maintainer, I want M4's blockers translated into M5 baseline evidence so that hardening starts from measured gaps.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** None

**Acceptance Criteria:**

- [ ] Given `docs/m4-public-proof-report.md`, when M5 baseline is generated, then CSI positioning, ED/EL/ECH, DEC private modes, real-session replay, Linux/macOS measurement, package metadata, publish order and OpenSSF Scorecard gaps are listed.
- [ ] Given `tasks/prd-m4-public-proof-status.json`, when M5 baseline is generated, then M4 `DONE` status is referenced without modifying M4 history.
- [ ] Given an M4 blocker is already resolved before M5 starts, when baseline generation runs, then it records current evidence instead of duplicating stale blocker text.
- [ ] Given an M4 artifact is missing or malformed, when baseline generation runs, then M5 status stays `READY` but the baseline marks the dependency as blocked with the missing path.

#### US-002: Define M5 Evidence Manifest And Validators

**Description:** As a future reviewer, I want M5 artifacts governed by a manifest and validators so that claims stay reproducible and public-safe.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**

- [ ] Given `evidence/m5/evidence-manifest.json`, when validation runs, then every artifact has id, type, path, source command, generated timestamp, privacy class, platform field and reproducibility status.
- [ ] Given a public M5 JSON artifact, when validation scans it, then raw transcript fields, local home paths, tokens and `raw_local` privacy class are rejected.
- [ ] Given an artifact path is absolute, when validation runs, then it fails with the artifact id and path.
- [ ] Given a source command cannot be rerun locally, when validation runs, then the artifact is allowed only if it is marked non-reproducible with a reason and owner.

#### US-003: Define M5 Go/No-Go Thresholds

**Description:** As Arthur, I want explicit thresholds for M6 so that the next milestone is chosen by evidence instead of momentum.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**

- [ ] Given the M5 report, when the M6 recommendation is read, then it names one of three outcomes: host replacement experiment, public pre-release packaging, or another compatibility hardening milestone.
- [ ] Given any P0 compatibility row is `failed` or `not_implemented`, when M5 closes, then host replacement is marked blocked.
- [ ] Given any intended public crate package dry-run fails without an accepted blocker, when M5 closes, then public pre-release packaging is marked blocked.
- [ ] Given the go/no-go thresholds are ambiguous, when report validation runs, then it fails until every outcome has measurable criteria.

### EP-002: Core VT Compatibility Hardening

Close the M4 compatibility gaps or explicitly defer them with tests, rationale and matrix rows.

**Definition of Done:** CSI positioning, ED/EL/ECH and DEC private modes have fixture-backed pass or documented deferred status, and the compatibility matrix no longer hides those gaps.

#### US-004: Implement CSI Cursor Positioning Fixtures

**Description:** As a terminal engine evaluator, I want CSI cursor positioning covered by fixtures so that Hera can handle common cursor-addressed output.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given a fixture with `CUP` and `HVP`, when replayed, then Hera places printable cells at the expected one-based row and column positions.
- [ ] Given missing CSI parameters, when cursor positioning is parsed, then defaults follow xterm-compatible one-based behavior documented in the fixture notes.
- [ ] Given zero, negative-equivalent or out-of-range parameters, when replayed, then Hera clamps or rejects according to documented terminal policy without panicking.
- [ ] Given the compatibility matrix is regenerated, when `vt.cursor.csi_positioning` is read, then it is fixture-backed pass or records a specific failing fixture.

#### US-005: Implement ED, EL And ECH Erasure Semantics

**Description:** As a terminal engine evaluator, I want display, line and character erasure semantics covered so that common full-screen programs render predictably.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-004

**Acceptance Criteria:**

- [ ] Given fixtures for `ED` modes 0, 1 and 2, when replayed, then Hera erases the correct viewport cells without corrupting scrollback rows outside policy.
- [ ] Given fixtures for `EL` modes 0, 1 and 2, when replayed, then Hera erases the expected cells on the active row and preserves cursor position.
- [ ] Given fixtures for `ECH`, when replayed, then Hera clears the requested number of cells from the cursor without shifting following content.
- [ ] Given an erasure parameter is unsupported, when replayed, then Hera records an unsupported action and preserves terminal state instead of clearing too much.

#### US-006: Resolve DEC Private Modes 47, 1047 And 1048

**Description:** As a Paneflow maintainer, I want DEC private alternate-screen variants resolved or explicitly deferred so that shadow comparison is not misleading.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given DECSET/DECRST 47, when replayed, then Hera either switches screens according to documented policy or marks the mode deferred in matrix data.
- [ ] Given DECSET/DECRST 1047, when replayed, then primary scrollback and alternate screen behavior match the documented policy.
- [ ] Given DECSET/DECRST 1048, when replayed, then cursor save/restore behavior is fixture-backed or explicitly deferred.
- [ ] Given a private mode is deferred, when the matrix is rendered, then it is not counted as pass and includes the reason, owner and M6 follow-up.

#### US-007: Expand M5 Compatibility Matrix And Validator

**Description:** As a reviewer, I want the compatibility matrix expanded and stricter so that M5 cannot overclaim terminal support.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-004, US-005, US-006

**Acceptance Criteria:**

- [ ] Given `evidence/m5/compatibility-matrix.json`, when validation runs, then every row has behavior id, category, source reference, fixture coverage, Windows/Linux/macOS measurement and M5 disposition.
- [ ] Given a row claims pass, when validation runs, then at least one checked-in fixture or replay artifact exists and is referenced by name.
- [ ] Given VTTEST, xterm or esctest2 source links are missing for a core behavior row, when validation runs, then the row fails.
- [ ] Given a row is deferred, when docs render, then it is visible in the summary and excluded from pass percentage.

### EP-003: Real-Session Replay And Dogfood Hardening

Move from synthetic proof toward scrubbed real-session evidence and broader Paneflow shadow dogfood.

**Definition of Done:** M5 includes public-safe Codex and Claude Code replay derivatives, rapid-output coverage and a Paneflow shadow report with mismatch severity.

#### US-008: Add Scrubbed Codex And Claude Code Replay Derivatives

**Description:** As a public evaluator, I want scrubbed real-session derivatives so that Hera's replay proof includes agent-like terminal behavior.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given a Codex session capture, when public derivative generation runs, then output is scrubbed, versioned and replayable without raw private prompt text.
- [ ] Given a Claude Code session capture, when public derivative generation runs, then output is scrubbed, versioned and replayable without raw private prompt text.
- [ ] Given redaction finds a local path, token-like value or raw input transcript, when derivative generation runs, then the artifact is rejected and no public file is written.
- [ ] Given replay verification runs twice, when derivatives are replayed, then final snapshot hashes and event counts match.

#### US-009: Add Rapid-Output And Long-Session Replay Coverage

**Description:** As a Paneflow maintainer, I want rapid-output and long-session replay fixtures so that Hera is tested against the output patterns that break terminal integrations.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-008

**Acceptance Criteria:**

- [ ] Given a generated rapid-output fixture, when replayed, then Hera completes within the documented timeout and produces deterministic final snapshots.
- [ ] Given a generated long-session fixture with at least 100k logical lines, when replayed, then Hera stays within the M5 memory budget and records discarded-row counts.
- [ ] Given replay exceeds timeout or memory budget, when report generation runs, then the scenario is marked failed with observed values.
- [ ] Given a mismatch occurs, when minimization runs, then the report links the smallest public-safe fixture or records why minimization is blocked.

#### US-010: Run Broader Paneflow Shadow Dogfood Scenario

**Description:** As a Paneflow maintainer, I want a broader shadow dogfood run so that M5 can judge whether Hera is ready for any host replacement experiment.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-007, US-009

**Acceptance Criteria:**

- [ ] Given `PANEFLOW_HERA_DOGFOOD=shadow`, when the M5 dogfood scenario runs, then it records duration, command classes, pane count, mismatch count and artifact directory.
- [ ] Given no P0 mismatch files are produced, when the M5 report is generated, then Paneflow dogfood status is `targeted_pass`.
- [ ] Given any mismatch is produced, when the report is generated, then it is classified P0/P1/P2 with sanitized summary and reproduction pointer.
- [ ] Given raw local retention is used, when public artifacts are generated, then raw bytes remain outside the Hera repo and only scrubbed summaries are linked.

### EP-004: Cross-Platform Runtime Evidence

Replace Windows-only local confidence with measured Windows, Linux and macOS evidence or precise blocked states.

**Definition of Done:** M5 platform evidence rows show what was measured on Windows, Linux and macOS, with commands, exit codes and artifact links.

#### US-011: Define Platform Measurement Runner

**Description:** As a maintainer, I want one documented platform measurement command set so that Windows, Linux and macOS results are comparable.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given a platform runner command, when it completes, then it writes OS, target triple, rustc version, command list, exit codes and generated artifact paths.
- [ ] Given a command is unavailable on a platform, when the runner executes, then the row is marked blocked with stderr summary and does not become pass.
- [ ] Given platform evidence is generated, when validation runs, then Windows, Linux and macOS fields are present for every required M5 command.
- [ ] Given a platform runner writes an absolute local path into public evidence, when validation runs, then the artifact fails.

#### US-012: Record Windows M5 Runtime Evidence

**Description:** As Arthur, I want Windows runtime evidence refreshed under M5 so that local development remains the primary measured baseline.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** Blocked by US-011

**Acceptance Criteria:**

- [ ] Given the Windows runner, when it runs on Arthur's machine, then `cargo test --workspace`, M5 compatibility validation and M5 replay verification are recorded.
- [ ] Given a Windows command fails, when evidence is generated, then M5 platform status is failed with command and exit code.
- [ ] Given Windows evidence is older than the current M5 artifact timestamp policy, when validation runs, then it is marked stale.
- [ ] Given Windows-only evidence is present, when docs render, then Linux and macOS are not inferred as pass.

#### US-013: Record Linux And macOS Runtime Evidence

**Description:** As a future embedder, I want Linux and macOS measurements so that Hera does not generalize Windows-only results.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-011

**Acceptance Criteria:**

- [ ] Given Linux runner output, when evidence is generated, then it records `cargo test --workspace`, M5 compatibility validation and M5 replay verification.
- [ ] Given macOS runner output, when evidence is generated, then it records `cargo test --workspace`, M5 compatibility validation and M5 replay verification.
- [ ] Given CI or local runner access is unavailable, when M5 closes, then Linux or macOS status is blocked with exact attempted command and missing prerequisite.
- [ ] Given either platform fails a core M5 command, when the final report is generated, then host replacement and public pre-release packaging are blocked unless the failure is explicitly scoped out.

### EP-005: Release And API Hardening

Make Hera package-ready without publishing and convert API audit gaps into actionable release policy.

**Definition of Done:** Intended public crates have metadata, docs.rs policy, package dry-runs, publish order and API stability notes.

#### US-014: Add Package Metadata And Docs.rs Policy

**Description:** As a Rust crate maintainer, I want crate metadata and docs.rs configuration so that Hera package artifacts are understandable before publication.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**

- [ ] Given each intended public crate manifest, when inspected, then description, license or license-file, repository, documentation, readme and keywords/categories policy are present or intentionally omitted with reason.
- [ ] Given docs.rs metadata is needed, when manifests are inspected, then feature and target behavior is documented per crate.
- [ ] Given metadata is incomplete, when package-readiness validation runs, then the crate is marked release-blocked.
- [ ] Given private or experimental crates are not intended for publication, when docs render, then they are listed as internal or explicitly non-publishable.

#### US-015: Decide Publish Order And Make Package Dry-Runs Coherent

**Description:** As a maintainer, I want a release plan and package dry-runs so that internal path dependencies stop blocking readiness evidence.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-014

**Acceptance Criteria:**

- [ ] Given the workspace dependency graph, when the release plan is generated, then crates are ordered so dependencies appear before dependents.
- [ ] Given a crate remains `publish = false`, when package-readiness runs, then it is marked intentionally internal and excluded from public publish order.
- [ ] Given `cargo package` fails for an intended public crate, when the report is generated, then failure output is summarized and M5 release readiness is blocked.
- [ ] Given all intended public crates package, when the report is generated, then it still states that `cargo publish` was not run.

#### US-016: Harden Public API And Semver Baseline

**Description:** As a future embedder, I want a stable pre-release API baseline so that Hera does not accidentally expose parser, PTY or host internals.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-014

**Acceptance Criteria:**

- [ ] Given public Hera crates, when API audit runs, then exported types are checked for docs, meaningful errors, common trait implementations and boundary leaks.
- [ ] Given `vte`, `portable-pty`, Paneflow, GPUI or platform types leak across the wrong public boundary, when audit runs, then the issue is P0.
- [ ] Given `cargo-semver-checks` or an equivalent semver baseline tool is unavailable, when API evidence is generated, then the blocked command is recorded instead of claiming semver coverage.
- [ ] Given a public API rename is recommended, when M5 closes, then it is either implemented before pre-release readiness or listed as an M6 release blocker.

### EP-006: Security Posture And Final M5 Report

Close the milestone with a security baseline, final report and M6 recommendation grounded in evidence.

**Definition of Done:** M5 has a supply-chain/security artifact, a final report and updated milestone docs.

#### US-017: Add Supply-Chain And Security Baseline

**Description:** As an OSS maintainer, I want Rust and OpenSSF security checks recorded so that release hardening includes trust posture.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-014

**Acceptance Criteria:**

- [ ] Given RustSec or cargo-audit is available, when the security baseline runs, then advisory findings are recorded with severity and release-blocking status.
- [ ] Given cargo-deny is available, when the baseline runs, then advisories, licenses, bans, duplicate versions and sources are recorded.
- [ ] Given OpenSSF Scorecard CLI or GitHub Action is available, when it runs, then the result is recorded without treating score alone as release approval.
- [ ] Given any security tool is unavailable, when the baseline report is generated, then it records install status, attempted command and blocked reason.

#### US-018: Write M5 Final Report And Update Milestone Docs

**Description:** As Arthur, I want a final M5 report so that M6 starts from a clear decision rather than another audit pass.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-003, US-007, US-010, US-013, US-015, US-016, US-017

**Acceptance Criteria:**

- [ ] Given all M5 evidence, when `docs/m5-compatibility-release-hardening-report.md` is written, then it summarizes compatibility, replay, dogfood, platform, package, API and security status.
- [ ] Given any M5 category is failed or blocked, when the report is read, then the final M6 recommendation reflects that blocker.
- [ ] Given README and `docs/research-map.md` are opened after M5, when milestone text is read, then they match the final M5 verdict and recommended M6 chantier.
- [ ] Given a report claim lacks command, artifact or source reference, when report validation runs, then the claim is rejected or marked as rationale instead of evidence.

## Functional Requirements

- FR-01: The system must extend the compatibility matrix with M5 rows for CSI positioning, ED/EL/ECH and DEC private modes.
- FR-02: The system must provide fixture or replay artifacts for every M5 compatibility row marked pass.
- FR-03: The system must generate public-safe replay derivatives for at least one Codex and one Claude Code session or record blocked evidence.
- FR-04: The system must record Windows, Linux and macOS measurement rows with commands, exit codes and artifact paths.
- FR-05: The system must generate a package-readiness report that distinguishes public crates, internal crates and blocked crates.
- FR-06: The system must not run `cargo publish`.
- FR-07: The system must not make Paneflow use Hera as the default authoritative terminal path.
- FR-08: The system must reject public artifacts containing raw private terminal bytes, local home paths, prompt text or token-like values.

## Non-Functional Requirements

- **Compatibility Coverage:** M5 final matrix must contain at least 18 rows and every pass row must have at least 1 checked-in fixture or replay artifact.
- **Replay Determinism:** M5 public replay verification must run each fixture 2 times and require identical final snapshot hash plus event count.
- **Memory:** 100k logical-line replay scenarios must stay within the configured Hera-owned byte budget from M4 or record a failed metric.
- **Platform Evidence:** Windows, Linux and macOS must each have a status for at least 5 required commands: check, test, doc, compatibility validation and replay verification.
- **Security:** Any high-confidence advisory, secret-like value or raw private artifact in public evidence blocks M5 release readiness.
- **Packaging:** 100 percent of intended public crates must either package successfully or be listed with one explicit blocker and owner.
- **Dogfood:** M5 Paneflow shadow scenario must produce 0 P0 mismatch artifacts to unlock host replacement experiments.

## Edge Cases & Error States

| # | Scenario | Trigger | Expected Behavior | User Message |
|---|----------|---------|-------------------|--------------|
| 1 | Missing M4 evidence | M4 report or evidence artifact is absent | M5 baseline marks dependency blocked and does not invent pass status | "M4 artifact missing: {path}" |
| 2 | Unsupported escape sequence | Fixture includes unimplemented CSI or DEC mode | Hera records unsupported action or documented deferred status without panicking | "Unsupported terminal action recorded" |
| 3 | Redaction failure | Public replay derivative contains private path or token-like value | Artifact generation fails and no public file is written | "Public artifact rejected by redaction policy" |
| 4 | Platform runner unavailable | Linux or macOS runner cannot execute | Platform row is `blocked` with command and reason | "Platform measurement blocked: {reason}" |
| 5 | Package resolution failure | `cargo package` cannot resolve unpublished internal dependency | Crate status is blocked with release-plan action | "Package blocked by internal dependency order" |
| 6 | Security tool unavailable | cargo-deny, cargo-audit or Scorecard is missing | Security report records attempted command and blocked reason | "Security check blocked: tool unavailable" |
| 7 | Paneflow mismatch | Shadow dogfood writes mismatch artifact | Final report classifies severity and blocks replacement if P0 | "P0 mismatch blocks host replacement" |

## Risks & Mitigations

| # | Risk | Probability | Impact | Mitigation |
|---|------|-------------|--------|------------|
| 1 | Compatibility scope expands beyond 20 stories | Med | High | Keep M5 limited to named M4 blockers plus matrix expansion; defer broad VTTEST parity to M6 or later |
| 2 | Real-session derivatives leak private data | Med | High | Redaction validator rejects paths, tokens, raw input and `raw_local`; raw captures stay outside repo |
| 3 | Linux/macOS measurement is unavailable | Med | Med | Record blocked evidence with exact command and prerequisite; do not infer pass from Windows |
| 4 | Package readiness turns into accidental publish | Low | High | Keep `cargo publish` out of gates and explicitly mark publish out of scope |
| 5 | API hardening causes breaking churn late in milestone | Med | Med | Audit early in EP-005 and classify renames as implemented or M6 blockers |
| 6 | Security tooling creates noisy blockers | Med | Med | Separate advisory severity, duplicate dependency notes and unavailable tool states |

## Non-Goals

Explicit boundaries for M5:

- No standalone Hera desktop terminal app.
- No GPUI renderer or GPU rendering milestone.
- No default Paneflow replacement path.
- No crates.io publication and no semver stability promise.
- No full VTTEST/esctest2 parity claim.
- No Kitty, iTerm2 or Sixel image rendering.
- No telemetry upload of terminal or dogfood artifacts.

## Files NOT to Modify

- `target/` - generated build output, never part of public evidence.
- `C:\dev\paneflow` default terminal path - M5 may use dogfood feature gates only; it must not make Hera authoritative by default.
- `docs/reference-inventory/` - update only when new research changes a durable architectural decision.
- `crates/terminal-core` public boundary - do not add PTY, GPUI, Paneflow, renderer or platform dependencies.

## Technical Considerations

- **Architecture:** M5 should extend the existing M4 evidence pattern under `evidence/m5/`. Engineering to confirm whether validators share M4 types or get versioned M5 modules.
- **Compatibility Semantics:** CSI, ED/EL/ECH and DEC private modes should be implemented in `terminal-core` only if fixtures prove behavior. Deferred policy must be explicit in matrix data.
- **Replay Data:** Real-session derivatives should preserve terminal-shape evidence while removing private content. Engineering to confirm whether derivative generation lives in `terminal-fixtures` or `terminal-cli`.
- **Platform Evidence:** CI matrix is recommended for Linux and macOS if local machines are unavailable. Engineering to confirm whether GitHub Actions belongs in M5 or whether blocked evidence is acceptable.
- **Release Metadata:** Workspace metadata can centralize package values, but docs.rs metadata may need per-crate entries. Engineering to confirm actual Cargo behavior before editing every manifest.
- **Security Tooling:** cargo-deny or cargo-audit may need installation. Engineering to decide whether M5 commits config files or records local command output only.

## Success Metrics

| Metric | Baseline (current) | Target | Timeframe | How Measured |
|--------|--------------------|--------|-----------|--------------|
| M5 compatibility matrix rows | 11 M4 rows | At least 18 M5 rows | M5 close | `evidence/m5/compatibility-matrix.json` |
| M5 pass rows with fixtures | M4 pass rows fixture-backed, key gaps not implemented | 100 percent of pass rows fixture-backed | M5 close | M5 compatibility validator |
| Real-session replay derivatives | 0 public Codex/Claude derivatives in M4 | 2 public-safe derivatives or blocked evidence | M5 close | M5 replay verification report |
| Platform measurement rows | Windows pass, Linux/macOS not measured | Windows/Linux/macOS have pass, failed or blocked command evidence | M5 close | M5 platform report |
| Intended public crate package readiness | 2 leaf crates package, 4 blocked | 100 percent package pass or explicit blocker | M5 close | M5 package-readiness report |
| P0 Paneflow mismatches | Targeted M4 smoke produced 0 mismatch files | 0 P0 mismatch artifacts in M5 scenario | M5 close | M5 dogfood report |
| Security posture checks | Local partial baseline, Scorecard not measured | Rust advisory check plus OpenSSF run or blocked evidence | M5 close | M5 security baseline |

## Open Questions

- Who decides whether blocked Linux/macOS evidence is acceptable for M5 closeout if CI cannot run by the time stories are implemented?
- Which crates are intended public surfaces for first pre-release: all six crates or only `terminal-core`, `terminal-protocol`, `terminal-render-model` and `terminal-pty`?
- Should M6 prioritize host replacement experiment or public pre-release packaging if M5 passes compatibility but package readiness remains partial?
[/PRD]
