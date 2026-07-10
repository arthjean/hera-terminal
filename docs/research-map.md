# Research Map

Status: M6 controlled Paneflow render-authority experiment in progress
Date: 2026-07-10
Scope: architecture research and decision register for `Hera`, not implementation.

## Executive Synthesis

Hera should start as a headless terminal engine, not as a terminal app. The
strongest references converge on one shape: a core object owns parser plus
terminal state, consumes bytes, emits a renderer-neutral snapshot, and exposes
scrollback, viewport, damage, snapshot and replay APIs without knowing the host
renderer or PTY runtime.

The key decision: Hera must own terminal semantics. A parser such as
`alacritty/vte` can tokenize VT input, but it intentionally does not assign
meaning to escape sequences. The state machine, grids, scrollback, modes,
alternate screen, reflow, semantic timeline and snapshots belong in Hera.

Current architecture bias:

1. Wrap `alacritty/vte` first, behind a Hera parser boundary.
2. Model `Terminal { state, parser }`, closer to WezTerm than to app-first
   terminals.
3. Start with an Alacritty/Rio-style grid for simplicity, but design stable row
   IDs and page/chunk storage early so huge scrollbacks do not become a rewrite.
4. Keep PTY outside `terminal-core`. The M2 `terminal-pty` crate owns
   `portable-pty`, process IO, resize, lifecycle and platform quirks.
5. Make render output a snapshot/damage model, not a renderer callback API.
6. Treat semantic session intelligence as an observer sidecar. It can be wrong
   without breaking terminal correctness.
7. Defer image rendering. Parse and preserve metadata first, because Kitty,
   iTerm2 and Sixel are stateful protocols with large compatibility cost.
8. Keep the implementation Rust-public and Rust-first. Other languages are
   references, fixture oracles, or narrow OS FFI boundaries only.

## Reference Inventory

Most initial reference repos were cloned under `C:\dev\terminal-research`.
Dedicated inventory passes use the source path listed in each reference file.

| Reference | What Hera should learn | Caveat |
|---|---|---|
| `alacritty-vte` | Parser boundary, Paul Williams state machine, `Perform` dispatch. | Parser only, no terminal semantics. Detail: [Alacritty VTE inventory](reference-inventory/alacritty-vte.md). |
| `alacritty` | Mature Rust grid, primary/alternate screen, damage/render handoff, fixture tests. | App-oriented, OpenGL/front-end assumptions live nearby. Detail: [Alacritty inventory](reference-inventory/alacritty.md). |
| `wezterm` | Clean `Terminal { state, parser }`, PTY traits, mux/renderable dimensions, ConPTY handling. | Large app and mux surface, not all concepts belong in M1. Detail: [WezTerm inventory](reference-inventory/wezterm.md). |
| `rio` | Rust renderer modernity, Alacritty-derived grid/reflow, damage snapshots, Sugarloaf backends. | More useful for renderer lessons than core terminal originality. Detail: [Rio inventory](reference-inventory/rio.md). |
| `ghostty` | Embeddable terminal API, render snapshots, page-based scrollback, dirty row/cell iteration. | Zig/C API, not directly reusable in Rust M1. Detail: [Ghostty inventory](reference-inventory/ghostty.md). |
| `ghostling` | Minimal host loop around libghostty, render-state iterators, host effects, Raylib drawing. | Great embedding sample, not a Rust engine or cross-platform PTY model. Detail: [Ghostling inventory](reference-inventory/ghostling.md). |
| `kitty` | Advanced protocol scope, especially graphics, keyboard, shell integration and extension tests. | Not a Hera stack model: C/Python/Go/Objective-C, POSIX PTY bias, graphics protocol too broad for M1 rendering. Detail: [Kitty inventory](reference-inventory/kitty.md). |
| `contour` | C++23 terminal layers, ConPTY plus Unix PTY, reflow mode, Sixel/GIP, OSC 133, semantic block query, buffer capture and CSI u. | C++/Qt surface is too broad for Hera core; use as protocol and fixture oracle. Detail: [Contour inventory](reference-inventory/contour.md). |
| `windows-terminal` | ConPTY internals, TextBuffer/ROW, TerminalCore/ControlCore split, renderer invalidation, marks and reflow pain. | Windows-only C++/WinRT product constraints. Use as Windows behavior oracle, not as Hera's core stack. Detail: [Windows Terminal inventory](reference-inventory/windows-terminal.md). |
| `gnome-vte` | Mature GTK VTE widget, parser, ring, rewrap docs, Unix PTY, RingView, OSC 8/133 and Sixel metadata. | GTK/GObject and Unix bias. Use as behavior oracle, not as Hera's core stack. Detail: [GNOME VTE inventory](reference-inventory/gnome-vte.md). |
| `xterm.js` | Ergonomic public API, headless state tracking, parser hooks, buffers, addons, serialization and VS Code-style embedding. | JS/web model, private renderer seams and async parser hook caveats do not map cleanly to Rust. Detail: [xterm.js inventory](reference-inventory/xterm-js.md). |
| `vt100-rust` | Minimal Rust headless parser, `vte::Perform` screen layer, state/diff byte reproduction, JSON fixtures and replay/property tests. | Excellent M1 test/API reference, not final huge-scrollback, reflow, PTY or renderer architecture; AFL harness appears stale. Detail: [vt100-rust inventory](reference-inventory/vt100-rust.md). |
| `vt-push-parser` | Streaming push parser, event allocation discipline, input parser ideas. | Newer and narrower than `alacritty/vte`. |
| `libvterm` | Parser/state/screen callback seams, damage callbacks, reflow fixtures. | C library, FFI shape is instructive but not the target API. |
| `emacs-libvterm` | Real Emacs host embedding of libvterm, native module boundary, PTY process filter, damage redraw, copy mode, OSC 51/52 shell integration. | Emacs/Unix-biased and text-buffer-rendered. Use as host-adapter and trust-boundary reference, not core architecture. Detail: [emacs-libvterm inventory](reference-inventory/emacs-libvterm.md). |

Note: `portable-pty` is not a separate local repo in this research folder. It is
the `pty` crate inside WezTerm. Dedicated pass source: `C:\dev\wezterm\pty`.

Detailed per-reference files:

- [Alacritty VTE](reference-inventory/alacritty-vte.md), focused pass against
  `C:\dev\vte`.
- [Alacritty](reference-inventory/alacritty.md), focused pass against
  `C:\dev\alacritty`.
- [WezTerm](reference-inventory/wezterm.md), focused pass against
  `C:\dev\wezterm`.
- [Rio](reference-inventory/rio.md), focused pass against
  `C:\dev\rio`.
- [Ghostty](reference-inventory/ghostty.md), focused pass against
  `C:\dev\ghostty`.
- [Ghostling](reference-inventory/ghostling.md), focused pass against
  `C:\dev\ghostling`.
- [Kitty](reference-inventory/kitty.md), focused pass against
  `C:\dev\kitty`.
- [Contour](reference-inventory/contour.md), focused pass against
  `C:\dev\contour`.
- [Windows Terminal](reference-inventory/windows-terminal.md), focused pass
  against `C:\dev\terminal`.
- [GNOME VTE](reference-inventory/gnome-vte.md), focused pass against
  `C:\dev\terminal-research\gnome-vte`. Note: `C:\dev\vte` is
  `alacritty/vte` in this workspace.
- [xterm.js](reference-inventory/xterm-js.md), focused pass against
  `C:\dev\xterm.js`.
- [vt100-rust](reference-inventory/vt100-rust.md), focused pass against
  `C:\dev\vt100-rust`.
- [emacs-libvterm](reference-inventory/emacs-libvterm.md), focused pass
  against `C:\dev\emacs-libvterm`.

## Architecture Findings

### Parser Boundary

`alacritty/vte` is the safest parser seed. Its own docs say it is a parser for
VT emulators, based on Paul Williams' state machine, and that it does not assign
meaning to actions. The host implements `Perform`, and the parser delegates
print, execute, CSI, OSC, DCS and escape actions.

