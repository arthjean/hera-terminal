# M4 Benchmarks And Memory

Generated: 2026-07-04T12:49:32Z
Status: `baseline_created`

## Benchmark Evidence

Command: `terminal-cli m4-benchmark --output evidence/m4/m4-benchmark-summary.json`

| Metric | Operation | Input bytes | Iterations | ns/iter | Throughput bytes/s | Status |
|---|---:|---:|---:|---:|---:|---|
| `byte_ingest` | byte_ingest | 26890 | 8 | 4300350 | 6252979 | `baseline_created` |
| `snapshot_generation` | snapshot_generation | 26890 | 8 | 6379237 | 4215237 | `baseline_created` |
| `replay` | replay | 26890 | 8 | 104432675 | 257486 | `baseline_created` |
| `snapshot_comparison` | snapshot_comparison | 26890 | 8 | 101247662 | 265586 | `baseline_created` |

## Memory Profiles

Command: `terminal-cli m4-memory-profile --output evidence/m4/m4-memory-profile.json`

| Scenario | Processed lines | Scrollback rows | Discarded rows | Hera-owned bytes | Status |
|---|---:|---:|---:|---:|---|
| `memory_10000_lines` | 10000 / 10000 | 1183 | 8793 | 8385104 | `pass` |
| `memory_100000_lines` | 100000 / 100000 | 1183 | 98793 | 8385104 | `pass` |
| `memory_1000000_lines` | 1000000 / 1000000 | 1183 | 998793 | 8385104 | `pass` |

## Threshold Evaluation

| Metric | Source | Observed | Status | Notes |
|---|---|---:|---|---|
| `byte_ingest` | benchmark | 4300350 ns/iter | `baseline_created` | No accepted latency baseline exists yet. |
| `snapshot_generation` | benchmark | 6379237 ns/iter | `unstable_excluded` | Snapshot timing is recorded but excluded from hard pass claims until a stable release baseline exists. |
| `replay` | benchmark | 104432675 ns/iter | `baseline_created` | No accepted latency baseline exists yet. |
| `snapshot_comparison` | benchmark | 101247662 ns/iter | `baseline_created` | No accepted latency baseline exists yet. |
| `memory_10000_lines` | memory_profile | 8385104 bytes | `pass` |  |
| `memory_100000_lines` | memory_profile | 8385104 bytes | `pass` |  |
| `memory_1000000_lines` | memory_profile | 8385104 bytes | `pass` |  |

## Public Artifact Policy

Criterion target output is intentionally not listed as public evidence. The public package keeps the stable JSON summaries, this Markdown report and threshold config only.
