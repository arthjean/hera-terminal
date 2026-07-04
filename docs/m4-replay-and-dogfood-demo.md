# M4 Replay And Dogfood Demo

Status: EP-004 public replay and dogfood proof
Date: 2026-07-04

## Replay Corpus

The public replay corpus lives under
`crates/terminal-fixtures/fixtures/m4-replay/`.

| Fixture | Purpose | Privacy |
|---|---|---|
| `basic-shell.json` | Synthetic command output with a marker event. | `synthetic_public` |
| `resize-and-wrap.json` | Synthetic resize plus wrapped output. | `synthetic_public` |
| `alternate-screen.json` | Synthetic alternate screen transition. | `synthetic_public` |

Run:

```text
cargo run -p terminal-cli -- verify-m4-replay crates/terminal-fixtures/fixtures/m4-replay --json-output evidence/m4/m4-replay-verification.json
```

The command loads every public replay fixture, validates redaction metadata,
rejects private path and secret patterns, replays each fixture twice, and writes
the final snapshot hash plus event counts. The current generated summary is
`evidence/m4/m4-replay-verification.json`.

The verification report proves deterministic replay for the public corpus:

| Fixture | Events | Output | Resize | Marker | Hash |
|---|---:|---:|---:|---:|---|
| `alternate-screen` | 4 | 3 | 0 | 1 | `fnv1a64:547f00e8bba7ee3b` |
| `basic-shell` | 4 | 3 | 0 | 1 | `fnv1a64:b71f8eff7558aca2` |
| `resize-and-wrap` | 3 | 2 | 1 | 0 | `fnv1a64:2fca5f8956605cc1` |

The corpus is not a compatibility claim for real private sessions. It is a
public-safe deterministic replay proof that later M4 stories can extend with
scrubbed capture derivatives.

## Public Event Stream Export

Run:

```text
cargo run -p terminal-cli -- export-m4-event-stream crates/terminal-fixtures/fixtures/m4-replay/basic-shell.json --output evidence/m4/replay-event-streams/basic-shell.jsonl
```

The stream is newline-delimited JSON with a versioned header on the first line
and one Hera event object per following line. It borrows the same broad shape as
asciicast v2: header first, then event stream. It is intentionally Hera-specific:

| Topic | Hera EP-004 stream |
|---|---|
| Header | JSON object with `schema = hera.m4_event_stream`, version, fixture id and terminal size. |
| Event line | JSON object with `kind`, `time_ms`, public data and privacy class. |
| Output data | Public synthetic or scrubbed text only. |
| Input events | Not emitted by the public exporter. |
| Compatibility | Not an asciicast v2 file. Do not feed it to an asciicast player. |

Public export refuses fixtures that are not `synthetic_public` or
`scrubbed_public`. Unknown future event kinds fail with a clear unsupported-event
error instead of being silently converted into misleading evidence.

Reference: <https://docs.asciinema.org/manual/asciicast/v2/>.

## Paneflow Dogfood Demo

The public-safe dogfood procedure is run from the Paneflow repository. Hera
stays in shadow mode: Paneflow's current terminal path remains authoritative,
and Hera records comparison evidence only.

PowerShell sequence:

```text
Set-Location <paneflow-repo>
$env:PANEFLOW_HERA_TERMINAL_ROOT = '<hera-repo>'
$env:PANEFLOW_HERA_DOGFOOD = 'shadow'
$env:PANEFLOW_HERA_DOGFOOD_ARTIFACT_DIR = '<paneflow-repo>/.paneflow-audit/hera-dogfood/m4-public-demo'
$env:PANEFLOW_HERA_DOGFOOD_RETENTION = 'scrubbed'
cargo test -p paneflow-app --features hera-dogfood startup_token_checkpoint_compares_after_shadow_drain
```

Pass condition:

- The test command exits 0.
- No mismatch report files are written.
- If the artifact directory is not created, record that as `0` mismatch files
  rather than a missing proof file.
- Any raw local captures remain under the Paneflow audit directory and are not
  copied into Hera.

Failure or partial condition:

- If mismatch report files are produced, keep raw artifacts local.
- Copy only scrubbed summaries into Hera.
- Mark Paneflow proof as `failed` or `partial` in the M4 report and link the
  sanitized summary.

Current public evidence:
`evidence/m4/paneflow-dogfood-smoke-2026-07-04.json` records a targeted Hera
shadow smoke with one passing test and zero mismatch report files.
`evidence/m4/paneflow-dogfood-demo-2026-07-04-ep004.json` records the EP-004
feature-gated Paneflow check, workspace test and targeted shadow smoke. This
document turns that local pattern into the public M4 demo procedure; it does not
replace the need for broader real-session dogfood before any M5 replacement
decision.
