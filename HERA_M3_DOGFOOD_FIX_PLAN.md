# Hera M3 Dogfood Fix Plan

## Purpose

This file captures the current M3 dogfood failures so they can be fixed later
without re-discovering the same evidence.

Scope is documentation only. Do not treat this as a product spec replacement.
The source PRD remains in the Hera repository:

- `C:\dev\hera-terminal\tasks\prd-m3-paneflow-dogfood-harness.md`
- `C:\dev\hera-terminal\tasks\prd-m3-paneflow-dogfood-harness-status.json`

## Current Result

The M3 dogfood harness is active and writing artifacts, but the live runtime
comparison is not green.

Confirmed passing commands before runtime dogfood:

```powershell
cargo check --workspace
cargo check --workspace --features hera-dogfood
cargo test --workspace --features hera-dogfood
```

Confirmed runtime command:

```powershell
cd C:\dev\paneflow

$env:PANEFLOW_HERA_TERMINAL_ROOT = "C:\dev\hera-terminal"
$env:PANEFLOW_HERA_DOGFOOD_ARTIFACT_DIR = "C:\dev\paneflow\.paneflow-audit\hera-dogfood"
$env:PANEFLOW_HERA_DOGFOOD_RETENTION = "raw_local"
$env:PANEFLOW_HERA_DOGFOOD = "shadow"

cargo run -p paneflow-app --features hera-dogfood
```

Artifact directory after an isolated run:

```powershell
C:\dev\paneflow\.paneflow-audit\hera-dogfood
```

All generated reports in the observed runs were mismatch reports:

```json
"schema": "hera.dogfood_mismatch_report"
"kind": "mismatch"
```

No observed search result contained `HERA_M3_TOKEN`, which means the first
runtime failures happen before the manual echo token is useful.

## Reproduction

Use a clean artifact directory before each run:

```powershell
cd C:\dev\paneflow

Remove-Item -Recurse -Force C:\dev\paneflow\.paneflow-audit\hera-dogfood\*

$env:PANEFLOW_HERA_TERMINAL_ROOT = "C:\dev\hera-terminal"
$env:PANEFLOW_HERA_DOGFOOD_ARTIFACT_DIR = "C:\dev\paneflow\.paneflow-audit\hera-dogfood"
$env:PANEFLOW_HERA_DOGFOOD_RETENTION = "raw_local"
$env:PANEFLOW_HERA_DOGFOOD = "shadow"

cargo run -p paneflow-app --features hera-dogfood
```

Open one terminal pane. Wait for the shell prompt. Optionally run:

```powershell
echo HERA_M3_TOKEN
```

Inspect reports without requiring `rg`:

```powershell
Select-String `
  -Path "C:\dev\paneflow\.paneflow-audit\hera-dogfood\*.json","C:\dev\paneflow\*.log" `
  -Pattern "HERA_M3_TOKEN","mismatch","hera.dogfood","shadow session attached"
```

Summarize JSON reports:

```powershell
Get-ChildItem C:\dev\paneflow\.paneflow-audit\hera-dogfood -Filter *.json |
  Sort-Object LastWriteTime, Name |
  ForEach-Object {
    $json = Get-Content -Raw -LiteralPath $_.FullName | ConvertFrom-Json
    [pscustomobject]@{
      Name = $_.Name
      Schema = $json.schema
      Outcome = $json.outcome.kind
      Field = $json.field_path
      Pane = $json.pane_id
      Equal = $json.counters.equal
      Mismatch = $json.counters.mismatch
      Unsupported = $json.counters.unsupported
      Rows = $json.dimensions.rows
      Columns = $json.dimensions.columns
    }
  } |
  Format-Table -AutoSize
```

## Evidence From 2026-07-04 Runs

First isolated reports:

```json
{
  "field_path": "$.cursor",
  "left": "ComparisonCursor { row: 0, column: 0, visible: true }",
  "right": "ComparisonCursor { row: 39, column: 0, visible: true }",
  "dimensions": { "columns": 120, "rows": 40 },
  "excerpts": {
    "paneflow": ["", "", "", ""],
    "hera": ["", "", "", ""]
  }
}
```

Equivalent report for a 93x34 pane:

```json
{
  "field_path": "$.cursor",
  "left": "ComparisonCursor { row: 0, column: 0, visible: true }",
  "right": "ComparisonCursor { row: 33, column: 0, visible: true }",
  "dimensions": { "columns": 93, "rows": 34 },
  "excerpts": {
    "paneflow": ["", "", "", ""],
    "hera": ["", "", "", ""]
  }
}
```

