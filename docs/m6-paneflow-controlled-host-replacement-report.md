# M6 Paneflow Controlled Host Replacement Report

Status: replacement experiment blocked

## Scope

M6 tested one controlled boundary: selected Paneflow panes used Hera as the visible render authority while the existing PTY, input-mode logic and synchronized Alacritty control remained in place. The default engine was not changed, Alacritty was not removed, and no crate publication was attempted.

The measured Hera revision is `66c9229a`. The Paneflow worktree is based on `4129f8ac` on `feat/hera-m6-host-replacement`, with the M6 implementation present as a worktree diff.

## Default path

The default Paneflow path passed check, Clippy and workspace tests without selecting Hera. The additive `hera-host` path also passed check, Clippy, workspace tests and the terminal golden selection on Windows.

| Gate | Result | Duration |
|---|---:|---:|
| `cargo check --workspace` | PASS | 25.550 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | 8.690 s |
| `cargo test --workspace` | PASS | 40.300 s |
| `cargo check --workspace --features hera-host` | PASS | 12.467 s |
| `cargo clippy --workspace --all-targets --features hera-host -- -D warnings` | PASS | 13.091 s |
| `cargo test --workspace --features hera-host` | PASS | 50.430 s |
| `cargo test -p paneflow-app --features hera-host terminal::` | PASS | 5.311 s |

## Windows visible canary

The visible run completed 3,719 seconds with two concurrent panes. It exercised PowerShell Unicode and styled output, rapid output, a 100,000-line stream, alternate-screen enter and exit, Codex CLI and Claude Code startup. No Paneflow process crash, blank frame, lost input or orphan direct child was observed. Claude Code reached its local login requirement, so its real agent turn remains blocked.

The canary failed the authority thresholds:

| Metric | Result | Threshold |
|---|---:|---:|
| P0 mismatches | 219 | 0 |
| Unsupported checkpoints | 1 | 0 silently skipped |
| Fallbacks | 1 | 0 |
| Dropped Hera bytes | 1,526,241 | 0 |
| Loaded-pane ingestion plus adaptation P95 | 0.0124 ms | at most 2.0 ms |
| Loaded-pane latency samples | 4,096 | at least 100 |
| Maximum observed process RSS | 209,739,776 bytes | paired comparison required |

The rapid-output pane reached its final expected line, but the bounded Hera queue overflowed before render drain caught up. The pane then performed the intended one-way fallback to Alacritty without losing the process. That proves rollback safety for this failure, but it prohibits a canary pass. The paired Alacritty memory run was not performed after the hard-failure conditions had already prohibited advancement.

## Interaction coverage

| Surface | Status | Evidence |
|---|---|---|
| Visible Hera render | PASS | Both selected panes painted through the normal GPUI terminal element before fallback. |
| PowerShell Unicode and style | PASS | Unicode and styled output remained visible. |
| 100,000-line rapid output | FAIL | Queue drop, P0 mismatches and one fallback recorded. |
| Alternate screen | PASS | Alternate content appeared and the primary screen restored. |
| Codex CLI | PASS | A bounded no-tool turn completed in-pane. |
| Claude Code | BLOCKED | The installed CLI required local login. |
| Process cleanup | PASS | No direct Paneflow child remained after shutdown. |

## Platform rows

| Platform | Default gates | `hera-host` gates | Visual result | Status |
|---|---|---|---|---|
| Windows x86_64 MSVC | PASS | PASS | Failed canary thresholds | FAILED |
| Linux x86_64 GNU | Not run | Not run | No runner | BLOCKED |
| macOS Apple Silicon | Not run | Not run | No runner | BLOCKED |

Linux and macOS are not inferred from Windows. The worktree diff is not published to a branch that a hosted runner can check out, and no local Linux distribution or macOS runner was available in this run.

## Decision

The selected outcome is `replacement_experiment_blocked`.

`broader_hera_canary` is prohibited by P0 mismatches, a fallback, dropped output, an unsupported comparison checkpoint, missing paired memory evidence and missing Linux/macOS measurements. `targeted_host_hardening` is also premature because the Windows canary itself did not pass.

