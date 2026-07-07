# M5 Platform Runtime Evidence

Status: Windows measured, Linux and macOS blocked pending runner access
Date: 2026-07-04

EP-004 defines one comparable platform runner for Windows, Linux and macOS:

```text
cargo run -p terminal-cli -- measure-m5-platform --json-output evidence/m5/platform-runtime-evidence.json
```

The runner records the OS label, target triple, rustc version, required command
list, command exit codes, public artifact paths and blocked reasons. The current
required command set is:

| Command id | Command |
|---|---|
| `cargo_check_workspace` | `cargo check --workspace` |
| `cargo_test_workspace` | `cargo test --workspace` |
| `cargo_doc_workspace` | `cargo doc --workspace --no-deps` |
| `validate_m5_compatibility` | `cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json` |
| `verify_m5_replay` | `cargo run -p terminal-cli -- verify-m5-replay crates/terminal-fixtures/fixtures/m5-replay --json-output evidence/m5/m5-replay-verification.json` |

`evidence/m5/platform-runtime-evidence.json` must contain rows for Windows,
Linux and macOS. A row can be `pass`, `failed`, `blocked` or `stale`; a blocked
row must include the exact command and reason. Public artifact paths are
repo-relative slash paths. Absolute paths fail validation.

Current checked-in evidence:

| Platform | Status | Evidence |
|---|---|---|
| Windows | pass | `evidence/m5/platform-runtime-evidence.json` records `x86_64-pc-windows-msvc`, `rustc 1.96.1 (31fca3adb 2026-06-26)` and exit code 0 for all required commands. |
| Linux | blocked | No Linux runner is available in this local session. The row lists the same required commands to rerun on Linux or CI. |
| macOS | blocked | No macOS runner is available in this local session. The row lists the same required commands to rerun on macOS or CI. |

Linux and macOS must not be inferred from Windows. Until the same runner is
executed on those platforms or in CI, their rows remain blocked with command
evidence.

Validation:

```text
cargo run -p terminal-cli -- validate-m5-platform evidence/m5/platform-runtime-evidence.json
```