Local refs:

- [Alacritty VTE inventory](reference-inventory/alacritty-vte.md)
- `C:\dev\vte\src\lib.rs:1`
- `C:\dev\vte\src\lib.rs:50`
- `C:\dev\vte\src\lib.rs:753`

Hera implication: define `terminal-protocol` events or an internal
`TerminalAction` layer immediately. Do not let `vte::Perform` leak through the
public API.

WezTerm offers the cleaner engine shape: `Terminal` owns `TerminalState` plus
`Parser`, and `advance_bytes` applies parsed actions into terminal state through
a performer.

Local refs:

- [WezTerm inventory](reference-inventory/wezterm.md)
- `C:\dev\wezterm\term\src\terminal.rs:85`
- `C:\dev\wezterm\term\src\terminal.rs:164`

vt100-rust is the smallest Rust proof of the first version of that shape:
`Parser` owns `vte::Parser` plus `WrappedScreen`, and `WrappedScreen`
implements `vte::Perform` to map parser callbacks into screen mutation and
host callbacks. It is not broad enough to replace a Hera action layer, but it
is a good M1 skeleton.

Local refs:

- [vt100-rust inventory](reference-inventory/vt100-rust.md)
- `C:\dev\vt100-rust\src\parser.rs:3`
- `C:\dev\vt100-rust\src\parser.rs:48`
- `C:\dev\vt100-rust\src\parser.rs:55`
- `C:\dev\vt100-rust\src\perform.rs:5`
- `C:\dev\vt100-rust\src\perform.rs:33`
- `C:\dev\vt100-rust\src\perform.rs:34`
- `C:\dev\vt100-rust\src\perform.rs:87`

Rio and Alacritty confirm the same lower-level pattern: bytes go through a raw
parser, a performer maps parser calls into state mutation, and printable text is
batched into handler paths.

Local refs:

- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\event_loop.rs:153`
- [Rio inventory](reference-inventory/rio.md)
- `C:\dev\rio\rio-backend\src\performer\handler.rs:91`
- `C:\dev\rio\rio-backend\src\performer\handler.rs:889`

Contour independently confirms the same parser boundary: parser events are
separate from backend semantics, and the parser emits print, execute, ESC, CSI,
OSC, DCS hook/unhook and APC callbacks.

Local refs:

- [Contour inventory](reference-inventory/contour.md)
- `C:\dev\contour\src\vtparser\ParserEvents.h:15`
- `C:\dev\contour\src\vtparser\ParserEvents.h:31`
- `C:\dev\contour\src\vtparser\ParserEvents.h:108`
- `C:\dev\contour\src\vtparser\ParserEvents.h:140`
- `C:\dev\contour\src\vtparser\Parser.h:669`

GNOME VTE is a secondary parser oracle: it has its own CSI/DCS/OSC parser and
generated sequence tables, including unripe DECSIXEL handling. It should inform
coverage and edge cases, not replace the Rust `alacritty/vte` seed.

Local refs:

- [GNOME VTE inventory](reference-inventory/gnome-vte.md)
- `C:\dev\terminal-research\gnome-vte\src\parser.hh:216`
- `C:\dev\terminal-research\gnome-vte\src\parser.hh:394`
- `C:\dev\terminal-research\gnome-vte\src\parser.hh:932`
- `C:\dev\terminal-research\gnome-vte\src\parser.cc:323`
- `C:\dev\terminal-research\gnome-vte\src\parser-seq.py:981`

xterm.js is not a parser seed for Hera, but it is a good public parser-hook
warning. Its public API lets embedders register CSI, DCS, ESC, OSC and APC
handlers, including async handlers that can pause input processing. Hera should
keep extension points outside the terminal-correct hot path.

Local refs:

- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\typings\xterm.d.ts:1937`
- `C:\dev\xterm.js\typings\xterm.d.ts:1941`
- `C:\dev\xterm.js\src\common\parser\EscapeSequenceParser.ts:263`
- `C:\dev\xterm.js\src\common\parser\EscapeSequenceParser.ts:574`
- `C:\dev\xterm.js\src\common\InputHandler.ts:162`
- `C:\dev\xterm.js\src\common\InputHandler.ts:200`
- `C:\dev\xterm.js\src\common\public\ParserApi.ts:13`

### Terminal State And Screens

The core must explicitly model primary and alternate screens. Alacritty's
`Term<T>` owns an active grid and an inactive grid for primary/alternate screen,
and swaps them on screen mode transitions. xterm.js exposes normal and alternate
buffers publicly, with no scrollback on alternate. libvterm has explicit
alternate screen toggles and screen damage APIs.

Local refs:

- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\term\mod.rs:268`
- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\term\mod.rs:713`
- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\typings\xterm.d.ts:1697`
- `C:\dev\xterm.js\typings\xterm.d.ts:1699`
- `C:\dev\xterm.js\typings\xterm.d.ts:1704`
- `C:\dev\xterm.js\typings\xterm.d.ts:1709`
- `C:\dev\xterm.js\src\common\buffer\BufferSet.ts:16`
- `C:\dev\xterm.js\src\common\buffer\BufferSet.ts:82`
- `C:\dev\xterm.js\src\common\buffer\BufferSet.ts:103`
- `C:\dev\terminal-research\libvterm\include\vterm.h:562`
- [Windows Terminal inventory](reference-inventory/windows-terminal.md)
- `C:\dev\terminal\src\cascadia\TerminalCore\Terminal.hpp:157`
- `C:\dev\terminal\src\cascadia\TerminalCore\TerminalApi.cpp:242`
- `C:\dev\terminal\src\cascadia\TerminalCore\TerminalApi.cpp:293`
- [GNOME VTE inventory](reference-inventory/gnome-vte.md)
- `C:\dev\terminal-research\gnome-vte\src\vteinternal.hh:468`
- `C:\dev\terminal-research\gnome-vte\src\vteinternal.hh:469`
- `C:\dev\terminal-research\gnome-vte\src\vteinternal.hh:470`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:8412`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:8413`
- [vt100-rust inventory](reference-inventory/vt100-rust.md)
- `C:\dev\vt100-rust\src\screen.rs:55`
- `C:\dev\vt100-rust\src\screen.rs:56`
- `C:\dev\vt100-rust\src\screen.rs:57`
- `C:\dev\vt100-rust\src\screen.rs:76`
- `C:\dev\vt100-rust\src\screen.rs:548`
- `C:\dev\vt100-rust\src\screen.rs:1139`
- `C:\dev\vt100-rust\src\screen.rs:1176`

emacs-libvterm confirms the same alternate-screen requirement from a real host
embedding: native initialization enables libvterm's alternate screen, registers
screen callbacks and treats alternate-screen property changes as invalidating
the visible screen.

Local refs:

- [emacs-libvterm inventory](reference-inventory/emacs-libvterm.md)
- `C:\dev\emacs-libvterm\vterm-module.c:1268`
- `C:\dev\emacs-libvterm\vterm-module.c:1270`
- `C:\dev\emacs-libvterm\vterm-module.c:675`
- `C:\dev\emacs-libvterm\vterm-module.c:767`
- `C:\dev\emacs-libvterm\vterm-module.c:768`

Hera implication: alternate screen is not an attribute on the viewport. It is
a distinct screen state with different scrollback semantics, cursor behavior and
resize rules.

### Grid And Scrollback

Alacritty/Rio are good M1 implementation references: visible rows plus
scrollback storage, display offset, max scroll limit and wrap markers. This is
enough for a first Rust headless core.

Local refs:

- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\grid\mod.rs:110`
- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\grid\storage.rs:33`
- [Rio inventory](reference-inventory/rio.md)
- `C:\dev\rio\rio-backend\src\crosswords\grid\mod.rs:35`

GNOME VTE is the strongest row-ring reference in this set. Its `Ring` stores
row records, text/attr streams, hyperlinks and image maps, and exposes rewrap
as a first-class ring operation.

Local refs:

- [GNOME VTE inventory](reference-inventory/gnome-vte.md)
- `C:\dev\terminal-research\gnome-vte\src\ring.hh:52`
- `C:\dev\terminal-research\gnome-vte\src\ring.hh:102`
- `C:\dev\terminal-research\gnome-vte\src\ring.hh:132`
- `C:\dev\terminal-research\gnome-vte\src\ring.hh:241`
- `C:\dev\terminal-research\gnome-vte\src\ring.hh:259`
- `C:\dev\terminal-research\gnome-vte\src\cell.hh:238`

But huge scrollback is a product thesis, not a default vector length. Ghostty's
page list is the stronger long-term reference: pages are self-contained chunks
ordered from top scrollback to active page, with viewport pins and byte budgets.
WezTerm also warns that stable row indexes can become invalid when scrollback is
busy or alternate screen changes.

Local refs:

- `C:\dev\terminal-research\ghostty\src\terminal\PageList.zig:36`
- `C:\dev\terminal-research\ghostty\src\terminal\PageList.zig:117`
- `C:\dev\wezterm\mux\src\pane.rs:167`
- `C:\dev\wezterm\mux\src\renderable.rs:128`

xterm.js adds a useful public inspection shape: external consumers can see
active, normal and alternate buffers, cursor position, viewport position and
line views without owning the raw ring. Internally it uses `CircularList`,
`BufferLine` typed arrays, trim events and markers. This is not enough for
Hera's huge scrollback thesis, but it is a good M1 API reference.

Local refs:

- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\src\common\public\BufferNamespaceApi.ts:25`
- `C:\dev\xterm.js\src\common\public\BufferApiView.ts:22`
- `C:\dev\xterm.js\src\common\public\BufferApiView.ts:24`
- `C:\dev\xterm.js\src\common\buffer\Buffer.ts:31`
- `C:\dev\xterm.js\src\common\buffer\Buffer.ts:47`
- `C:\dev\xterm.js\src\common\buffer\BufferLine.ts:74`
- `C:\dev\xterm.js\src\common\buffer\BufferLine.ts:75`
- `C:\dev\xterm.js\src\common\CircularList.ts:129`
- `C:\dev\xterm.js\src\common\CircularList.ts:213`

