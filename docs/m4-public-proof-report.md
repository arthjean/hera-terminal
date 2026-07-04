# M4 Public Proof Report

Date: 2026-07-04
Status: EP-006 complete. M4 public proof package assembled, with partial public proof and explicit M5 hardening recommendation.
Scope: EP-001 evidence baseline, EP-002 compatibility matrix, EP-003 benchmark and memory evidence, EP-004 replay and dogfood demo artifacts, EP-005 API/package readiness, and EP-006 final report plus M5 recommendation.

## Summary

M4 starts from a verified M3 closeout, but it does not yet claim full public
terminal proof. The package now consolidates the public artifact layout, the
evidence manifest contract, redaction rules, the first Paneflow dogfood smoke
summary, a structured compatibility matrix, the first benchmark and memory proof
package, public replay plus dogfood demo package, compiling public API examples,
package readiness evidence and an OSS security baseline.

The current public claim is narrow: M3 is marked `DONE`, checked-in M3 evidence
still proves schema and replay mechanics more than real production parity, and a
targeted Paneflow Hera shadow smoke compiled and passed without writing mismatch
reports. EP-003 adds measurable M4 performance evidence: latency metrics create
baselines rather than pass claims, while 10k, 100k and 1M logical-line memory
profiles pass the configured Hera-owned byte budget. EP-004 adds three
public-safe replay fixtures, deterministic replay verification, a Hera-specific
JSONL event stream export and a repeatable Paneflow shadow demo procedure.
EP-005 adds a headless core example, a PTY boundary example, docs readiness,
package dry-run evidence and a lightweight supply-chain baseline.

M4 is useful public evidence, not a release green light. The strongest categories
are replay determinism, bounded Hera-owned memory counters, public API boundary
examples and redaction discipline. The weaker categories are terminal
compatibility breadth, cross-platform runtime proof, real-session dogfood breadth,
package metadata, internal publish order and external OpenSSF Scorecard coverage.

| Decision | Result | Why |
|---|---|---|
| Public proof package | complete | The report links the public docs, evidence JSON, replay fixtures, examples and validation commands. |
| Public claim | partial | Several categories are intentionally `partial`, `baseline_created`, `blocked` or `not_measured`. |
| Paneflow replacement readiness | no | The dogfood proof is targeted shadow smoke, not broad real-session replacement evidence. |
| M5 recommendation | compatibility and release hardening | M5 should close compatibility, package and platform gaps before host replacement experiments. |

## Current Evidence

| Area | Status | Evidence |
|---|---|---|
| M3 PRD status | done | `tasks/prd-m3-paneflow-dogfood-harness-status.json` has `prd.status = DONE`. |
| M3 historical report | partial proof | `docs/m3-paneflow-dogfood-report.md` says checked-in artifacts are synthetic or scrubbed derivatives, so real 10k-line performance remains blocked. |
| Paneflow dogfood smoke | pass | `evidence/m4/paneflow-dogfood-smoke-2026-07-04.json` records 1 passing Hera shadow test and 0 mismatch reports. |
| M4 artifact contract | ready | `docs/m4-evidence-contract.md` defines layout, metadata, redaction and reproducibility rules. |
| Machine manifest | ready | `evidence/m4/evidence-manifest.json` is validated by `terminal-cli validate-m4-evidence`. |
| Compatibility matrix | partial proof | `evidence/m4/compatibility-matrix.json` and `docs/m4-compatibility-matrix.md` list fixture-backed rows, platform measurements and explicit gaps. |
| Benchmark harness | baseline created | `crates/terminal-cli/benches/m4_benchmarks.rs` uses Criterion; `evidence/m4/m4-benchmark-summary.json` records ingest, snapshot, replay and comparison metrics. |
| Memory profiles | pass | `evidence/m4/m4-memory-profile.json` covers 10k, 100k and 1M logical lines within the configured Hera-owned byte budget. |
| Threshold report | baseline created | `evidence/m4/m4-performance-report.json` and `docs/m4-benchmarks-and-memory.md` evaluate benchmark and memory metrics against `evidence/m4/m4-performance-thresholds.json`. |
| Replay corpus | pass | `crates/terminal-fixtures/fixtures/m4-replay/*.json` replay twice with identical final snapshot hashes and event counts. |
| Public event stream | ready | `evidence/m4/replay-event-streams/basic-shell.jsonl` is exported from the public replay corpus with a versioned Hera header. |
| Paneflow public-safe demo | documented | `docs/m4-replay-and-dogfood-demo.md` documents the shadow-mode command sequence and raw local retention policy. |
| Public API examples | pass | `crates/terminal-core/examples/headless_embedder.rs` and `crates/terminal-pty/examples/pty_boundary.rs` compile under `cargo test --workspace`. |
| API audit | partial | `evidence/m4/m4-api-audit.json` records no P0 boundary leak and open release blockers for metadata and publish order. |
| Package/docs readiness | partial | `cargo doc --workspace --no-deps` passes; package dry-runs pass for leaf crates and block on unpublished internal dependencies for dependent crates. |
| OSS security baseline | partial | `evidence/m4/m4-oss-security-baseline.json` records local checks and marks external OpenSSF Scorecard as not measured. |

