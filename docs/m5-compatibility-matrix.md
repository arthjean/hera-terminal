# M5 Compatibility Matrix

Status: EP-002 fixture-backed pass
Date: 2026-07-04

The M5 compatibility matrix is `evidence/m5/compatibility-matrix.json`. It
expands the M4 compatibility surface from broad gaps into explicit M5 rows for
cursor positioning, erasure semantics and DEC private alternate-screen modes.

## Summary

| Metric | Count |
|---|---:|
| Total rows | 19 |
| Pass rows | 18 |
| Failed rows | 0 |
| Deferred rows | 0 |
| Not implemented rows | 0 |
| Out-of-scope rows | 1 |
| Measured rows | 18 |
| Fixture-backed pass rate | 100% |

The out-of-scope row is `xterm.dcs.sixel_images`. It is tracked so the matrix
does not silently imply image protocol support, but it is excluded from the pass
percentage and M5 measured denominator.

## EP-002 Rows

| Behavior | Disposition | Fixture evidence |
|---|---|---|
| `vt.cursor.csi_positioning` | pass | `csi-cup-hvp-positioning` |
| `vt.cursor.csi_positioning_defaults_and_bounds` | pass | `csi-positioning-defaults-and-bounds` |
| `vt.screen.ed` | pass | `ed-mode-0`, `ed-mode-1`, `ed-mode-2` |
| `vt.screen.el` | pass | `el-mode-0`, `el-mode-1`, `el-mode-2` |
| `vt.screen.ech` | pass | `ech-clears-without-shift` |
| `vt.screen.erasure_unsupported_params` | pass | `unsupported-erasure-parameter-preserves-state` |
| `xterm.private_modes.47` | pass | `dec-private-47-switches-screen` |
| `xterm.private_modes.1047` | pass | `dec-private-1047-clears-alternate-on-reset` |
| `xterm.private_modes.1048` | pass | `dec-private-1048-save-restore-cursor` |
| `xterm.private_modes.47_1047_1048` | pass | DEC private mode fixture group |

All pass rows reference checked-in fixtures and source references. Core CSI,
erasure and DEC private mode references point to xterm control sequences:
https://www.xfree86.org/current/ctlseqs.html.

## Deferred Rows

No EP-002 row is deferred. If a future row is deferred, the matrix validator
requires a reason, owner and M6 follow-up, and `measured_count` excludes it from
the pass denominator.

## Validation

Run:

```text
cargo run -p terminal-cli -- replay crates/terminal-fixtures/fixtures/m5-compatibility.json
cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json
```