vt100-rust is a useful lower bound for M1 storage: current rows plus a
`VecDeque<Row>` scrollback, visible rows composed from scrollback and drawing
rows, fixed-size cells and explicit wrap flags. It proves the API can be simple,
but also proves what Hera must outgrow for huge scrollback.

Local refs:

- [vt100-rust inventory](reference-inventory/vt100-rust.md)
- `C:\dev\vt100-rust\src\grid.rs:4`
- `C:\dev\vt100-rust\src\grid.rs:13`
- `C:\dev\vt100-rust\src\grid.rs:14`
- `C:\dev\vt100-rust\src\grid.rs:126`
- `C:\dev\vt100-rust\src\grid.rs:561`
- `C:\dev\vt100-rust\src\grid.rs:566`
- `C:\dev\vt100-rust\src\row.rs:4`
- `C:\dev\vt100-rust\src\row.rs:78`
- `C:\dev\vt100-rust\src\cell.rs:12`
- `C:\dev\vt100-rust\src\cell.rs:17`

emacs-libvterm is a host-adapter warning for scrollback. It keeps native
scrollback bounded by `SB_MAX`, mirrors pending rows into the Emacs buffer, and
uses host text for scrollback interaction. Useful UX, wrong storage model for
Hera's long-session thesis.

Local refs:

- [emacs-libvterm inventory](reference-inventory/emacs-libvterm.md)
- `C:\dev\emacs-libvterm\vterm-module.h:27`
- `C:\dev\emacs-libvterm\vterm-module.c:29`
- `C:\dev\emacs-libvterm\vterm-module.c:95`
- `C:\dev\emacs-libvterm\vterm-module.c:101`
- `C:\dev\emacs-libvterm\vterm-module.c:480`
- `C:\dev\emacs-libvterm\vterm-module.c:491`
- `C:\dev\emacs-libvterm\vterm-module.c:512`
- `C:\dev\emacs-libvterm\vterm.el:210`

Hera implication: M1 can use simple ring storage, but the public API should
already speak in stable row handles, byte budgets and snapshots, not raw
`Vec<Row>` ownership.

Recommended policy:

- User-facing config: max lines plus max bytes.
- Internal storage: chunks/pages with cold-history compaction later.
- Viewport addressing: stable row handle plus generation, not naked index.
- Semantic index: points to row handles and byte offsets, not copied text blobs.

### Resize And Reflow

Reflow is one of the highest-risk features. Alacritty/Rio reflow inside
`Grid::resize`, using wrap markers and display offset clamping. WezTerm tags wrap
positions during printing, rewraps primary scrollback on resize, and has ConPTY
specific preservation behavior. GNOME VTE rewraps normal scrollback but not the
alternate screen.

Local refs:

- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\grid\resize.rs:14`
- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\grid\resize.rs:100`
- [Rio inventory](reference-inventory/rio.md)
- `C:\dev\rio\rio-backend\src\crosswords\grid\resize.rs:235`
- `C:\dev\wezterm\term\src\screen.rs:225`
- `C:\dev\wezterm\term\src\screen.rs:287`
- [Contour inventory](reference-inventory/contour.md)
- `C:\dev\contour\docs\vt-extensions\line-reflow-mode.md:5`
- [Windows Terminal inventory](reference-inventory/windows-terminal.md)
- `C:\dev\terminal\src\cascadia\TerminalCore\Terminal.cpp:303`
- `C:\dev\terminal\src\buffer\out\textBuffer.cpp:2718`
- `C:\dev\terminal\src\buffer\out\ut_textbuffer\ReflowTests.cpp:403`
- `C:\dev\contour\src\vtbackend\Grid.cpp:593`
- `C:\dev\contour\src\vtbackend\Grid.cpp:813`
- `C:\dev\contour\src\vtbackend\Grid_test.cpp:512`
- [GNOME VTE inventory](reference-inventory/gnome-vte.md)
- `C:\dev\terminal-research\gnome-vte\doc\rewrap.txt:109`
- `C:\dev\terminal-research\gnome-vte\doc\rewrap.txt:117`
- `C:\dev\terminal-research\gnome-vte\doc\rewrap.txt:129`
- `C:\dev\terminal-research\gnome-vte\src\ring.cc:1338`
- `C:\dev\terminal-research\gnome-vte\src\ring.cc:1372`
- `C:\dev\terminal-research\gnome-vte\src\ring.cc:1602`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:8303`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:8342`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:8345`
- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\src\common\buffer\BufferReflow.ts:25`
- `C:\dev\xterm.js\src\common\buffer\BufferReflow.ts:45`
- `C:\dev\xterm.js\src\common\buffer\BufferReflow.ts:65`
- `C:\dev\xterm.js\src\common\buffer\BufferReflow.ts:179`
- `C:\dev\xterm.js\src\common\buffer\BufferReflow.ts:203`
- `C:\dev\xterm.js\src\common\buffer\BufferReflow.test.ts:25`
- `C:\dev\xterm.js\src\common\buffer\BufferReflow.test.ts:113`

vt100-rust is useful as a resize caveat, not a reflow target. It resizes both
grids, but when column count changes `Grid::set_size` clears wrap flags instead
of reflowing primary scrollback.

Local refs:

- [vt100-rust inventory](reference-inventory/vt100-rust.md)
- `C:\dev\vt100-rust\src\screen.rs:88`
- `C:\dev\vt100-rust\src\grid.rs:66`
- `C:\dev\vt100-rust\src\grid.rs:67`

emacs-libvterm proves the host-resize side of the same problem. Emacs window
resize is ignored while copy mode is active, then the adapter clamps width,
calls native `vterm--set-size`, and native code delegates to `vterm_set_size`
with damage flushing and redraw.

Local refs:

