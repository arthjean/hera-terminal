# Hera

Hera est un moteur de terminal headless en Rust. Il transforme un flux de bytes
VT en etat terminal deterministe, snapshots renderer-neutral, scrollback borne
et replays exploitables par une GUI, une TUI, des tests ou un host distant.

Le projet vise les longues sessions CLI et agentiques sans lier la correction
du terminal a un renderer, un runtime PTY ou une application particuliere.
Paneflow est son premier terrain de dogfood.

## Etat Actuel

Hera est un workspace Rust 2024 de six crates. M1 a M5 sont termines. Le travail
M6 en cours porte sur une experience controlee d'autorite de rendu dans
Paneflow, pour des panes explicitement selectionnees. Il ne remplace ni le PTY,
ni l'input, ni le chemin terminal par defaut.

| Milestone | Statut | Preuve principale |
|---|---|---|
| M0: research map | DONE | Decisions et inventaires d'engines de reference |
| M1: headless core | DONE | Ingestion VT, ecrans, scrollback, resize, snapshots et fixtures |
| M2: PTY runtime | DONE | Commandes directes ou shell, resize, IO, lifecycle et recordings |
| M3: Paneflow shadow dogfood | DONE | Integration side-by-side sans changer le rendu autoritatif |
| M4: public proof | DONE | Replays, benchmarks, profils memoire, exemples API et evidence package |
| M5: compatibility and release hardening | DONE | 18 lignes de compatibilite en pass et dogfood Windows cible en pass |
| M6: controlled host replacement | IN PROGRESS | Baseline et activation terminees, render authority en cours |

Le dernier run M5 verifie a execute deux panes Paneflow pendant 45 minutes en
shadow mode et n'a produit aucun rapport de mismatch. Cette preuve debloque une
experience de rendu autoritatif limitee. Elle ne prouve pas encore un
remplacement cross-platform ou une release publique.

La direction M6 a ete decidee apres le rapport final M5. Ce rapport reste la
cloture historique du milestone; le research map et le PRD M6 portent la
decision courante.

Limites encore ouvertes:

- Linux et macOS ne disposent pas encore des mesures runtime M5 equivalentes.
- Les dry-runs des crates dependantes restent bloques par les crates Hera non
  publiees ou par l'absence d'une strategie de staging.
- La baseline semver et une partie de la posture supply-chain restent a fermer.
- Hera n'est ni une application terminal desktop, ni un renderer GPU, ni le
  moteur par defaut de Paneflow.

## Capacites Implementees

`terminal-core` wrappe `alacritty/vte` derriere des types Hera. Le parser
tokenise le flux, Hera possede les semantiques et l'etat observable.

Le coeur couvre aujourd'hui:

- ingestion incrementale de bytes et actions VT structurees
- ecrans primary et alternate, curseur, tabs et modes
- controles C0, CUP/HVP, ED/EL/ECH et attributs SGR
- modes DEC 47, 1047, 1048 et 1049
- scrollback borne par lignes et bytes avec row handles stables
- resize et reflow predictibles sans pollution du primary scrollback
- snapshots de viewport, cellules, styles, curseur, damage et scrollback
- bracketed paste expose dans le modele de rendu
- payloads inconnus ou avances preserves comme metadata sure

Le runtime et l'outillage ajoutent:

- PTY cross-platform derriere une frontiere Hera-owned
- execution directe en argv ou shell explicite
- resize, input/output, exit, timeout, backpressure et recordings
- golden fixtures, replay deterministe et comparaison de snapshots
- generation et validation d'evidence machine-readable pour les milestones

Le support Sixel reste volontairement limite au parsing et a la metadata. Hera
ne rend pas encore les protocoles image.

## Architecture Du Workspace

| Crate | Responsabilite |
|---|---|
| `terminal-core` | Parser integration, etat terminal, scrollback, resize et snapshots |
| `terminal-protocol` | Actions VT et payloads structures sans fuite des types `vte` |
| `terminal-render-model` | Viewport, cellules, styles, damage, curseur et placeholders neutres |
| `terminal-pty` | Process IO, resize, lifecycle et transport plateforme |
| `terminal-fixtures` | Fixtures, replays, snapshots et schemas d'evidence |
| `terminal-cli` | Debug local, execution PTY, replay, benchmarks et validation d'evidence |

La dependance centrale reste a sens unique:

```text
bytes -> terminal-core -> RenderSnapshot -> host renderer
             ^
             |
       terminal-pty events
```

`terminal-core` ne depend pas de PTY, GPUI, Paneflow, windowing ou API
plateforme. `terminal-render-model` ne depend d'aucun renderer concret.

