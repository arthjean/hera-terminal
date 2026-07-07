# M5 Compatibility And Release Hardening Report

Status: EP-006 final report complete, M5 host replacement and public pre-release packaging blocked
Date: 2026-07-04

M5 converted M4's partial public proof into a broader compatibility, replay,
platform, package, API and security evidence package. The milestone should not
move directly into Paneflow host replacement or crates.io pre-release packaging.
The evidence now says the next chantier is another focused compatibility and
release hardening milestone, with live dogfood, Linux/macOS runners, package
staging, semver tooling and security policy as the blocking work.

## Final Verdict

| Category | M5 status | Evidence |
|---|---|---|
| Baseline and policy | pass | `evidence/m5/m5-baseline.json` references M4 `DONE` status and `evidence/m5/m5-go-no-go-thresholds.json` defines the three M6 outcomes. |
| Compatibility | pass with platform caveat | `evidence/m5/compatibility-matrix.json` has 19 rows: 18 pass rows, 18 measured-on-Windows rows and one out-of-scope Sixel row. |
| Replay | pass | `evidence/m5/m5-replay-verification.json` verifies Codex, Claude Code, rapid-output and 100k-line fixtures twice with deterministic snapshot hashes. |
| Paneflow shadow dogfood | failed | `evidence/m5/paneflow-shadow-dogfood.json` records the 2026-07-07 isolated live GPUI shadow rerun: 28 P0 mismatch reports across style buckets and viewport line alignment. |
| Platform runtime | partial | `evidence/m5/platform-runtime-evidence.json` records Windows pass for the five required commands and blocked Linux/macOS rows with exact rerun commands. |
| Package readiness | blocked | `evidence/m5/m5-package-readiness.json` records metadata/docs.rs pass for all six public crates, but dependent dry-runs fail until upstream Hera crates exist in the registry or a staging strategy is accepted. |
| API hardening | partial | `evidence/m5/m5-api-audit.json` records no P0 boundary leak, but `cargo-semver-checks` is unavailable and public milestone-prefixed names remain an M6 release blocker. |
| Security posture | failed | `evidence/m5/m5-security-baseline.json` records `cargo-audit` blocked, OpenSSF Scorecard blocked and `cargo-deny` failed because no explicit license policy is configured. |

## M6 Recommendation

M6 should be another compatibility and release hardening milestone.

Host replacement experiment is blocked because the 2026-07-07 live Paneflow M5
shadow rerun produced P0 mismatch reports and keeps `replacement_blocked = true`.
Public pre-release packaging is blocked because package dry-runs for dependent
crates fail, semver tooling is unavailable and the security baseline has one
release-blocking finding.

The viable M6 scope is therefore narrow and evidence-driven:

- Fix the live Paneflow shadow mismatches and replace the failed summary with a
  zero-P0 scrubbed run.
- Add Linux and macOS platform evidence through local runners or CI.
- Add explicit cargo-deny policy, rerun cargo-audit or record CI evidence, and
  run OpenSSF Scorecard through CLI or GitHub Action.
- Decide package staging for unpublished Hera crate dependencies before
  treating dry-runs as publication-ready.
- Run semver baseline tooling and either rename or explicitly accept
  milestone-prefixed public constants before any semver-stable promise.

## Evidence Package

The public manifest is `evidence/m5/evidence-manifest.json`. It lists 19 public
artifacts and validates with:

```text
cargo run -p terminal-cli -- validate-m5-evidence evidence/m5/evidence-manifest.json
```

The manifest rejects absolute paths, raw local artifacts, stale redaction
checks, raw transcript fields in public JSON artifacts and non-reproducible rows
without a reason and owner. The security baseline is included as
`m5_security_baseline` and is scanned as a public summary.

## Compatibility Evidence

The compatibility matrix is `evidence/m5/compatibility-matrix.json` and
validates with:

```text
cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json
```

M5 closes the named M4 VT blockers with fixture-backed rows:

| Blocker | M5 result |
|---|---|
| `vt.cursor.csi_positioning` | `CUP` and `HVP` fixture-backed pass, including defaults and bounds policy. |
| `vt.screen.ed_el_ech` | `ED`, `EL` and `ECH` fixture-backed pass, including unsupported parameter preservation. |
| `xterm.private_modes.47_1047_1048` | DEC private modes 47, 1047 and 1048 fixture-backed pass. |

Linux and macOS are still `not_measured` in compatibility rows. That is a
platform evidence blocker, not a hidden compatibility pass.

## Replay And Dogfood Evidence

Replay fixtures live under `crates/terminal-fixtures/fixtures/m5-replay/` and
are regenerated with:

```text
cargo run -p terminal-cli -- generate-m5-replay-derivatives --output-dir crates/terminal-fixtures/fixtures/m5-replay
cargo run -p terminal-cli -- verify-m5-replay crates/terminal-fixtures/fixtures/m5-replay --json-output evidence/m5/m5-replay-verification.json
```