- [emacs-libvterm inventory](reference-inventory/emacs-libvterm.md)
- `C:\dev\emacs-libvterm\vterm.el:1642`
- `C:\dev\emacs-libvterm\vterm.el:1650`
- `C:\dev\emacs-libvterm\vterm.el:1657`
- `C:\dev\emacs-libvterm\vterm.el:1662`
- `C:\dev\emacs-libvterm\vterm-module.c:1386`
- `C:\dev\emacs-libvterm\vterm-module.c:1393`
- `C:\dev\emacs-libvterm\vterm-module.c:1396`

Hera implication: reflow must be fixture-driven from day one. Do not expose a
"correct by intuition" resize path. Track wrap metadata in rows before
attempting broad compatibility.

### Renderer Boundary

The renderer boundary should be a pull/snapshot API. Ghostty is the strongest
reference: terminal mutation is separated from `render_state_update`; render
state exposes dirty state, row iterators, cell iterators, styles, graphemes,
selection and UTF-8 text. Alacritty exposes renderable content and renderable
cells. Microsoft Terminal's docs show a clean conceptual split, while its own
control layer contains a noted circular dependency between terminal and
renderer, which Hera should avoid.

Ghostling is the smallest host proof of the same boundary: PTY bytes are fed
into libghostty, `ghostty_render_state_update` snapshots terminal state, and the
host draws through row/cell iterators without owning terminal semantics.

Local refs:

- `C:\dev\terminal-research\ghostty\include\ghostty\vt\render.h:90`
- `C:\dev\terminal-research\ghostty\include\ghostty\vt\render.h:139`
- `C:\dev\terminal-research\ghostty\include\ghostty\vt\render.h:342`
- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\term\mod.rs:637`
- `C:\dev\terminal-research\alacritty\alacritty_terminal\src\term\mod.rs:2393`
- [Ghostling inventory](reference-inventory/ghostling.md)
- `C:\dev\ghostling\main.c:1383`
- `C:\dev\ghostling\main.c:1531`
- [Windows Terminal inventory](reference-inventory/windows-terminal.md)
- `C:\dev\terminal\src\renderer\inc\IRenderData.hpp:51`
- `C:\dev\terminal\src\renderer\inc\IRenderEngine.hpp:62`
- `C:\dev\terminal\src\renderer\base\renderer.cpp:1012`
- `C:\dev\terminal\src\cascadia\TerminalControl\ControlCore.h:443`
- [GNOME VTE inventory](reference-inventory/gnome-vte.md)
- `C:\dev\terminal-research\gnome-vte\src\ringview.hh:54`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:9426`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:9445`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:10075`
- `C:\dev\terminal-research\gnome-vte\src\vte.cc:10110`
- `C:\dev\terminal-research\gnome-vte\src\drawing-cairo.hh:27`
- `C:\dev\terminal-research\gnome-vte\src\drawing-gsk.hh:47`

xterm.js is useful as a renderer boundary warning. It has a replaceable
`IRenderer`, a `RenderService` that queues refreshes, and DOM/WebGL renderer
implementations. But the browser renderer and WebGL addon are tied to DOM,
canvas and private `unsafeCore` access. Hera should copy the service contract
shape, not the extension seam.

Local refs:

- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\src\browser\services\RenderService.ts:25`
- `C:\dev\xterm.js\src\browser\services\RenderService.ts:28`
- `C:\dev\xterm.js\src\browser\services\RenderService.ts:247`
- `C:\dev\xterm.js\src\browser\renderer\shared\Types.ts:53`
- `C:\dev\xterm.js\src\browser\renderer\shared\Types.ts:72`
- `C:\dev\xterm.js\src\browser\renderer\dom\DomRenderer.ts:39`
- `C:\dev\xterm.js\addons\addon-webgl\src\WebglAddon.ts:17`
- `C:\dev\xterm.js\addons\addon-webgl\src\WebglAddon.ts:88`
- `C:\dev\xterm.js\addons\addon-webgl\src\WebglRenderer.ts:35`

emacs-libvterm reinforces the adapter boundary from the opposite direction:
libvterm reports damage, movement and cursor changes, while the host adapter
renders by inserting Emacs strings and text properties. Copy the damage contract,
not the text-buffer renderer.

Local refs:

- [emacs-libvterm inventory](reference-inventory/emacs-libvterm.md)
- `C:\dev\emacs-libvterm\vterm-module.c:575`
- `C:\dev\emacs-libvterm\vterm-module.c:580`
- `C:\dev\emacs-libvterm\vterm-module.c:586`
- `C:\dev\emacs-libvterm\vterm-module.c:628`
- `C:\dev\emacs-libvterm\vterm-module.c:633`
- `C:\dev\emacs-libvterm\vterm-module.c:777`
- `C:\dev\emacs-libvterm\vterm-module.c:831`
- `C:\dev\emacs-libvterm\vterm-module.c:847`

Hera implication: `terminal-render-model` should define stable serializable
types: `RenderFrame`, `RenderLine`, `RenderCell`, `Damage`, `CursorState`,
`Viewport`, `Selection`, `HyperlinkSpan`, and image placeholders. The core
should never import GPUI.

### PTY And Process Lifecycle

Use a trait-based PTY layer, not platform conditionals in core. WezTerm's
`portable-pty` is the best direct reference: `MasterPty` exposes resize, size,
reader and writer; `SlavePty` spawns commands; `PtySystem` opens master/slave
pairs; native dispatch maps Unix to Unix PTY and Windows to ConPTY.

Local refs:

- `C:\dev\wezterm\pty\src\lib.rs:88`
- `C:\dev\wezterm\pty\src\lib.rs:163`
- `C:\dev\wezterm\pty\src\lib.rs:263`
- `C:\dev\wezterm\pty\src\lib.rs:400`
- `C:\dev\wezterm\pty\src\win\conpty.rs:85`
- `C:\dev\wezterm\pty\src\unix.rs:22`
- [Contour inventory](reference-inventory/contour.md)
- `C:\dev\contour\src\vtpty\Pty.h:52`
- `C:\dev\contour\src\vtpty\Pty.cpp:17`
- `C:\dev\contour\src\vtpty\ConPty.cpp:17`
- `C:\dev\contour\src\vtpty\UnixPty.cpp:67`
- [GNOME VTE inventory](reference-inventory/gnome-vte.md)
- `C:\dev\terminal-research\gnome-vte\src\pty.hh:30`
- `C:\dev\terminal-research\gnome-vte\src\vte\vtepty.h:62`
- `C:\dev\terminal-research\gnome-vte\src\pty.cc:184`
- `C:\dev\terminal-research\gnome-vte\src\pty.cc:281`
- `C:\dev\terminal-research\gnome-vte\src\pty.cc:425`

xterm.js confirms the same separation from the opposite direction: the terminal
is a browser/headless emulator, while a real PTY is supplied by an external
package or remote transport. The attach addon bridges WebSocket messages into
`write` and terminal `onData`/`onBinary` back out to the socket. Treat this as
evidence for a thin transport adapter, not as a PTY model.

Local refs:

- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\README.md:17`
- `C:\dev\xterm.js\README.md:41`
- `C:\dev\xterm.js\README.md:44`
- `C:\dev\xterm.js\README.md:118`
- `C:\dev\xterm.js\addons\addon-attach\src\AttachAddon.ts:15`
- `C:\dev\xterm.js\addons\addon-attach\src\AttachAddon.ts:31`
- `C:\dev\xterm.js\addons\addon-attach\src\AttachAddon.ts:36`
- `C:\dev\xterm.js\src\common\CoreTerminal.ts:148`
- `C:\dev\xterm.js\src\common\CoreTerminal.ts:158`
- `C:\dev\xterm.js\src\common\buffer\Buffer.ts:316`

emacs-libvterm shows a real host-owned PTY path, but it is deliberately
Emacs/Unix-shaped: `vterm-mode` starts an Emacs `make-process` with
`:connection-type 'pty`, invokes `/bin/sh -c stty ... exec`, installs a filter,
and passes the process TTY name to native code. Use it as adapter evidence, not
as cross-platform PTY design.

Local refs:

