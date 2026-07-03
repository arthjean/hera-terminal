# AGENTS.md

Project guidance for agents working in Hera.

## Project Thesis

Hera is a Rust-first terminal engine core, not a terminal application clone.

Preserve the README thesis: study existing engines, extract durable invariants,
build a correct headless foundation, then innovate once the base is solid. The
target is renderer-agnostic, cross-platform, terminal-correct, memory-bounded for
large scrollbacks, deterministic for snapshot/replay, optionally session-aware,
and cleanly embeddable.

Do not optimize for visible product surface before terminal correctness,
fixtures, replay and embedding boundaries are real.

## Source Of Truth

Read these first:

1. `README.md` for vision, non-goals and milestone framing.
2. `docs/research-map.md` for architecture decisions, M1 sequencing and risks.
3. Relevant files under `docs/reference-inventory/` only when touching an area
   covered by that reference.

When README and research map disagree on implementation order, prefer
`docs/research-map.md`. The README describes the long-term shape; the research
map is the current decision register.

Known drift to keep in mind: `README.md` still says the next step is writing
`docs/research-map.md`, but that file already exists. If milestone text changes,
update the README and research map together.

## Current Milestone

The next implementation step is M1: create the minimal headless Rust workspace.

Initial crates:

- `terminal-core`
- `terminal-protocol`
- `terminal-render-model`
- `terminal-fixtures`
- `terminal-cli`

Hold `terminal-pty` until the core can ingest bytes, maintain visible state,
switch primary and alternate screens, resize predictably and serialize snapshots.
PTY belongs in M2, not in the first correctness milestone.

## Architectural Guardrails

`terminal-core` owns terminal state and correctness:

- parser integration
- `Terminal { state, parser }`
- primary and alternate screens
- grid, row identity and scrollback policy
- cursor, modes, tabs and charsets
- resize and reflow
- snapshot serialization
- headless byte ingestion

`terminal-core` must not depend on PTY code, GPUI, renderer crates, windowing,
font stacks, platform APIs or Paneflow types.

Wrap `alacritty/vte` first, behind a Hera-owned action boundary. Do not expose
`vte::Perform` or parser-specific types in the public Hera API. `vte` tokenizes
bytes; Hera owns terminal semantics, state mutation, compatibility and
observable state.

`terminal-render-model` owns renderer-neutral output: visible viewport,
scrollback slices, dirty regions, render cells, styles, cursor state, selection
state, hyperlink spans and image placeholders. Prefer pull snapshots and damage
models over renderer callbacks.

`terminal-protocol` owns structured protocol and replay types: normalized VT
actions, OSC/CSI/DCS payload models where useful, hyperlinks, semantic timeline
events, replay schemas and image metadata placeholders.

The semantic/session layer is optional and non-authoritative. Agent-aware
features observe bytes, input, PTY events, timestamps and patterns. If command
detection or agent block detection is wrong, rendering, replay and terminal
state must remain correct.

## Scrollback, Snapshot And Replay

Do not implement "infinite scrollback". Hera needs bounded long-session storage:
line limits for user expectation, byte budgets for memory truth, chunk/page-ready
storage, stable row handles, fast viewport access and benchmarks.

Snapshots and replay are first-class. Prefer APIs that can support:

- terminal state snapshot
- raw byte/event offsets
- semantic index snapshot
- deterministic fixture replay
- bug reproduction from recorded streams

Avoid public APIs that expose naked row indexes or raw internal vectors. Stable
handles and explicit generation/version fields are safer for reflow, selection,
markers, semantic indexes and replay.

## Platform And Language Policy

Hera is Rust-first and Rust-public. Non-Rust code is allowed only as reference
material, fixture source, generated C ABI, or a tiny platform shim when Rust
bindings are not maintainable.

Core logic must compile without platform-specific imports. Platform behavior
belongs behind explicit traits and crate boundaries. Windows ConPTY details stay
out of `terminal-core`; Unix PTY details stay out of `terminal-core`.

Do not introduce C#, C++, Swift, Zig or Objective-C source into M1/M2 core work.
Use Rust plus crates such as `windows-rs`, `rustix`, `nix`, `libc` or `objc2`
only where the relevant adapter actually needs them.

## Non-Goals For Early Work

Do not build these before the headless core is proven:

- desktop terminal app
- GPU renderer
- theme/config system beyond tests
- terminal multiplexer
- shell integration magic
- Paneflow-specific core API
- full Kitty/iTerm2/Sixel rendering
- privileged host command execution from OSC payloads

Advanced protocols can be parsed, preserved or represented as safe metadata
before they are rendered.

## Testing And Verification

Terminal behavior changes need fixtures. Prefer raw byte inputs plus golden
snapshots over prose-only confidence.

As the workspace appears, verification should grow in this order:

1. `cargo fmt`
2. `cargo check --workspace`
3. focused golden snapshot tests
4. replay tests from recorded byte streams
5. resize/reflow and alternate-screen fixtures
6. fuzz or corpus tests for parser/action boundaries
7. memory benchmarks for 10k, 100k and 1M line scenarios

Do not mark a terminal behavior as correct because it works in one local shell.
Use upstream-inspired fixtures, VTTEST/esctest-style cases, ConPTY recordings and
large-output sessions when the behavior touches compatibility.

## Rust Implementation Rules

Keep public APIs small and boring until fixtures prove the shape.

Prefer explicit domain types over loosely typed strings and tuples. Use `Result`
at fallible boundaries. Avoid production `unwrap()` and `expect()` unless the
invariant is documented and truly local. Do not leak platform `cfg` branches into
public core APIs.

Keep modules focused. Add abstractions only when they protect a real boundary:
parser actions, terminal state, render model, replay, fixtures, PTY adapters or
semantic observers.

## Research Workflow

Reference repos live outside this repository. Do not vendor cloned terminal
engines into Hera.

When researching an area, start from `docs/research-map.md`, then open the
matching inventory file. Use local reference paths cited in the inventories when
available. If relying on external docs or specs, cite the URL in the generated
doc or final answer.

Do not re-run broad research when a focused inventory already answers the
question. Update the inventory only when new evidence changes a decision.

## Documentation Discipline

Keep `README.md` short and thesis-level. Keep durable architecture decisions in
`docs/research-map.md`. Keep per-engine evidence in `docs/reference-inventory/`.

When changing a decision, update the decision register, the impacted inventory
references and any README milestone text that would otherwise mislead future
agents.

Use ASCII text unless editing a file that already intentionally uses non-ASCII.

## Git Hygiene

This repo may contain research docs or WIP edits from another session. Before
editing, check `git status --short`. Do not revert or rewrite unrelated changes.

Keep commits atomic:

- `docs(...)` for research and guidance
- `feat(...)` for new engine code
- `test(...)` for fixtures and harnesses
- `refactor(...)` only when behavior is unchanged
- `chore(...)` for workspace/config maintenance

Before committing, inspect the staged diff and ensure the message describes only
what is staged.