## Compatibility Matrix

EP-002 adds the first public compatibility matrix. It is deliberately narrow:
fixture-backed rows cover C0 cursor controls, primary scrollback, SGR reset and
truecolor, alternate screen 1049, primary resize reflow, alternate resize
preservation and bracketed paste mode. Windows is marked `pass` for those rows
because the checked-in fixture suite was validated locally; Linux and macOS stay
`not_measured`.

The same matrix calls out gaps instead of hiding them: CSI cursor positioning,
ED/EL/ECH screen clearing, DEC private modes 47/1047/1048 and Sixel/image
protocols are not public pass claims.

## Paneflow Smoke

Command run from the Paneflow repo:

```text
PANEFLOW_HERA_TERMINAL_ROOT=<hera-repo>
PANEFLOW_HERA_DOGFOOD=shadow
PANEFLOW_HERA_DOGFOOD_ARTIFACT_DIR=<paneflow-repo>/.paneflow-audit/hera-dogfood/m4-smoke-2026-07-04-ep001
PANEFLOW_HERA_DOGFOOD_RETENTION=scrubbed
cargo test -p paneflow-app --features hera-dogfood startup_token_checkpoint_compares_after_shadow_drain
```

Result: 1 test passed, 0 failed, 0 mismatch report files under the configured
artifact directory. An earlier exact-filter attempt compiled the same feature
path but ran 0 tests; it is excluded from the pass claim.

## Historical Blockers Kept Visible

These are not current EP-001 failures. They are the M3 report's known limits
that M4 keeps visible in the final recommendation.

| Blocker | Current treatment |
|---|---|
| Real Codex and Claude Code 10k-line captures are not checked in. | M5 must add scrubbed public derivatives or explicit blocked evidence before replacement claims. |
| RSS and latency values are `not_measured` in checked-in M3 summaries. | EP-003 adds M4-specific benchmark and memory evidence; M5 still needs public-safe real-session derivatives. |
| Side-by-side render sampling still needs live rapid-output coverage. | EP-004 documents a public-safe procedure and targeted pass; broader rapid-output coverage remains M5 work. |
| Synthetic fixtures cannot justify terminal replacement. | M5 stays evidence-driven, not replacement-first. |

## Artifact Map

Public M4 artifacts use repo-relative paths:

| Path | Purpose |
|---|---|
| `docs/m4-public-proof-report.md` | Human report, final M4 verdict and M5 recommendation. |
| `docs/m4-evidence-contract.md` | Artifact layout, manifest fields, redaction rules and validation commands. |
| `docs/m4-compatibility-matrix.md` | Human-readable compatibility matrix with known gaps and platform measurements. |
| `docs/m4-benchmarks-and-memory.md` | Human-readable EP-003 benchmark, memory and threshold report. |
| `docs/m4-replay-and-dogfood-demo.md` | Human-readable EP-004 replay corpus, event stream and Paneflow demo procedure. |
| `docs/m4-api-and-package-readiness.md` | Human-readable EP-005 public API, package and OSS security readiness report. |
| `evidence/m4/evidence-manifest.json` | Machine-readable artifact registry. |
| `evidence/m4/compatibility-matrix.json` | Machine-readable compatibility matrix. |
| `evidence/m4/m4-benchmark-summary.json` | Machine-readable benchmark summary for ingest, snapshot, replay and comparison. |
| `evidence/m4/m4-memory-profile.json` | Machine-readable 10k, 100k and 1M logical-line memory profiles. |
| `evidence/m4/m4-performance-thresholds.json` | Threshold policy for M4 performance and memory evidence. |
| `evidence/m4/m4-performance-report.json` | Machine-readable threshold evaluation and EP-003 rollup. |
| `evidence/m4/m4-replay-verification.json` | Machine-readable deterministic replay verification for the EP-004 corpus. |
| `evidence/m4/replay-event-streams/basic-shell.jsonl` | Public Hera-specific event stream exported from `basic-shell.json`. |
| `evidence/m4/paneflow-dogfood-smoke-2026-07-04.json` | Public summary of the targeted Paneflow dogfood smoke. |
| `evidence/m4/paneflow-dogfood-demo-2026-07-04-ep004.json` | Public EP-004 Paneflow feature gate and shadow smoke summary. |
| `evidence/m4/m4-api-audit.json` | Machine-readable API audit and release blockers. |
| `evidence/m4/m4-package-readiness.json` | Machine-readable package dry-run and docs readiness evidence. |
| `evidence/m4/m4-oss-security-baseline.json` | Machine-readable local security baseline and OpenSSF gap. |
| `crates/terminal-core/examples/headless_embedder.rs` | Public headless embedder example. |
| `crates/terminal-pty/examples/pty_boundary.rs` | Public PTY boundary example. |

