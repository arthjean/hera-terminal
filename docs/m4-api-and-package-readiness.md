# M4 API And Package Readiness

Status: EP-005 public API and package readiness proof
Date: 2026-07-04

## Summary

EP-005 adds compiling public API examples and records the current package,
documentation and OSS security readiness state. Hera is closer to an embeddable
Rust workspace, but it is not publish-ready yet.

The important result: `cargo test --workspace` compiles the public examples,
`cargo doc --workspace --no-deps` renders local docs, and two leaf crates package
successfully. Crates that depend on unpublished Hera crates are intentionally
blocked by Cargo package resolution until publish ordering or registry names are
decided. No `cargo publish` command was run.

## Public API Examples

| Example | Crate | Shows | Runtime boundary |
|---|---|---|---|
| `crates/terminal-core/examples/headless_embedder.rs` | `terminal-core` | byte ingestion, resize and render snapshot | no PTY, Paneflow, renderer or platform dependency |
| `crates/terminal-pty/examples/pty_boundary.rs` | `terminal-pty` | spawning a PTY, forwarding output into `terminal-core`, then reading a render snapshot | PTY runtime stays outside `terminal-core` |

Compile gate:

```text
cargo test --workspace
```

Optional manual runs:

```text
cargo run -p terminal-core --example headless_embedder
cargo run -p terminal-pty --example pty_boundary
```

The PTY example is intentionally hosted by `terminal-pty`, not `terminal-core`.
It demonstrates the host boundary without importing Paneflow, GPUI or renderer
internals into core terminal state.

## API Audit

Audit source:

- Rust API Guidelines: <https://rust-lang.github.io/api-guidelines/>
- Existing Hera boundary tests in `terminal-core` and `terminal-pty`
- `cargo test --workspace`
- `cargo doc --workspace --no-deps`

| Area | Status | Evidence |
|---|---|---|
| Public examples | pass | `cargo test --workspace` compiles both examples. |
| Parser boundary | pass | `terminal-core` tests reject public `vte` exposure. |
| PTY boundary | pass | `terminal-core` has no PTY/runtime deps; `terminal-pty` tests reject public `portable-pty` types. |
| Renderer boundary | pass | `terminal-core` exposes `RenderSnapshot` and render-model types, not GPUI or windowing types. |
| Error shape | pass | Core and PTY expose typed errors implementing `std::error::Error`. |
| Docs render | pass | `cargo doc --workspace --no-deps` completes locally. |
| Package metadata | release blocker | Cargo warns that manifests lack description, license, documentation, homepage or repository metadata. |
| Internal dependency publishing | release blocker | Dependent crates cannot package until internal crates exist in the registry or publish ordering is decided. |
| M1/M2 public names | accepted M4 gap | Milestone-prefixed constants are useful now, but should be reviewed before semver commitments. |

No P0 boundary leak was found. The release blockers are packaging and public
metadata, not parser, PTY, GPUI or Paneflow leakage.

## Package And Docs Readiness

Commands run:

```text
cargo doc --workspace --no-deps
cargo package -p terminal-core --allow-dirty --no-verify
cargo package -p terminal-protocol --allow-dirty --no-verify
cargo package -p terminal-render-model --allow-dirty --no-verify
cargo package -p terminal-fixtures --allow-dirty --no-verify
cargo package -p terminal-pty --allow-dirty --no-verify
cargo package -p terminal-cli --allow-dirty --no-verify
```

| Crate | Package status | Current reason |
|---|---|---|
| `terminal-protocol` | pass | Leaf crate packages successfully. |
| `terminal-render-model` | pass | Leaf crate packages successfully. |
| `terminal-core` | blocked | Cargo cannot resolve unpublished `terminal-protocol` from crates.io during package preparation. |
| `terminal-fixtures` | blocked | Cargo cannot resolve unpublished `terminal-core` from crates.io during package preparation. |
| `terminal-pty` | blocked | Cargo cannot resolve unpublished `terminal-core` from crates.io during package preparation. |
| `terminal-cli` | blocked | Cargo cannot resolve unpublished `terminal-core` from crates.io during package preparation. |

The workspace keeps `publish = false`, so this is readiness evidence only. The
correct next action is to decide crate names, package metadata and publish order,
not to publish during M4.

## OSS Security Baseline

Internal checks run:

| Check | Status | Result |
|---|---|---|
| Locked metadata | pass | `cargo metadata --locked --no-deps --format-version 1` sees 6 workspace packages and 8 direct registry dependencies. |
| Duplicate dependency versions | partial | `cargo tree --locked --duplicates` reports `bitflags` 1.x and 2.x through `portable-pty` and `nix`. This is indirect and non-blocking for M4. |
| Binary artifacts | pass | No executable, dynamic library or static library artifacts are present outside `target`. |
| Secret-like scan | pass | Only redaction-policy marker literals were matched in test policy code; no token material was found. |
| OpenSSF Scorecard | blocked | Scorecard CLI is not installed locally, so Hera does not claim an external Scorecard result. |

OpenSSF Scorecard is useful as an external health signal, but this M4 artifact
does not conflate it with Hera's internal posture. Internal evidence says the
checked local baseline has no obvious binary artifact or secret leak. External
repository posture remains a follow-up until the public repo and Scorecard run
exist.

## Machine Artifacts

| Path | Purpose |
|---|---|
| `evidence/m4/m4-api-audit.json` | Machine-readable public API audit. |
| `evidence/m4/m4-package-readiness.json` | Machine-readable package and docs readiness results. |
| `evidence/m4/m4-oss-security-baseline.json` | Machine-readable local security baseline and OpenSSF gap. |

These artifacts are public summaries. They do not contain private terminal
content, local home paths, tokens or raw session material.