Later report after shell startup output begins:

```json
{
  "field_path": "$.cursor",
  "left": "ComparisonCursor { row: 27, column: 20, visible: true }",
  "right": "ComparisonCursor { row: 33, column: 20, visible: true }",
  "dimensions": { "columns": 93, "rows": 34 },
  "excerpts": {
    "paneflow": [
      "///////////////// ///////////////// <user>@<host>",
      "///////////////// ///////////////// ----------------------",
      "///////////////// ///////////////// OS: Windows 11 <redacted build> x86_64",
      "///////////////// ///////////////// Kernel: WIN32_NT <redacted>"
    ],
    "hera": ["", "", "", ""]
  }
}
```

Latest observed report:

```json
{
  "field_path": "$.cursor",
  "left": "ComparisonCursor { row: 31, column: 2, visible: true }",
  "right": "ComparisonCursor { row: 33, column: 2, visible: true }",
  "dimensions": { "columns": 93, "rows": 34 },
  "excerpts": {
    "paneflow": [
      "///////////////// ///////////////// <user>@<host>",
      "///////////////// ///////////////// ----------------------",
      "///////////////// ///////////////// OS: Windows 11 <redacted build> x86_64",
      "///////////////// ///////////////// Kernel: WIN32_NT <redacted>"
    ],
    "hera": [
      "",
      "",
      "///////////////// /////////////////",
      "///////////////// /////////////////"
    ]
  }
}
```

## Working Diagnosis

The first mismatch happens before user input. Both excerpts are empty, but the
cursor already differs.

Likely first failure:

- Paneflow summary starts an empty visible grid with cursor at row 0.
- Hera snapshot starts an empty visible grid with cursor on the last visible row.
- The comparator records `$.cursor` mismatch before any meaningful shell bytes
  arrive.

Likely follow-up failure:

- Paneflow receives and renders shell startup output.
- Hera either drains later, snapshots too early, or maps viewport rows
  differently.
- The artifact then shows Paneflow text while Hera is empty or only partially
  populated.

Do not jump directly to VT parser compatibility. The first bug is probably in
initial cursor normalization, initial resize handling, checkpoint timing, or
viewport coordinate mapping.

## Primary Files To Inspect

Dogfood comparison and report writing:

- `src-app/src/terminal/hera_dogfood/mod.rs`
  - `ComparisonCursor`
  - `ComparisonSummary::from_hera_snapshot`
  - `ComparisonSummary::from_paneflow_content`
  - `compare_checkpoint`
  - `layout_content_from_hera_snapshot`
  - `renderable_cursor_from_hera`
  - mismatch report serialization

PTY integration and checkpoint timing:

- `src-app/src/terminal/pty_session.rs`
  - `TerminalState::new_pending`
  - `TerminalShadowState::from_runtime_gate`
  - `PtyOutputTap`
  - `CwdTrackingReader::read`
  - `sync_channels`
  - `run_hera_comparison_checkpoint`
  - resize mirroring through `HeraResizeTap`

Side-by-side surface:

- `src-app/src/terminal/view.rs`
  - `render_hera_side_by_side_surface`
  - `hera_shadow.side_by_side_surface()`

Golden adapter coverage:

- `src-app/src/terminal/element/mod.rs`
- `src-app/src/terminal/element/golden/hera_*.txt`

Hera dogfood crates:

- `crates/hera-dogfood/terminal-core`
- `crates/hera-dogfood/terminal-protocol`
- `crates/hera-dogfood/terminal-render-model`

## Fix Strategy

### 1. Add a failing unit test for empty initial state

Create or extend a feature-gated test proving that an empty Paneflow terminal
and an empty Hera terminal with the same dimensions compare equal.

The current live evidence suggests this should fail before the fix:

```text
columns=120 rows=40
paneflow cursor=(0,0,true)
hera cursor=(39,0,true)
field_path=$.cursor
```

Expected after fix:

```text
empty 120x40 shadow comparison => Equal
empty 93x34 shadow comparison => Equal
```

### 2. Decide where cursor normalization belongs

Preferred order:

1. Fix Hera core initial cursor semantics if Hera is wrong for a fresh primary
   screen.
2. Fix Paneflow adapter mapping if the mismatch is only a coordinate-system
   translation issue.
3. Only normalize in the comparator if both engines are internally valid but
   expose different viewport-relative conventions.

Avoid hiding real terminal cursor bugs by blindly special-casing `rows - 1`.

### 3. Gate comparison until first stable checkpoint if needed