The replay verifier covers Codex, Claude Code, rapid-output and 100k-line
fixtures. The long-session fixture records 100,000 logical lines, Hera-owned
scrollback bytes and discarded rows, so long-session pressure is represented in
public evidence without raw private transcripts.

The dogfood report is `evidence/m5/paneflow-shadow-dogfood.json` and validates
with:

```text
cargo run -p terminal-cli -- validate-m5-dogfood evidence/m5/paneflow-shadow-dogfood.json
```

It is still `failed`: the 2026-07-07 isolated live GPUI shadow rerun restored
five existing terminal sessions, created a two-pane scripted dogfood workspace
and produced 28 P0 mismatch reports before the 45-minute target could be
meaningful. The scrubbed breakdown is 17 `$.style_buckets` reports, 2
`$.viewport_lines[0]` reports and 9 viewport row alignment reports across
`$.viewport_lines[11]` through `$.viewport_lines[19]`. The public scrubbed summary is
`evidence/m5/dogfood/live-gpui-summary-2026-07-07.json`. Raw local terminal
bytes and private mismatch JSON stay outside this repo.

## Platform Evidence

The platform artifact is `evidence/m5/platform-runtime-evidence.json` and
validates with:

```text
cargo run -p terminal-cli -- validate-m5-platform evidence/m5/platform-runtime-evidence.json
```

Windows has pass rows for:

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo doc --workspace --no-deps`
- `cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json`
- `cargo run -p terminal-cli -- verify-m5-replay crates/terminal-fixtures/fixtures/m5-replay --json-output evidence/m5/m5-replay-verification.json`

Linux and macOS rows are blocked with the same required commands and runner
absence as the reason. They are not inferred from Windows.

## Package And API Evidence

Package readiness is generated and validated with:

```text
cargo run -p terminal-cli -- generate-m5-package-readiness --output evidence/m5/m5-package-readiness.json --release-plan-output evidence/m5/m5-release-plan.json
cargo run -p terminal-cli -- validate-m5-package-readiness evidence/m5/m5-package-readiness.json
cargo run -p terminal-cli -- validate-m5-release-plan evidence/m5/m5-release-plan.json
```

`terminal-protocol` and `terminal-render-model` package successfully. The
dependent crates are blocked because Cargo cannot resolve unpublished Hera
dependencies from crates.io during package preparation. The release order is
documented as `terminal-protocol`, `terminal-render-model`, `terminal-core`,
`terminal-fixtures`, `terminal-pty`, then `terminal-cli`. `cargo publish` was
not run.

The API audit is generated and validated with:

```text
cargo run -p terminal-cli -- generate-m5-api-audit --output evidence/m5/m5-api-audit.json
cargo run -p terminal-cli -- validate-m5-api-audit evidence/m5/m5-api-audit.json
```

The audit records no P0 leak of `vte`, `portable-pty`, Paneflow, GPUI, windowing
or platform renderer types across the public Hera boundary. It does not claim
semver stability because `cargo-semver-checks` is unavailable locally.

## Security Evidence

The security baseline is generated and validated with:

```text
cargo run -p terminal-cli -- generate-m5-security-baseline --output evidence/m5/m5-security-baseline.json
cargo run -p terminal-cli -- validate-m5-security-baseline evidence/m5/m5-security-baseline.json
```

Current local result:

| Tool | Status | Release impact |
|---|---|---|
| `cargo audit --json` | blocked | `cargo-audit` is not installed as a Cargo subcommand. |
| `cargo deny check advisories licenses bans sources` | failed | No deny config exists, so license policy rejects dependencies. This is release-blocking until policy is explicit. |
| `scorecard --repo=github.com/arthjean/hera-terminal --format=json` | blocked | OpenSSF Scorecard CLI is not installed locally. |

M5 therefore records a failed security posture. That is useful evidence, not a
reason to hide the baseline.

## Full Validation Set

Run:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
git diff --check
cargo run -p terminal-cli -- replay crates/terminal-fixtures/fixtures/m5-compatibility.json
cargo run -p terminal-cli -- validate-m5-baseline evidence/m5/m5-baseline.json
cargo run -p terminal-cli -- validate-m5-evidence evidence/m5/evidence-manifest.json
cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json
cargo run -p terminal-cli -- validate-m5-go-no-go evidence/m5/m5-go-no-go-thresholds.json
cargo run -p terminal-cli -- verify-m5-replay crates/terminal-fixtures/fixtures/m5-replay --json-output evidence/m5/m5-replay-verification.json
cargo run -p terminal-cli -- validate-m5-dogfood evidence/m5/paneflow-shadow-dogfood.json
cargo run -p terminal-cli -- validate-m5-platform evidence/m5/platform-runtime-evidence.json
cargo run -p terminal-cli -- validate-m5-package-readiness evidence/m5/m5-package-readiness.json
cargo run -p terminal-cli -- validate-m5-release-plan evidence/m5/m5-release-plan.json
cargo run -p terminal-cli -- validate-m5-api-audit evidence/m5/m5-api-audit.json
cargo run -p terminal-cli -- validate-m5-security-baseline evidence/m5/m5-security-baseline.json
```
