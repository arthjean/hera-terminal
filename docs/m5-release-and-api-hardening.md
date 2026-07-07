# M5 Release And API Hardening

Status: EP-005 evidence complete, public pre-release packaging still blocked
Date: 2026-07-04

EP-005 adds package metadata, docs.rs policy, release ordering, package dry-run
evidence and a public API audit for the Hera workspace. It does not publish any
crate and does not claim semver stability.

## Package Metadata

All six Hera crates are treated as intended public pre-release surfaces for this
M5 pass:

| Crate | Metadata | docs.rs policy | Package dry-run |
|---|---|---|---|
| `terminal-protocol` | pass | pass | pass |
| `terminal-render-model` | pass | pass | pass |
| `terminal-core` | pass | pass | blocked by unpublished `terminal-protocol` |
| `terminal-fixtures` | pass | pass | blocked by unpublished `terminal-core` |
| `terminal-pty` | pass | pass | blocked by unpublished `terminal-core` dev dependency |
| `terminal-cli` | pass | pass | blocked by unpublished Hera dependencies |

Each manifest now records description, license, repository, documentation,
README, keywords, categories and `[package.metadata.docs.rs]`. Homepage is
intentionally omitted because Hera does not have a dedicated crate homepage
beyond the repository and docs.rs pages.

## Release Plan

The publish order is:

```text
terminal-protocol
terminal-render-model
terminal-core
terminal-fixtures
terminal-pty
terminal-cli
```

The order keeps path dependency owners before dependents. The remaining package
dry-run failures are coherent blockers, not hidden errors: Cargo tries to
resolve publishable path dependencies from the crates.io index during package
preparation, so dependent crates cannot complete until the upstream Hera crates
exist in the registry or a future release process uses an accepted registry
staging strategy.

`cargo publish` was not run.

## API Audit

The API audit records:

| Boundary | Status | Evidence |
|---|---|---|
| Parser boundary | pass | `terminal-core` does not expose public `vte` types. |
| PTY boundary | pass | `terminal-core` stays PTY-free and `terminal-pty` hides `portable-pty` types. |
| Host and renderer boundary | pass | No Paneflow, GPUI, windowing or platform renderer types cross public crate APIs. |
| Semver baseline | blocked | `cargo-semver-checks` is not installed locally, so coverage is recorded as blocked. |

No P0 boundary leak was found. Two release blockers remain visible for M6:
run a semver baseline before publication, and either rename or explicitly
accept milestone-prefixed public constants before any semver-stable release.

## Machine Artifacts

| Path | Purpose |
|---|---|
| `evidence/m5/m5-package-readiness.json` | Metadata, docs.rs policy and package dry-run evidence. |
| `evidence/m5/m5-release-plan.json` | Dependency-safe publish order and publish out-of-scope guard. |
| `evidence/m5/m5-api-audit.json` | Public API boundary audit and semver baseline status. |

## Validation

Run:

```text
cargo run -p terminal-cli -- generate-m5-package-readiness --output evidence/m5/m5-package-readiness.json --release-plan-output evidence/m5/m5-release-plan.json
cargo run -p terminal-cli -- validate-m5-package-readiness evidence/m5/m5-package-readiness.json
cargo run -p terminal-cli -- validate-m5-release-plan evidence/m5/m5-release-plan.json
cargo run -p terminal-cli -- generate-m5-api-audit --output evidence/m5/m5-api-audit.json
cargo run -p terminal-cli -- validate-m5-api-audit evidence/m5/m5-api-audit.json
```