## Demarrage Rapide

Prerequis: Rust 1.85 ou plus recent.

```powershell
cargo check --workspace
cargo test --workspace
cargo run -p terminal-core --example headless_embedder
cargo run -p terminal-cli -- replay crates/terminal-fixtures/fixtures/m1-golden.json
```

Executer une commande reelle via PTY sous Windows:

```powershell
cargo run -p terminal-cli -- run -- cmd.exe /D /C "echo Hera"
```

Sous Linux ou macOS:

```sh
cargo run -p terminal-cli -- run -- /bin/sh -lc "printf 'Hera\n'"
```

Sans argument, `terminal-cli` affiche la liste complete des commandes de debug,
replay, benchmark et validation:

```powershell
cargo run -p terminal-cli --
```

## Embedding Headless

L'API minimale consomme des bytes puis produit un snapshot neutre:

```rust
use terminal_core::{ScrollbackConfig, Terminal, TerminalConfig};

let config = TerminalConfig::with_scrollback(
    80,
    24,
    ScrollbackConfig::new(10_000, 8 * 1024 * 1024),
)?;
let mut terminal = Terminal::with_config(config);

terminal.advance_bytes(b"cargo test\r\nrunning 1 test\r\n");
terminal.resize(100, 30)?;

let snapshot = terminal.render_snapshot();
println!("rows={}", snapshot.viewport_rows().len());
```

Exemples complets:

- [`crates/terminal-core/examples/headless_embedder.rs`](crates/terminal-core/examples/headless_embedder.rs)
- [`crates/terminal-pty/examples/pty_boundary.rs`](crates/terminal-pty/examples/pty_boundary.rs)

## Verification Et Evidence

La correction terminal repose sur des inputs bruts et des snapshots golden, pas
sur la confiance dans un shell local. La matrice M5 couvre notamment les
controles C0, CUP/HVP, ED/EL/ECH, scrollback, SGR, alternate screen,
resize/reflow et bracketed paste.

Commandes de validation principales:

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo doc --workspace --no-deps
cargo run -p terminal-cli -- validate-m5-compatibility evidence/m5/compatibility-matrix.json
cargo run -p terminal-cli -- validate-m5-evidence evidence/m5/evidence-manifest.json
```

Les scenarios live PTY sont separes du test workspace normal:

```powershell
cargo test -p terminal-pty --features live-pty-tests --test live_pty -- --ignored
```

## Direction M6

M6 mesure une frontiere precise: Hera devient la source visuelle autoritative de
panes Paneflow selectionnees, tandis que le PTY, l'input-mode authority et le
chemin par defaut restent controles. Le canary doit prouver zero mismatch P0,
zero fallback, zero byte perdu, une latence bornee et une memoire comparable au
controle Alacritty avant tout elargissement.

Un succes Windows autorise au maximum un canary plus large. Le remplacement par
defaut reste interdit tant que les interactions essentielles, Linux, macOS et
les gates de non-regression ne sont pas mesures.

## Documentation

- [`docs/research-map.md`](docs/research-map.md): decision register et architecture de reference
- [`tasks/prd-m6-paneflow-controlled-host-replacement.md`](tasks/prd-m6-paneflow-controlled-host-replacement.md): contrat M6 courant
- [`tasks/prd-m6-paneflow-controlled-host-replacement-status.json`](tasks/prd-m6-paneflow-controlled-host-replacement-status.json): progression M6 courante
- [`evidence/m6/m6-baseline.json`](evidence/m6/m6-baseline.json): baseline publique et policy de decision M6
- [`docs/m5-compatibility-release-hardening-report.md`](docs/m5-compatibility-release-hardening-report.md): cloture historique M5
- [`docs/m4-public-proof-report.md`](docs/m4-public-proof-report.md): preuve publique M4
- [`docs/reference-inventory/`](docs/reference-inventory/): inventaires par engine
- [`evidence/m5/`](evidence/m5/): evidence machine-readable M5
- [`tasks/`](tasks/): PRD et trackers de milestones

## Principes Non Negociables

- Rust-first et Rust-public.
- Correction terminal avant surface produit.
- APIs publiques petites, stables et renderer-neutral.
- Scrollback borne par politique explicite, jamais "infini".
- Snapshots et replay traites comme des capacites de base.
- Features agentiques optionnelles et non autoritatives.
- Aucun type Paneflow, GPUI, PTY ou plateforme dans `terminal-core`.
- Protocoles privilegies parses comme donnees, jamais executes par le host.