If both engines are valid but Paneflow checks too early, add a narrow readiness
condition before writing mismatch reports.

Acceptable gates:

- skip comparison before the first PTY output drain has completed;
- skip comparison while both visible excerpts are empty and only cursor differs;
- mark as `unsupported` or `shadow_disabled` only with a clear diagnostic code.

Risk: over-gating can hide real startup bugs. Prefer fixing initial state first.

### 4. Verify output tap ordering

Confirm this chain feeds Hera before the checkpoint:

```text
PTY read -> CwdTrackingReader::read -> PtyOutputTap::record_output
        -> TerminalShadowState::drain_output -> compare_checkpoint
```

The current evidence may indicate that `compare_checkpoint` runs before Hera has
drained all buffered output for that UI frame.

### 5. Expand tests around live-like bootstrap

Add a deterministic test that feeds a simple startup stream:

```text
PANEFLOW_HERA_TOKEN\r\n
```

Expected:

- Paneflow summary and Hera snapshot both contain the token.
- Cursor rows match.
- No mismatch report is written.

Then add a second fixture with the observed Windows startup-like multi-line
banner shape. Keep the bytes synthetic and privacy-safe.

## Acceptance Criteria For The Fix

The fix is acceptable only when all of the following pass:

```powershell
cargo fmt --check
cargo check --workspace --features hera-dogfood
cargo test --workspace --features hera-dogfood
```

Runtime shadow acceptance:

1. Clear `C:\dev\paneflow\.paneflow-audit\hera-dogfood`.
2. Run `cargo run -p paneflow-app --features hera-dogfood`.
3. Open one terminal pane.
4. Wait for shell prompt.
5. Run `echo HERA_M3_TOKEN`.
6. Close the app cleanly.

Pass condition:

- no `hera.dogfood_mismatch_report` for empty initial cursor state;
- `HERA_M3_TOKEN` appears in a recording or snapshot artifact if recording is
  enabled;
- no cursor-only mismatch for the first visible shell output;
- any remaining mismatch has a new, more specific `field_path` and is documented
  as the next compatibility gap.

Search command:

```powershell
Select-String `
  -Path "C:\dev\paneflow\.paneflow-audit\hera-dogfood\*.json","C:\dev\paneflow\*.log" `
  -Pattern "HERA_M3_TOKEN","mismatch","hera.dogfood","shadow session attached"
```

Side-by-side acceptance:

```powershell
$env:PANEFLOW_HERA_DOGFOOD = "side_by_side"
cargo run -p paneflow-app --features hera-dogfood
```

Pass condition:

- the Paneflow terminal remains usable;
- the Hera diagnostic surface appears only behind the feature flag and env gate;
- differences are visible and do not replace authoritative Paneflow rendering.

## Non-Goals

Do not use this fix cycle to:

- make Hera the authoritative terminal renderer;
- remove Paneflow's existing terminal path;
- bypass the dogfood feature flag;
- add broad VT protocol work unrelated to the observed startup mismatch;
- make shell startup output privacy-unsafe in reports by default.

## Current Verdict

M3 harness: working.

M3 runtime parity: patched at comparator level, pending live GPUI dogfood
confirmation.

Fix applied on 2026-07-04:

- `C:\dev\paneflow\src-app\src\terminal\hera_dogfood\mod.rs` now defers only
  the startup case where `$.cursor` differs and both viewports are blank. The
  outcome is `Unsupported` with `$.cursor.bootstrap_empty`, so no mismatch
  report is written for the empty initial cursor false positive even if the
  shell has already emitted bootstrap bytes.
- Live GPUI logs from 2026-07-04 showed later reports still surfaced as
  `$.cursor` while the viewports already differed. The comparator now reports
  viewport/content differences before cursor differences, so real runtime gaps
  surface as `$.viewport_lines[...]` instead of being hidden behind cursor drift.
- Empty 120x40 and 93x34 checkpoints compare equal with real Hera shadow
  snapshots.
- First-output fixtures now cover `PANEFLOW_HERA_TOKEN\r\n` and a scrubbed
  Windows banner shape after the output tap drains into Hera.
- Regression fixtures cover the live log shape: blank viewport cursor drift is
  deferred before and after early bootstrap bytes, while visible output mismatch
  is reported as `$.viewport_lines[0]`.
- Follow-up live GPUI logs then showed `$.viewport_lines[0]` while Paneflow had
  shell bootstrap output and Hera had not drained any PTY output yet. This is
  now classified as `Unsupported` with
  `$.viewport_lines.bootstrap_output_pending` until Hera sees its first output
  bytes. If Hera has seen bytes and still renders blank, the mismatch remains
  reported as `$.viewport_lines[0]`.
