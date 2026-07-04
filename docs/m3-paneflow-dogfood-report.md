# M3 Paneflow Dogfood Go/No-Go Report

Date: 2026-07-04
Decision: continue dogfood

## Recommendation

Do not replace Paneflow's authoritative Alacritty terminal path yet.

M3 now has the evidence plumbing required for local dogfood: Paneflow can emit
Hera recordings and a paired metrics summary under the configured local artifact
directory, Hera can replay checked-in scrubbed M3 recordings, and the fixture
suite validates scrubbed long-session summaries for Codex CLI and Claude Code.

The replacement path remains blocked because the checked-in artifacts are
synthetic or scrubbed derivatives. They prove schema, caps and replay mechanics,
not real 10k-line performance on Arthur's Paneflow sessions.

## Evidence

Local raw artifacts are produced only when Paneflow is compiled with
`hera-dogfood`, dogfood mode is enabled, and `PANEFLOW_HERA_DOGFOOD_ARTIFACT_DIR`
points to a local directory. Raw terminal bytes, prompts and paths stay local.
Checked-in files use scrubbed or synthetic content only.

Checked-in M3 evidence:

| Artifact | Type | Status |
|---|---|---|
| `crates/terminal-fixtures/fixtures/m3-dogfood/synthetic-shadow.json` | scrubbed replay recording | replayed deterministically |
| `crates/terminal-fixtures/fixtures/m3-dogfood/codex-long-session-summary.json` | scrubbed Codex CLI long-session summary derivative | validates 10k-line metric schema |
| `crates/terminal-fixtures/fixtures/m3-dogfood/claude-code-long-session-summary.json` | scrubbed Claude Code long-session summary derivative | validates 10k-line metric schema and truncation reporting |

## Metrics

| Metric | Current Evidence | M3 Target | Result |
|---|---:|---:|---|
| Codex logical output lines | 10,000 in scrubbed derivative | >=10,000 | schema pass |
| Claude Code logical output lines | 10,000 in scrubbed derivative | >=10,000 | schema pass |
| Recording cap behavior | Claude derivative marks `output_truncated=true` at 64 MB cap | explicit truncation | pass |
| Paneflow RSS baseline | not measured in checked-in derivative | measured local value | blocked |
| Dogfood RSS delta | not measured in checked-in derivative | <=64 MB at 10k lines | blocked |
| PTY batch P50/P95/P99 | not measured in checked-in derivative | P95 <=2 ms | blocked |
| Diff rate | counters present in metrics schema | total mismatch count reported | schema pass |

When metrics collection fails or is intentionally omitted from a checked-in
derivative, the summary marks the field `not_measured` with OS, command and
reason. No report value is fabricated.

## Gaps

Compatibility gaps:

- Real Codex CLI and Claude Code 10k-line sessions still need local capture from
  Paneflow, not only synthetic summaries.
- P0 mismatch classification is not complete until the real captures produce
  diff reports with explained or waived fields.
- Windows/macOS/Linux RSS collection parity is not complete. Windows and macOS
  are currently explicit `not_measured` in checked-in derivatives.

Replay gaps:

- Checked-in replay covers a scrubbed synthetic M3 recording, not a reduced real
  Codex or Claude recording.
- Long-session summaries validate metrics shape, but they do not replay raw
  10k-line output in the public corpus.

Render gaps:

- Hera layout goldens prove deterministic window-free mapping, but side-by-side
  live rendering still needs real rapid-output sampling with
  `PANEFLOW_LATENCY_PROBE=1`.
- Unsupported fields and image placeholders remain diagnostics, not parity.

## Required M4 Work

Recommended next PRD scope: continue dogfood hardening before replacement work.

Files and zones:

- `C:\dev\paneflow\src-app\src\terminal\hera_dogfood\mod.rs`: capture real
  long-session metrics, improve RSS support, classify mismatch blockers.
- `C:\dev\paneflow\src-app\src\terminal\pty_session.rs`: verify checkpoint
  cadence under high-output Codex and Claude sessions.
- `C:\dev\paneflow\src-app\src\terminal\view.rs`: sample side-by-side render
  outcomes under `PANEFLOW_LATENCY_PROBE=1`.
- `C:\dev\hera-terminal\crates\terminal-fixtures\fixtures\m3-dogfood\`: add
  scrubbed derivatives from real local captures only after private text is
  removed.
- `C:\dev\hera-terminal\docs\m3-paneflow-dogfood-report.md`: refresh numbers
  after real captures.

Risk areas:

- Private prompt/path leakage in raw recordings.
- False-positive mismatch reports from unsupported fields.
- RSS measurement drift across OSes.
- Hera shadow backpressure under rapid agent output.
- Treating passing synthetic fixtures as production parity.
