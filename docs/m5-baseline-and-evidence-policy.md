# M5 Baseline And Evidence Policy

Status: EP-001 baseline ready
Date: 2026-07-04

M5 starts from the M4 public proof package, not from a fresh audit pass. The
machine-readable baseline is `evidence/m5/m5-baseline.json`; it references
`tasks/prd-m4-public-proof-status.json` with `prd.status = DONE` and does not
modify M4 history.

## Baseline Sources

| Source | Purpose | Missing or malformed behavior |
|---|---|---|
| `docs/m4-public-proof-report.md` | Human M4 verdict and M5 recommendation | Dependency row becomes `missing` or `malformed`; M5 remains `READY`. |
| `tasks/prd-m4-public-proof-status.json` | M4 completion status | Dependency row records the read failure; no M4 status history is edited. |
| `evidence/m4/compatibility-matrix.json` | CSI, erasure and DEC private mode source rows | Compatibility blockers become `blocked_dependency`. |
| `evidence/m4/m4-package-readiness.json` | Metadata and publish-order source evidence | Release blockers become `blocked_dependency`. |
| `evidence/m4/m4-oss-security-baseline.json` | OpenSSF Scorecard gap source evidence | Security blocker becomes `blocked_dependency`. |

The required M5 baseline blockers are:

| Blocker | Follow-up |
|---|---|
| `vt.cursor.csi_positioning` | `US-004` |
| `vt.screen.ed_el_ech` | `US-005` |
| `xterm.private_modes.47_1047_1048` | `US-006` |
| `replay.real_session_derivatives` | `US-008` |
| `platform.linux_macos_measurement` | `US-013` |
| `release.package_metadata` | `US-014` |
| `release.publish_order` | `US-015` |
| `security.openssf_scorecard` | `US-017` |

If one of these gaps is already resolved before a later M5 run, the generator
records `m5_disposition = current_evidence` and links the current M5 artifact
instead of copying stale M4 blocker prose.

## Evidence Manifest Contract

`evidence/m5/evidence-manifest.json` is the public artifact registry for M5
evidence. Every artifact row has:

- `id`
- `type`
- `path`
- `source_command`
- `generated_at`
- `redaction_checked_at`
- `privacy`
- `platform`
- `reproducibility_status`
- `owner`

Public paths must be slash-separated repo-relative paths. Absolute paths and
parent-directory segments fail validation. Public artifacts marked
`public_summary` or `scrubbed_public` are scanned against the M5 redaction
policy. `raw_local` is rejected in the public manifest.

Non-reproducible artifacts are allowed only with `non_reproducible_reason` and
`owner`. This keeps authored reports distinct from command-replayable evidence.

The EP-002 compatibility matrix is a public summary artifact. Its validator
requires behavior ids, source references, fixture coverage, platform measurement
state and disposition for every row. Pass rows must link a checked-in fixture or
replay artifact by name. Deferred rows must carry a reason, owner and M6
follow-up and are excluded from measured pass percentage.

The EP-004 platform runtime artifact is `evidence/m5/platform-runtime-evidence.json`.
Its validator requires Windows, Linux and macOS rows for the full M5 command
set. Platform rows can pass, fail, block or go stale, but Linux/macOS cannot be
inferred from a Windows pass.

## Go/No-Go Thresholds

`evidence/m5/m5-go-no-go-thresholds.json` names exactly three M6 outcomes:

| Outcome | Eligibility |
|---|---|
| Host replacement experiment | No P0 compatibility row is `failed` or `not_implemented`, Paneflow shadow P0 mismatch count is 0, and platform rows are measured or explicitly blocked with command evidence. |
| Public pre-release packaging | Every intended public crate packages or has an accepted blocker, publish order is dependency-safe, and no release-blocking security finding remains. |
| Another compatibility hardening milestone | Chosen when unresolved P0 compatibility, replay, dogfood, platform, package or security blockers remain with owners and follow-up work. |

The validator rejects missing outcomes and ambiguous criteria markers such as
`TBD`, `maybe` or `unclear`.

## Validation

Run:

```text
cargo run -p terminal-cli -- generate-m5-baseline --output evidence/m5/m5-baseline.json
cargo run -p terminal-cli -- validate-m5-baseline evidence/m5/m5-baseline.json
cargo run -p terminal-cli -- validate-m5-evidence evidence/m5/evidence-manifest.json
cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json
cargo run -p terminal-cli -- validate-m5-go-no-go evidence/m5/m5-go-no-go-thresholds.json
cargo run -p terminal-cli -- validate-m5-platform evidence/m5/platform-runtime-evidence.json
```