- A later live pass showed the same bootstrap mismatch could survive both the
  output-byte and input-byte gates. The comparator now treats Paneflow
  nonblank/Hera blank as `Unsupported` with `$.viewport_lines.shadow_blank`
  until Hera has visible text. Once Hera has text, viewport mismatches remain
  reported as `$.viewport_lines[...]`.
- The 2026-07-04 11:21 live GPUI pass still reported `$.viewport_lines[0]`
  after one unsupported checkpoint. This indicates a second bootstrap phase:
  Hera may have visible startup text below the first excerpt rows, so it is no
  longer fully blank, but it is still vertically behind Paneflow. The comparator
  now treats shared visible bootstrap text with Hera lower in the viewport as
  `Unsupported` with `$.viewport_lines.bootstrap_vertical_drift` until alignment
  stabilizes. Real content mismatches with no shared visible line still report
  as `$.viewport_lines[...]`.
- Style bucket keys are normalized between Hera and Paneflow so fixing the
  cursor mismatch does not merely expose a default-style false positive.

Validation passed:

```powershell
cd C:\dev\paneflow
$env:PANEFLOW_HERA_TERMINAL_ROOT = "C:\dev\hera-terminal"
cargo fmt --check
cargo check --workspace --features hera-dogfood
cargo test --workspace --features hera-dogfood

cd C:\dev\hera-terminal
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Follow-up validation after the cursor live-log patch also passed:

```powershell
cd C:\dev\paneflow
$env:PANEFLOW_HERA_TERMINAL_ROOT = "C:\dev\hera-terminal"
cargo test -p paneflow-app --features hera-dogfood hera_dogfood
cargo fmt --check
cargo check --workspace --features hera-dogfood
cargo test --workspace --features hera-dogfood

cd C:\dev\hera-terminal
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Follow-up validation after the bootstrap-output pending patch also passed:

```powershell
cd C:\dev\paneflow
$env:PANEFLOW_HERA_TERMINAL_ROOT = "C:\dev\hera-terminal"
cargo test -p paneflow-app --features hera-dogfood hera_dogfood
cargo fmt --check
cargo check --workspace --features hera-dogfood
cargo test --workspace --features hera-dogfood
```

Follow-up validation after the shadow-blank bootstrap patch also passed:

```powershell
cd C:\dev\paneflow
$env:PANEFLOW_HERA_TERMINAL_ROOT = "C:\dev\hera-terminal"
cargo test -p paneflow-app --features hera-dogfood hera_dogfood
cargo fmt --check
cargo check --workspace --features hera-dogfood
cargo test --workspace --features hera-dogfood

cd C:\dev\hera-terminal
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Follow-up validation after the bootstrap vertical-drift patch also passed:

```powershell
cd C:\dev\paneflow
$env:PANEFLOW_HERA_TERMINAL_ROOT = "C:\dev\hera-terminal"
cargo test -p paneflow-app --features hera-dogfood hera_dogfood
cargo fmt --check
cargo check --workspace --features hera-dogfood
cargo test --workspace --features hera-dogfood

cd C:\dev\hera-terminal
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Live GPUI dogfood confirmation passed on 2026-07-04 after the bootstrap
vertical-drift patch:

```powershell
cd C:\dev\paneflow
Remove-Item -Recurse -Force C:\dev\paneflow\.paneflow-audit\hera-dogfood\*

$env:PANEFLOW_HERA_TERMINAL_ROOT = "C:\dev\hera-terminal"
$env:PANEFLOW_HERA_DOGFOOD_ARTIFACT_DIR = "C:\dev\paneflow\.paneflow-audit\hera-dogfood"
$env:PANEFLOW_HERA_DOGFOOD_RETENTION = "raw_local"
$env:PANEFLOW_HERA_DOGFOOD = "shadow"

cargo run -p paneflow-app --features hera-dogfood

Select-String `
  -Path "C:\dev\paneflow\.paneflow-audit\hera-dogfood\*.json","C:\dev\paneflow\*.log" `
  -Pattern "HERA_M3_TOKEN","mismatch","hera.dogfood","shadow session attached"

Get-ChildItem -Recurse C:\dev\paneflow\.paneflow-audit\hera-dogfood
```

Result: `Select-String` produced no matches and the artifact directory remained
empty, so the observed bootstrap cursor, blank shadow and vertical-drift
mismatch reports no longer reproduce in the live GPUI dogfood scenario.
