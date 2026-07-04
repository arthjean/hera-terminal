# M4 Compatibility Matrix

Status: EP-002 baseline
Generated from: `evidence/m4/compatibility-matrix.json`

## Reading Rules

The matrix separates implementation status from runtime measurement status.
`implemented` means Hera has a checked-in fixture or replay artifact for the
listed behavior. It does not mean full VTTEST or esctest2 coverage.

Windows measurements are local fixture validation results. Linux and macOS stay
`not_measured` until those platforms actually run the same matrix and fixture
commands.

## Summary

| Category | Implemented | Gap or deferred |
|---|---:|---:|
| Cursor movement | 1 | 1 |
| Screen clearing | 0 | 1 |
| Scrolling | 1 | 0 |
| Character attributes | 1 | 0 |
| Alternate screen | 1 | 0 |
| Resize and reflow | 2 | 0 |
| Xterm escape handling | 1 | 2 |

## Matrix

| Row | Category | Status | Fixture coverage | Windows | Linux | macOS | Source |
|---|---|---|---|---|---|---|---|
| `vt.cursor.c0_controls` | cursor movement | implemented | `controls-cr-lf-bs-tab` | pass | not measured | not measured | [VTTEST](https://invisible-island.net/vttest/) |
| `vt.cursor.csi_positioning` | cursor movement | not implemented | none | not applicable | not applicable | not applicable | [xterm control sequences](https://www.xfree86.org/current/ctlseqs.html) |
| `vt.screen.erase_display_line` | screen clearing | not implemented | none | not applicable | not applicable | not applicable | [VTTEST](https://invisible-island.net/vttest/) |
| `vt.scrolling.primary_scrollback` | scrolling | implemented | `scrollback-trim` | pass | not measured | not measured | [VTTEST](https://invisible-island.net/vttest/) |
| `vt.sgr.character_attributes` | character attributes | implemented | `sgr-reset`, `sgr-truecolor` | pass | not measured | not measured | [xterm control sequences](https://www.xfree86.org/current/ctlseqs.html) |
| `vt.screen.alternate_1049` | alternate screen | implemented | `alternate-screen-1049` | pass | not measured | not measured | [xterm control sequences](https://www.xfree86.org/current/ctlseqs.html) |
| `vt.resize.primary_reflow` | resize and reflow | implemented | `resize-primary-wrap-narrow-wide` | pass | not measured | not measured | [VTTEST](https://invisible-island.net/vttest/) |
| `vt.resize.alternate_preserves_primary` | resize and reflow | implemented | `resize-alternate-keeps-primary-scrollback` | pass | not measured | not measured | [VTTEST](https://invisible-island.net/vttest/) |
| `xterm.private_modes.1047_1048` | xterm escape handling | not implemented | none | not applicable | not applicable | not applicable | [xterm control sequences](https://www.xfree86.org/current/ctlseqs.html) |
| `xterm.mode.bracketed_paste` | xterm escape handling | implemented | `bracketed-paste-mode` | pass | not measured | not measured | [xterm control sequences](https://www.xfree86.org/current/ctlseqs.html) |
| `xterm.dcs.sixel_images` | xterm escape handling | out of scope | none | not applicable | not applicable | not applicable | [esctest2](https://github.com/ThomasDickey/esctest2) |

## Known Gaps

| Gap | Treatment |
|---|---|
| CSI cursor positioning | Listed as `not_implemented`; terminal-core currently ignores non-private CSI cursor movement. |
| ED, EL and ECH erasure semantics | Listed as `not_implemented`; no broad screen clearing claim is made. |
| DEC private modes 47, 1047 and 1048 | Listed separately from implemented 1049; unsupported actions are recorded rather than treated as pass. |
| Sixel and image protocols | Listed as `out_of_scope`; M4 does not claim image rendering support. |
| Linux and macOS runtime proof | Listed as `not_measured` for implemented rows; Windows-only local validation is not generalized. |

## Validation

```text
cargo test -p terminal-fixtures m4_compatibility
cargo run -p terminal-cli -- validate-m4-compatibility evidence/m4/compatibility-matrix.json
```

The validator fails if a row omits required schema fields, an `implemented` row
lacks a fixture or replay link, a platform field is missing, or a linked fixture
name cannot be found in the checked-in fixture pack.