Raw local captures stay outside checked-in public artifacts. The default local
dogfood directory is under `<paneflow-repo>/.paneflow-audit/`.

Local-only paths in this report are intentionally written as placeholders such
as `<paneflow-repo>` and `<hera-repo>`. They are not public links and are not
claimed as checked-in artifacts.

## Replay And Demo Artifacts

EP-004 adds a scrubbed public replay boundary. The checked-in corpus is synthetic
or scrubbed-public only, validates redaction metadata and rejects local home
paths, secret markers, unsupported event kinds and raw local classification.

Run:

```text
cargo run -p terminal-cli -- verify-m4-replay crates/terminal-fixtures/fixtures/m4-replay --json-output evidence/m4/m4-replay-verification.json
cargo run -p terminal-cli -- export-m4-event-stream crates/terminal-fixtures/fixtures/m4-replay/basic-shell.json --output evidence/m4/replay-event-streams/basic-shell.jsonl
```

The event stream borrows asciicast v2's newline-delimited shape but remains a
Hera-specific format. It emits a versioned header and public event objects; it
does not emit private input events and is not an asciicast-compatible file.

The Paneflow dogfood procedure is documented in
`docs/m4-replay-and-dogfood-demo.md`. It keeps `PANEFLOW_HERA_DOGFOOD=shadow`,
stores artifacts under the Paneflow audit directory, treats zero mismatch files
as the pass condition and keeps raw local retention out of Hera. The EP-004
summary records `cargo check --workspace --features hera-dogfood`,
`cargo test --workspace --features hera-dogfood`, and the targeted shadow smoke
as passing.

## Public API And Readiness

EP-005 adds two compiling examples:

- `crates/terminal-core/examples/headless_embedder.rs` shows byte ingestion,
  resize and render snapshot from the headless API without PTY, Paneflow,
  renderer or platform dependencies.
- `crates/terminal-pty/examples/pty_boundary.rs` shows the runtime boundary:
  `terminal-pty` spawns the PTY and forwards output into `terminal-core` before
  reading a renderer-neutral snapshot.

The API audit found no P0 boundary leak. Existing tests keep `vte` private to
`terminal-core`, keep PTY/runtime dependencies outside `terminal-core`, and keep
`portable-pty` types out of the public `terminal-pty` API.

Readiness is still partial. Cargo docs render locally, and
`terminal-protocol` plus `terminal-render-model` package successfully. Crates
that depend on unpublished Hera crates block during package preparation because
Cargo resolves packaged path dependencies through crates.io. Cargo also warns
that package metadata lacks description, license, documentation, homepage or
repository fields. The workspace still has `publish = false`, and M4 did not run
`cargo publish`.

The OSS baseline is local-only: locked metadata resolves, no binary artifacts are
present outside `target`, and the high-confidence credential scan only matched
redaction-policy marker literals in test policy code. `cargo tree --locked
--duplicates` reports indirect `bitflags` 1.x and 2.x through `portable-pty` and
`nix`. OpenSSF Scorecard is referenced as an external posture check, but no
Scorecard score is claimed because the CLI is not installed locally.

## M4 Verdict And M5 Recommendation

M4 closes as a public proof package with honest partial status. Hera can now be
evaluated from one report, but it should not move to Paneflow replacement or
crate publication yet.