The next work is specified as EP-007 in the M6 PRD. It stays inside the controlled host experiment: classify the comparison fields behind the 219 mismatches, make checkpoints sequence-coherent, replace the render-coupled 64-batch drop path with bounded lossless ingestion, pass deterministic and 10-minute qualification gates, then repeat the same Windows canary before spending runner time on cross-platform expansion.

## EP-007 mismatch classification

The scrubbed first-canary artifacts preserve aggregate counters but no mismatch field paths. The classification therefore keeps all 219 P0 mismatches as `unclassified_from_public_evidence`; the one unsupported checkpoint remains separate with the same classification. It does not infer a semantic cause from queue loss or scheduler timing. The machine-readable record is `evidence/m6/m6-mismatch-classification.json`.

The deterministic stale-checkpoint fixture proves the legacy render-cadence failure mode with `$.viewport_lines[0]` as the stable first differing field. It is a reproducer for the removed mechanism, not retrospective attribution. New authoritative mismatch reports contain only a pane pseudonym, field path, bounded outcome class, applied PTY sequence, resize generation, active-screen class and timing metadata.

The EP-007 deterministic admission package now passes. Comparison, authoritative-ingest and PTY lifecycle filters each selected non-zero tests and passed 25 consecutive iterations with zero flaky failures; the one-write, fragmented-read and one-byte 100,000-line variants converged without drops, sequence gaps or leaked workers. Default and `hera-host` workspace suites, Clippy, formatting and the M6 evidence validators also pass.

### EP-007 short smoke blocker on 13 July 2026

The final pre-qualification Windows smoke used run `run-f12675d72506494698a8cd7e756c4aec` and the private local artifact set `smoke-12k-final-f12675d7`. It exercised styled and wide Unicode output, alternate-screen enter and exit, interactive input, more than 12,000 lines in the burst pane, more than 500 lines in the second target pane and visible window resize. No mismatch report existed before resize. Resize generations 2 and 3 then produced these terminal semantic divergences:

| Private report | Field | PTY batch | Resize generation |
| --- | --- | ---: | ---: |
| `hera-dogfood-1783977796841-pane-9ba03cc5c0b708aa-1-report.json` | `$.viewport_lines[0]` | 529 | 2 |
| `hera-dogfood-1783977796904-pane-89d7441ade6231c2-1-report.json` | `$.cursor` | 11821 | 2 |
| `hera-dogfood-1783977853433-pane-9ba03cc5c0b708aa-2-report.json` | `$.scrollback_line_count` | 530 | 3 |

The host metrics aggregate four P0 mismatch checkpoints across the two affected panes. The 532-batch pane recorded three P0 mismatches, 91 publications, 5,904 queue high-water bytes, 0.228 ms P95 and no fallback or dropped byte. The 11,821-batch pane recorded one P0 mismatch, 95 publications, 171,832 queue high-water bytes, 0.095 ms P95, no dropped byte and one fallback classified `hera_resize_coherence_timeout`. Accepted and applied sequences were equal and complete in all three panes. Host pending-coherence ended at zero, while two raw-local dogfood metrics retained one pending sample each. The targeted shells, Paneflow process and all six observed child processes exited cleanly.

This smoke fails the hard zero-mismatch and zero-fallback gate before the formal 10-minute qualification. Per US-022 acceptance, US-022 and EP-007 are `BLOCKED`, US-023 is `TODO`, and neither the 10-minute qualification nor the repeated 60-minute canary and paired Alacritty control was launched. US-012 remains `IN_REVIEW`, US-016 remains `BLOCKED`, the PRD remains `BLOCKED`, and the decision remains `replacement_experiment_blocked`.

The public evidence is machine-readable under `evidence/m6`. It contains counters, timings, pseudonyms, commit identifiers and gate results only. No terminal transcript, raw bytes, screenshot, prompt, user identity, host identity or private path is included.