- [emacs-libvterm inventory](reference-inventory/emacs-libvterm.md)
- `C:\dev\emacs-libvterm\vterm.el:797`
- `C:\dev\emacs-libvterm\vterm.el:801`
- `C:\dev\emacs-libvterm\vterm.el:803`
- `C:\dev\emacs-libvterm\vterm.el:815`
- `C:\dev\emacs-libvterm\vterm.el:817`
- `C:\dev\emacs-libvterm\vterm.el:832`
- `C:\dev\emacs-libvterm\vterm-module.c:1402`
- `C:\dev\emacs-libvterm\vterm-module.c:1412`

ConPTY must be treated as an async VT stream transport, not as Hera's
authoritative buffer. Microsoft Terminal samples and internals show the host
pipe/write model, resize pathways and close lifecycle. Resize, pipe ownership,
EOF, cursor inheritance and row preservation deserve isolated tests.

Local refs:

- [Windows Terminal inventory](reference-inventory/windows-terminal.md)
- `C:\dev\terminal\src\cascadia\TerminalConnection\ITerminalConnection.idl:22`
- `C:\dev\terminal\src\cascadia\TerminalConnection\ConptyConnection.cpp:553`
- `C:\dev\terminal\src\cascadia\TerminalConnection\ConptyConnection.cpp:667`
- `C:\dev\terminal\src\winconpty\winconpty.cpp:462`
- `C:\dev\terminal\samples\ConPTY\EchoCon\EchoCon\EchoCon.cpp:91`

### Snapshot And Replay

Hera's replay should not depend only on reparsing the entire raw byte stream.
The durable shape is:

1. Raw byte/event log with timestamps and PTY boundaries.
2. Periodic `TerminalSnapshot` with terminal state, scrollback, cursor, modes,
   viewport and semantic index.
3. Fixture generator that can replay bytes and assert against snapshots.

vt100-rust is the strongest small reference here because it exposes parser,
screen, formatted state, state diff, content diff, row diff and JSON fixture
helpers. Alacritty has byte recording plus expected terminal state tests.
libvterm ships headless harnesses and reflow fixtures. xterm.js has
serialization and headless APIs, but async parser hooks can pause input and are
a warning for Hera's throughput-sensitive design.

Local refs:

- [vt100-rust inventory](reference-inventory/vt100-rust.md)
- `C:\dev\vt100-rust\src\parser.rs:3`
- `C:\dev\vt100-rust\src\parser.rs:48`
- `C:\dev\vt100-rust\src\screen.rs:224`
- `C:\dev\vt100-rust\src\screen.rs:236`
- `C:\dev\vt100-rust\src\screen.rs:249`
- `C:\dev\vt100-rust\src\screen.rs:311`
- `C:\dev\vt100-rust\src\screen.rs:379`
- `C:\dev\vt100-rust\src\screen.rs:412`
- `C:\dev\vt100-rust\tests\helpers\mod.rs:130`
- `C:\dev\vt100-rust\tests\helpers\mod.rs:188`
- `C:\dev\vt100-rust\tests\helpers\mod.rs:194`
- `C:\dev\vt100-rust\tests\helpers\fixtures.rs:84`
- `C:\dev\terminal-research\alacritty\alacritty_terminal\tests\ref.rs:100`
- `C:\dev\terminal-research\libvterm\t\harness.c:285`
- `C:\dev\terminal-research\libvterm\t\69screen_reflow.test:9`
- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\README.md:118`
- `C:\dev\xterm.js\addons\addon-serialize\README.md:3`
- `C:\dev\xterm.js\addons\addon-serialize\typings\addon-serialize.d.ts:33`
- `C:\dev\xterm.js\addons\addon-serialize\typings\addon-serialize.d.ts:42`
- `C:\dev\xterm.js\addons\addon-serialize\src\SerializeAddon.ts:594`
- `C:\dev\xterm.js\addons\addon-serialize\src\SerializeAddon.ts:608`
- `C:\dev\xterm.js\addons\addon-serialize\src\SerializeAddon.ts:622`
- `C:\dev\xterm.js\typings\xterm.d.ts:1937`
- `C:\dev\xterm.js\typings\xterm.d.ts:1941`

### Semantic Session Layer

The README is right: agent intelligence must be above the terminal-correct core.
Ghostty has semantic prompt state, Microsoft Terminal has command marks and
documents how reflow makes marks painful, and Contour has OSC 133 plus semantic
block query extensions. These are evidence that semantic metadata is valuable
and fragile.

Local refs:

- `C:\dev\terminal-research\ghostty\src\terminal\Screen.zig:1`
- `C:\dev\terminal-research\ghostty\src\terminal\Terminal.zig:12097`
- [Windows Terminal inventory](reference-inventory/windows-terminal.md)
- `C:\dev\terminal\doc\specs\#11000 - Marks\Shell-Integration-Marks.md:144`
- `C:\dev\terminal\doc\specs\#11000 - Marks\Shell-Integration-Marks.md:179`
- `C:\dev\terminal\src\buffer\out\textBuffer.cpp:3432`
- [Contour inventory](reference-inventory/contour.md)
- `C:\dev\contour\docs\vt-extensions\osc-133-shell-integration.md:7`
- `C:\dev\contour\docs\vt-extensions\semantic-block-query.md:3`
- `C:\dev\contour\src\vtbackend\SemanticBlockTracker.h:30`
- `C:\dev\contour\src\vtbackend\Screen.cpp:4094`
- [GNOME VTE inventory](reference-inventory/gnome-vte.md)
- `C:\dev\terminal-research\gnome-vte\src\attr.hh:24`
- `C:\dev\terminal-research\gnome-vte\src\cell.hh:227`
- `C:\dev\terminal-research\gnome-vte\src\vteseq.cc:1746`
- `C:\dev\terminal-research\gnome-vte\src\vteseq.cc:1826`
- `C:\dev\terminal-research\gnome-vte\src\vteseq.cc:7920`

xterm.js adds useful sidecar primitives: OSC 8 links are tracked through a link
service and markers, decorations are anchored to markers, and the public API
lets consumers register markers, decorations and link providers. Good product
shape, but still non-authoritative metadata.

Local refs:

- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\typings\xterm.d.ts:572`
- `C:\dev\xterm.js\typings\xterm.d.ts:1232`
- `C:\dev\xterm.js\typings\xterm.d.ts:1275`
- `C:\dev\xterm.js\typings\xterm.d.ts:1285`
- `C:\dev\xterm.js\src\common\services\OscLinkService.ts:31`
- `C:\dev\xterm.js\src\common\services\OscLinkService.ts:36`
- `C:\dev\xterm.js\src\common\services\DecorationService.ts:54`
- `C:\dev\xterm.js\src\common\InputHandler.ts:3134`

emacs-libvterm gives the sharpest trust-boundary warning. OSC 51;A carries
directory and prompt metadata from shell to host; OSC 51;E queues host commands
through a whitelist; OSC 52 selection updates are opt-in and disabled by
default for security reasons.

Local refs:

- [emacs-libvterm inventory](reference-inventory/emacs-libvterm.md)
- `C:\dev\emacs-libvterm\etc\emacs-vterm-bash.sh:31`
- `C:\dev\emacs-libvterm\etc\emacs-vterm-bash.sh:38`
- `C:\dev\emacs-libvterm\etc\emacs-vterm-bash.sh:52`
- `C:\dev\emacs-libvterm\vterm-module.c:1085`
- `C:\dev\emacs-libvterm\vterm-module.c:1108`
- `C:\dev\emacs-libvterm\vterm-module.c:1115`
- `C:\dev\emacs-libvterm\vterm-module.c:651`
- `C:\dev\emacs-libvterm\vterm.el:322`
- `C:\dev\emacs-libvterm\vterm.el:341`
- `C:\dev\emacs-libvterm\vterm.el:356`

Hera implication: `terminal-protocol` can store semantic events, but
`terminal-core` must remain correct if every semantic detector is disabled.

### Advanced Protocols

Kitty graphics is too complex for M1 rendering. It needs image IDs, placements,
transmission formats, pixel/cell geometry, deletion semantics, animation and
responses. Contour and Rio prove Sixel belongs in the compatibility backlog, not
in the first headless milestone. Contour also shows that buffer capture and
semantic query are more product-relevant than immediate image rendering.

Local refs:

- [Kitty inventory](reference-inventory/kitty.md)
- `C:\dev\kitty\docs\graphics-protocol.rst:1`
- `C:\dev\kitty\docs\graphics-protocol.rst:249`
- `C:\dev\kitty\docs\graphics-protocol.rst:480`
- `C:\dev\kitty\docs\graphics-protocol.rst:860`
- `C:\dev\kitty\docs\keyboard-protocol.rst:83`
- `C:\dev\kitty\docs\keyboard-protocol.rst:292`
- `C:\dev\terminal-research\kitty\docs\graphics-protocol.rst:1`
- `C:\dev\terminal-research\kitty\docs\graphics-protocol.rst:75`
- `C:\dev\terminal-research\kitty\docs\graphics-protocol.rst:458`
- [Contour inventory](reference-inventory/contour.md)
- `C:\dev\contour\docs\features.md:21`
- `C:\dev\contour\docs\features.md:25`
- `C:\dev\contour\docs\features.md:26`
- `C:\dev\contour\src\vtbackend\SixelParser.h:25`
- `C:\dev\contour\src\vtbackend\Functions.h:662`
- [Rio inventory](reference-inventory/rio.md)
- `C:\dev\rio\rio-backend\src\ansi\sixel.rs:868`
- [GNOME VTE inventory](reference-inventory/gnome-vte.md)
- `C:\dev\terminal-research\gnome-vte\meson_options.txt:111`
- `C:\dev\terminal-research\gnome-vte\src\parser-seq.py:981`
- `C:\dev\terminal-research\gnome-vte\src\vteseq.cc:5495`
- `C:\dev\terminal-research\gnome-vte\src\sixel-parser.hh:165`
- `C:\dev\terminal-research\gnome-vte\src\sixel-context.cc:463`
- `C:\dev\terminal-research\gnome-vte\src\ring.cc:1741`

xterm.js' image addon proves the protocol is a memory and parser-extension
problem before it is a drawing problem. It wires Sixel through DCS, iTerm IIP
through OSC 1337, and has explicit pixel, storage and sequence size limits.
This supports Hera's placeholder-first policy.

Local refs:

- [xterm.js inventory](reference-inventory/xterm-js.md)
- `C:\dev\xterm.js\addons\addon-image\README.md:3`
- `C:\dev\xterm.js\addons\addon-image\README.md:26`
- `C:\dev\xterm.js\addons\addon-image\README.md:30`
- `C:\dev\xterm.js\addons\addon-image\README.md:31`
- `C:\dev\xterm.js\addons\addon-image\README.md:123`
- `C:\dev\xterm.js\addons\addon-image\README.md:159`
- `C:\dev\xterm.js\addons\addon-image\src\ImageAddon.ts:99`
- `C:\dev\xterm.js\addons\addon-image\src\ImageAddon.ts:178`
- `C:\dev\xterm.js\addons\addon-image\src\ImageAddon.ts:183`
- `C:\dev\xterm.js\addons\addon-image\src\ImageAddon.ts:193`

Hera implication: M1 should parse unknown OSC/DCS/APC/PM payloads safely,
preserve metadata hooks and expose image placeholders. Do not promise image
rendering until the core harness is credible.

## Recommended Crate Boundaries

### `terminal-core`

Owns terminal state and correctness.

- `Terminal`
- `TerminalState`
- primary and alternate screens
- grid/chunk storage
- scrollback policy
- cursor, modes, tabs, charsets
- resize/reflow
- snapshot serialization
- headless byte ingestion

It should compile and test without PTY, GPUI or platform dependencies.

### `terminal-protocol`

Owns structured protocol/event types.

- normalized VT actions after parser dispatch
- OSC, CSI, DCS, APC, PM payload models where useful
- hyperlinks
- command markers
- semantic timeline events
- replay event schema
- image metadata placeholders

### `terminal-render-model`

Owns renderer-neutral output.

- visible viewport
- scrollback slices
- dirty regions
- render cells and styles
- cursor state
- selection state
- hyperlink spans
- image placeholders

This crate should be usable by GPUI, a debug CLI, a test snapshotter or a future
remote renderer.

### `terminal-pty`

Owns process and platform IO.

- POSIX PTY
- Windows ConPTY
- resize
- lifecycle and exit status
- input/output handles
- backpressure policy
- shell startup

Use a `portable-pty`-style trait boundary and keep ConPTY quirks here.

### `terminal-fixtures`

Owns compatibility data and test utilities.

- golden snapshots
- raw byte recordings
- reflow cases
- alternate screen cases
- OSC/DCS edge cases
- ConPTY replay corpus
- fuzz minimization helpers

### `terminal-cli`

Owns debugging and dogfood commands.

- inject bytes
- dump state
- replay session
- compare snapshots
- run a direct PTY command as argv, with shell execution only behind explicit shell mode
- benchmark parse, render snapshot and memory

## Language And Platform Strategy

Decision: Hera stays Rust-first and Rust-public. The core product is a Rust
workspace with a cross-platform API. Non-Rust languages are not implementation
languages for the engine. They can appear only as reference projects, fixture
sources, generated bindings, or narrow platform shims when Rust bindings are not
enough.

Rationale:

- Rust's official platform support covers the relevant target families for this
  project: Windows MSVC, Linux GNU/musl, and macOS ARM64/x86_64 targets. Support
  is expressed by target triple, not by Linux distribution name.
- Rust has first-class conditional compilation through `cfg(target_os = "...")`
  and related target keys, so platform code can be isolated without splitting
  the codebase by language.
- Windows APIs can be called from Rust through `windows-rs`; ConPTY does not
  require C# or C++ in Hera.
- Unix PTY code can stay Rust, using Rust wrappers around POSIX-like APIs plus
  direct libc/rustix/nix calls where needed.
- Apple framework interop can stay Rust through `objc2` or framework crates if
  a future macOS adapter needs Objective-C runtime calls.
- A future C ABI can be generated from Rust with tools such as `cbindgen`; that
  is an embedding surface, not a reason to write the core in C.

### Language Matrix By Zone

| Zone | Language | Windows | Linux distros | macOS | Decision |
|---|---|---|---|---|---|
| Terminal state core | Rust only | Same core crate | Same core crate | Same core crate | No FFI, no platform imports. |
| VT parser integration | Rust | Wrap `alacritty/vte` | Wrap `alacritty/vte` | Wrap `alacritty/vte` | Parser crate hidden behind Hera actions. |
| Protocol and replay schema | Rust | Same types | Same types | Same types | Serializable Rust types, no OS coupling. |
| Render model | Rust | Same snapshot model | Same snapshot model | Same snapshot model | Renderer-neutral API consumed by adapters. |
| Scrollback storage | Rust | Same storage engine | Same storage engine | Same storage engine | Target-independent chunks/pages and budgets. |
| Semantic session layer | Rust | Same observer API | Same observer API | Same observer API | Optional sidecar, no shell-specific language. |
| Fixtures and fuzzing | Rust tooling | Can ingest ConPTY recordings | Can ingest Linux PTY recordings | Can ingest macOS PTY recordings | C/JS/Zig projects are fixture sources only. |
| CLI tooling | Rust | Native exe | Native binary | Native binary | Cargo-built debug and replay tools. |
| PTY public API | Rust traits | `terminal-pty` trait impl | `terminal-pty` trait impl | `terminal-pty` trait impl | `portable-pty`-style runtime selection. |
| Windows ConPTY impl | Rust plus Win32 FFI | Use `windows-rs` or equivalent low-level crate | Not compiled | Not compiled | No C# wrapper, no C++ dependency. |
| Linux PTY impl | Rust plus Unix syscalls | Not compiled | Use `rustix`, `libc`, `nix`, or direct bindings | Not compiled | Test glibc and musl targets, not every distro manually. |
| macOS PTY impl | Rust plus Unix syscalls | Not compiled | Not compiled | Use Unix PTY path first | Objective-C not needed for PTY. |
| macOS native adapter | Rust plus `objc2` only if needed | Not compiled | Not compiled | Optional future adapter | Avoid Objective-C source unless Rust bindings fail. |
| GPUI/Paneflow adapter | Rust | Same adapter boundary | Same adapter boundary | Same adapter boundary | Hera core exports Rust render model. |
| Future C ABI | Rust crate exporting C ABI | Optional | Optional | Optional | Generated headers via `cbindgen`, no C core. |
| Reference engines | Their own languages | Read and test against | Read and test against | Read and test against | C, C++, C#, JS, Zig remain references. |

### Non-Rust Exception Rule

Adding non-Rust source requires all conditions below:

1. The platform API cannot be called safely or maintainably from Rust.
2. The code lives outside `terminal-core`, under a platform-specific adapter.
3. The boundary is a tiny C-compatible ABI or OS call wrapper.
4. No Hera-owned core type crosses the boundary directly.
5. The adapter has platform-specific CI or replay fixtures.

Under this rule, C#, Objective-C, Swift, Zig and C++ are rejected for M1/M2 core
implementation. C remains acceptable only as generated ABI surface or imported
system/library boundary.

### Platform Target Policy

Hera should define support by target triple and libc family:

- Windows primary: `x86_64-pc-windows-msvc`, then `aarch64-pc-windows-msvc`.
- Linux primary: `x86_64-unknown-linux-gnu`, then
  `aarch64-unknown-linux-gnu`.
- Linux compatibility lane: `x86_64-unknown-linux-musl` for musl-based distros
  and static-ish distribution tests.
- macOS primary: `aarch64-apple-darwin`.
- macOS compatibility lane: `x86_64-apple-darwin`, with release-time tier check
  because Rust/CI support levels can change.

Linux should not be tracked as "Ubuntu/Fedora/Arch support" in the core engine.
Track libc, kernel assumptions, PTY behavior, shell availability and CI images.

## M1 Decision Register

| Topic | Decision | Reason |
|---|---|---|
| Parser | Wrap `alacritty/vte` first. | Mature Rust parser, clear separation between parsing and semantics. |
| Parser API | Hide parser crate behind Hera actions. | Keeps future swap/fork possible. |
| Core object | `Terminal { state, parser }`. | Clean headless API, matches WezTerm-style engine shape. |
| Scrollback | Hybrid max lines plus max bytes. | Predictable UX and memory cap. |
| Storage | Simple ring/chunks in M1, page API reserved. | Ship core faster without locking public API to a naive vector. |
| Row identity | Stable handles plus generation. | Needed for viewport, semantic markers, search and snapshots. |
| Alternate screen | Separate screen state, no normal scrollback. | Matches mature terminal behavior. |
| Resize | Fixture-driven reflow, primary scrollback only at first. | Reduces compatibility blast radius. |
| Render | Pull snapshot plus damage. | Renderer-agnostic and testable. |
| Snapshot | State snapshot plus raw event offsets. | Fast reload and deterministic replay. |
| PTY | M2 crate, not core dependency. | Core must be headless first. |
| ConPTY | Isolated quirks in `terminal-pty`. | Windows behavior should not shape `terminal-core`. |
| Language policy | Rust-public, Rust-first, FFI-contained. | Keeps packaging, CI, ownership and embedding tractable across Windows, Linux and macOS. |
| Non-Rust code | Rejected for core; allowed only as tiny platform shim or generated C ABI. | Avoids multi-language runtime debt. |
| Semantics | Observer sidecar. | Wrong metadata must not corrupt rendering. |
| Images | Metadata placeholders only. | Avoid protocol bloat before core correctness. |

## Compatibility And Fixture Backlog

Start fixtures before code grows clever.

Core fixtures:

- plain text and Unicode width
- combining marks and wide characters
- SGR reset and truecolor
- cursor save/restore
- tabs and margins
- erase in display and erase in line
- insert/delete char and line
- scroll regions
- line wrap and wrap markers
- primary/alternate screen switches: 47, 1047, 1048, 1049
- resize narrower/wider with saved cursor
- primary scrollback reflow
- alternate screen resize without normal scrollback pollution
- scrollback trim and stable row handles
- hyperlink OSC 8
- bracketed paste
- mouse modes as protocol events
- OSC 52 clipboard query and set as events
- OSC 133 or shell integration markers as optional semantics
- DCS payload capture and cancellation
- unknown OSC/DCS/APC/PM payload limits

Hard fixtures from references:

- xterm.js escape sequence fixtures:
  `C:\dev\xterm.js\test\fixtures\escape_sequence_files`
  plus parser and benchmark coverage:
  `C:\dev\xterm.js\src\browser\Terminal2.test.ts:17`,
  `C:\dev\xterm.js\test\playwright\Parser.test.ts:30`,
  `C:\dev\xterm.js\test\playwright\InputHandler.test.ts:17`,
  `C:\dev\xterm.js\test\benchmark\EscapeSequenceParser.benchmark.ts:5`
- Alacritty reference tests:
  `C:\dev\terminal-research\alacritty\alacritty_terminal\tests\ref.rs:100`
- vt100-rust fixture helpers:
  `C:\dev\vt100-rust\tests\helpers\fixtures.rs:84`,
  `C:\dev\vt100-rust\tests\helpers\fixtures.rs:248`,
  `C:\dev\vt100-rust\tests\helpers\fixtures.rs:300`,
  plus reproduction helpers:
  `C:\dev\vt100-rust\tests\helpers\mod.rs:130`,
  `C:\dev\vt100-rust\tests\helpers\mod.rs:188`,
  and corpus/fuzz entries:
  `C:\dev\vt100-rust\tests\processing.rs:5`,
  `C:\dev\vt100-rust\tests\processing.rs:10`,
  `C:\dev\vt100-rust\tests\mode.rs:10`,
  `C:\dev\vt100-rust\tests\scroll.rs:16`,
  `C:\dev\vt100-rust\tests\window_contents.rs:550`,
  `C:\dev\vt100-rust\tests\quickcheck.rs:127`,
  plus a stale AFL harness that references removed screen APIs:
  `C:\dev\vt100-rust\fuzz\src\main.rs:7`,
  `C:\dev\vt100-rust\fuzz\src\main.rs:70`,
  `C:\dev\vt100-rust\CHANGELOG.md:30`,
  `C:\dev\vt100-rust\CHANGELOG.md:36`
- WezTerm terminal tests:
  `C:\dev\wezterm\term\src\test\mod.rs:64`
- libvterm screen reflow:
  `C:\dev\terminal-research\libvterm\t\69screen_reflow.test:9`
- emacs-libvterm smoke-only test surface and host integration references:
  `C:\dev\emacs-libvterm\CMakeLists.txt:104`,
  `C:\dev\emacs-libvterm\CMakeLists.txt:106`,
  `C:\dev\emacs-libvterm\README.md:290`,
  `C:\dev\emacs-libvterm\README.md:296`,
  plus shell-integration protocol examples:
  `C:\dev\emacs-libvterm\etc\emacs-vterm-bash.sh:38`,
  `C:\dev\emacs-libvterm\etc\emacs-vterm-bash.sh:53`
- Contour parser, reflow, Sixel and image protocol cases:
  `C:\dev\contour\src\vtparser\Parser_test.cpp:46`,
  `C:\dev\contour\src\vtbackend\Grid_test.cpp:512`,
  `C:\dev\contour\src\vtbackend\SixelParser_test.cpp:25`,
  `C:\dev\contour\src\vtbackend\GoodImageProtocol_test.cpp:71`
- Windows Terminal fuzzer, ConPTY and reflow entries:
  `C:\dev\terminal\src\host\ft_fuzzer\fuzzmain.cpp:127`,
  `C:\dev\terminal\src\terminal\parser\ft_fuzzer\VTCommandFuzzer.cpp:72`,
  `C:\dev\terminal\src\winconpty\ft_pty\ConPtyTests.cpp:155`,
  `C:\dev\terminal\src\buffer\out\ut_textbuffer\ReflowTests.cpp:89`
- GNOME VTE rewrap docs, parser, streams, Unicode and Sixel tests:
  `C:\dev\terminal-research\gnome-vte\doc\rewrap.txt:106`,
  `C:\dev\terminal-research\gnome-vte\doc\ambiguous.txt:41`,
  `C:\dev\terminal-research\gnome-vte\doc\scrolling-region.txt:13`,
  `C:\dev\terminal-research\gnome-vte\src\meson.build:854`,
  `C:\dev\terminal-research\gnome-vte\src\meson.build:938`,
  `C:\dev\terminal-research\gnome-vte\src\meson.build:960`,
  `C:\dev\terminal-research\gnome-vte\src\meson.build:1029`,
  `C:\dev\terminal-research\gnome-vte\src\meson.build:1043`

Long-session fixtures:

- 10k lines, bounded line policy
- 100k lines, bounded byte policy
- 1M logical lines, cold history and viewport jump benchmark
- `cargo test` output with warnings
- `git diff` with long files
- Codex CLI session
- Claude Code session
- Windows ConPTY PowerShell session

## Risks To Keep Visible

### Parser Trap

Using `vte` does not give a terminal. It gives tokens. The hard part is action
semantics, buffer mutation, compatibility and observable state.

### Async Extension Trap

xterm.js shows parser hooks are product-useful, but async handlers can pause
input processing. Hera should not let extension hooks block terminal
correctness, PTY backpressure or replay determinism.

### Minimal Emulator Trap

vt100-rust proves a small headless core and fixture harness are feasible, but
its simple scrollback, resize and protocol surface are not enough for Hera's
long-session and agentic-session goals. Treat it as an M1 floor, not the
ceiling.

### Scrollback Trap

"Huge scrollback" can quietly become unbounded RAM. Hera needs explicit memory
budgets, chunking and benchmarks before Paneflow dogfood.

### Reflow Trap

Reflow can invalidate cursor positions, marks, command blocks, selections and
semantic indexes. This is where terminal engines get subtle bugs.

### ConPTY Trap

ConPTY is not a POSIX PTY with Windows syntax. Pipe ownership, blocking reads,
resize behavior, lifecycle and cursor inheritance need their own adapter tests.

### Semantic Trap

Command detection and agent block detection will be probabilistic unless shell
integration is active. Keep semantics useful but non-authoritative.

### Privileged OSC Trap

Shell integration can become host command execution. emacs-libvterm's OSC 51;E
is useful in Emacs because it is allowlisted, but Hera should treat command-like
OSC payloads as untrusted host requests.

### Host Buffer Trap

Rendering into a rich host text buffer gives excellent editor integration, but
it couples scrollback, wrapping, selection and redraw to the host. Hera needs a
stable render model before any GPUI or editor adapter.

### Image Protocol Trap

Kitty graphics and Sixel are not just "draw image in cell". They are transfer
protocols with state, placement, deletion, sizing and acknowledgement behavior.

## External Source Pointers

These are the public specs/docs worth keeping near the repo:

- xterm control sequences:
  <https://www.xfree86.org/current/ctlseqs.html>
- VTTEST:
  <https://invisible-island.net/vttest/>
- esctest2:
  <https://github.com/ThomasDickey/esctest2>
- `alacritty/vte` docs:
  <https://docs.rs/vte/latest/vte/>
- `portable-pty` docs:
  <https://docs.rs/portable-pty/>
- Rust platform support:
  <https://doc.rust-lang.org/rustc/platform-support.html>
- Rust conditional compilation:
  <https://doc.rust-lang.org/reference/conditional-compilation.html>
- Rust for Windows and `windows-rs`:
  <https://learn.microsoft.com/en-us/windows/dev-environment/rust/rust-for-windows>
- `rustix`:
  <https://crates.io/crates/rustix>
- `objc2`:
  <https://docs.rs/objc2/>
- `cbindgen`:
  <https://github.com/mozilla/cbindgen>
- .NET install targets:
  <https://learn.microsoft.com/en-us/dotnet/core/install/>
- Swift platform support:
  <https://www.swift.org/platform-support/>
- Zig build target options:
  <https://ziglang.org/learn/build-system/>
- Kitty graphics protocol:
  <https://sw.kovidgoyal.net/kitty/graphics-protocol/>
- Windows ConPTY:
  <https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session>
- asciicast v2:
  <https://docs.asciinema.org/manual/asciicast/v2/>
- libvterm:
  <https://www.leonerd.org.uk/code/libvterm/>

## Answer To README Hard Questions

| Question | Current answer |
|---|---|
| Parser owned, forked or wrapped? | Wrapped first, with a Hera-owned action layer. Fork only if compatibility or performance requires it. |
| Scrollback by lines, bytes or hybrid? | Hybrid. Lines for user expectation, bytes for memory truth. |
| Snapshots store bytes, state, semantics or all three? | All three, but at different layers: state snapshot, raw event offsets, optional semantic index. |
| PTY in v1? | Not in `terminal-core` M1. Add `terminal-pty` in M2. |
| Full Rust or mixed languages? | Rust-first and Rust-public. Non-Rust is reference, fixture, generated C ABI, or tiny platform shim only. |
| Do Windows/macOS/Linux require C#, Objective-C, Swift, Zig or C++? | No for core and PTY API. Use Rust with `windows-rs`, Unix syscall bindings, and optional `objc2` if an Apple adapter needs framework calls. |
| Image support in core? | Metadata placeholders and safe payload capture only. Rendering later. |
| Smallest Paneflow API? | Byte ingestion, render snapshot, viewport query, snapshot/replay, semantic observer events. No Paneflow types. |
| Tests beyond local confidence? | Upstream-inspired golden fixtures, xterm/VTTEST/esctest2 corpus, fuzzing, ConPTY replay corpus and memory benchmarks. |

## M5 Final Report And M6 Direction

M1 through M5 now provide staged proof, not release maturity. Hera has a
headless core, PTY runtime boundary, Paneflow shadow evidence contracts, public
M4 proof artifacts and an M5 compatibility/release hardening package.

M4 public proof lives in `docs/m4-public-proof-report.md`. M5 final evidence
lives in `docs/m5-compatibility-release-hardening-report.md` and under
`evidence/m5/`. It covers the baseline contract, fixture-backed compatibility
matrix, scrubbed replay derivatives, Paneflow shadow scenario contract,
platform rows, package readiness, API audit and security posture.

The current verdict is still not default host replacement or public pre-release
packaging. Compatibility and replay evidence improved materially: CSI
positioning, ED/EL/ECH and DEC private modes 47/1047/1048 are fixture-backed,
and Codex/Claude Code derivatives replay deterministically. The 2026-07-10
isolated Paneflow live shadow run also completed two scripted panes for the full
45-minute target with zero mismatch report files.

That targeted pass unlocks one narrower M6 experiment: make Hera the visible
render authority for explicitly selected Paneflow panes while retaining the
existing PTY transport, input-mode authority and default Alacritty path. M6 must
keep engine ownership immutable per pane, expose an explicit fallback, measure
the visible Windows canary and record a go/no-go decision from scrubbed
evidence. It must not remove Alacritty, replace the PTY runtime or make Hera the
default.

The active contract and tracker are
`tasks/prd-m6-paneflow-controlled-host-replacement.md` and
`tasks/prd-m6-paneflow-controlled-host-replacement-status.json`. The M5 report
remains the historical closeout; this decision register and the M6 PRD carry
the subsequent host-experiment decision.

A successful Windows canary can authorize a broader canary, not default or
cross-platform replacement. Linux/macOS remain blocked rather than measured,
and every required interaction, fallback, output-ordering, latency, memory and
non-regression gate must stay explicit. Public packaging remains a separate
blocked track: dependent dry-runs still need a staging strategy,
`cargo-semver-checks` is unavailable, `cargo-audit` and Scorecard are
unavailable locally, and `cargo-deny` still lacks an explicit license policy.