| Category | M4 verdict | Evidence |
|---|---|---|
| Compatibility matrix | partial | `docs/m4-compatibility-matrix.md` and `evidence/m4/compatibility-matrix.json` show fixture-backed rows plus visible gaps. |
| Benchmarks | baseline created | `evidence/m4/m4-benchmark-summary.json` and `evidence/m4/m4-performance-report.json` create latency baselines without hard pass claims. |
| Memory profiles | pass | `evidence/m4/m4-memory-profile.json` covers 10k, 100k and 1M logical lines within the Hera-owned byte budget. |
| Replay demo corpus | pass | `evidence/m4/m4-replay-verification.json` records deterministic replay for all public M4 fixtures. |
| Paneflow demo | targeted pass | `evidence/m4/paneflow-dogfood-demo-2026-07-04-ep004.json` records feature gates and a shadow smoke with zero mismatches. |
| Public API examples | pass | `cargo test --workspace` compiles the headless embedder and PTY boundary examples. |
| Package readiness | partial | Leaf crates package, dependent crates block on unpublished internal dependencies, and metadata is incomplete. |
| OSS security baseline | partial | Local checks pass with gaps; external OpenSSF Scorecard is not measured. |

Recommended M5: compatibility and release hardening. The concrete chantier is to
turn M4's partial public proof into adoption-grade confidence by adding broader
fixture coverage, real-session scrubbed replay derivatives, Linux/macOS runtime
measurements, package metadata, internal crate publish ordering and an external
security posture check.

M5 should not replace Paneflow's terminal path by default. The threshold to
unlock host replacement experiments is: implemented or explicitly deferred CSI
positioning and ED/EL/ECH behavior, DEC private mode gap resolution or documented
compatibility policy, replay coverage from scrubbed real Codex and Claude Code
sessions, Linux and macOS measurement rows, all intended public crates packaging
cleanly, and Paneflow shadow dogfood producing no P0 mismatch evidence across a
longer rapid-output scenario.

Every claim above is backed by one of: a command in the validation section, a
machine artifact under `evidence/m4/`, a checked-in fixture under
`crates/terminal-fixtures/fixtures/`, a compiling example, or an explicit source
reference in the linked M4 docs.

## Validation

Run:

```text
cargo test -p terminal-fixtures m4_evidence
cargo test -p terminal-fixtures m4_compatibility
cargo test -p terminal-fixtures m4_performance
cargo test -p terminal-fixtures m4_replay
cargo test -p terminal-cli m4_performance_cli
cargo test -p terminal-cli m4_replay
cargo test -p terminal-cli m4_event_stream
cargo run -p terminal-cli -- validate-m4-evidence evidence/m4/evidence-manifest.json
cargo run -p terminal-cli -- validate-m4-compatibility evidence/m4/compatibility-matrix.json
cargo run -p terminal-cli -- verify-m4-replay crates/terminal-fixtures/fixtures/m4-replay --json-output evidence/m4/m4-replay-verification.json
cargo run -p terminal-cli -- export-m4-event-stream crates/terminal-fixtures/fixtures/m4-replay/basic-shell.json --output evidence/m4/replay-event-streams/basic-shell.jsonl
cargo run -p terminal-cli --release -- m4-benchmark --output evidence/m4/m4-benchmark-summary.json
cargo run -p terminal-cli --release -- m4-memory-profile --output evidence/m4/m4-memory-profile.json
cargo run -p terminal-cli --release -- m4-performance-report --bench evidence/m4/m4-benchmark-summary.json --memory evidence/m4/m4-memory-profile.json --thresholds evidence/m4/m4-performance-thresholds.json --json-output evidence/m4/m4-performance-report.json --markdown-output docs/m4-benchmarks-and-memory.md
cargo test --workspace
cargo doc --workspace --no-deps
cargo package -p terminal-protocol --allow-dirty --no-verify
cargo package -p terminal-render-model --allow-dirty --no-verify
```

The validator rejects stale redaction checks, `raw_local` manifest entries,
absolute artifact paths and public JSON dogfood or replay artifacts containing
raw transcript fields.

The compatibility validator rejects malformed rows, missing platform fields,
implemented rows without fixture or replay coverage, and fixture links whose
names do not exist in the checked-in fixture pack.

The performance report marks timing metrics as `baseline_created` until a
maintainer accepts regression thresholds. Memory scenarios are hard-gated
against the configured Hera-owned byte budget; the 10k, 100k and 1M runs passed
on the Windows release-profile baseline recorded in `evidence/m4/`.
