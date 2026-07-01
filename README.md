# kepler-terminal

Nom de projet: `kepler-terminal`.

## Vision

Construire un terminal engine moderne en Rust, concu comme un coeur propre pour les outils developpeur agentiques, les terminaux desktop, les terminaux embarques et les longues sessions CLI.

Le but n'est pas de cloner Alacritty, Ghostty, WezTerm, Kitty ou Windows Terminal. Le but est d'etudier leurs meilleures idees architecturales, d'extraire les invariants solides, puis de construire un nouvel engine avec une these plus nette:

> Un coeur terminal renderer-agnostic, cross-platform, pense pour la correction, les enormes scrollbacks, le replay deterministe, l'intelligence de session structuree et l'embedding propre.

C'est une approche de "fork mental": apprendre des engines existants, garder les meilleurs patterns, eviter d'heriter des contraintes produit, puis innover quand la base est saine.

## Pourquoi Ce Projet Compte

Le terminal redevient l'interface centrale du travail logiciel assiste par IA. Claude Code, Codex CLI, Gemini CLI, les build systems, les test runners, les linters, les package managers et les outils de deploiement passent tous par des flux terminal.

Les terminaux traditionnels optimisent surtout l'affichage interactif du texte. La prochaine couche doit aller plus loin:

- conserver les longues sessions sans perdre le contexte utile
- identifier les commandes, prompts, exits, diffs, logs et actions d'agents
- supporter snapshot et replay sans reparsing fragile de tout le flux brut
- exposer une API stable pour GUI, TUI, tests headless et sessions distantes
- rester coherent entre Linux, macOS et Windows
- rester rapide sous les gros volumes produits par les agents et outils modernes

Paneflow est le premier terrain naturel de dogfood.

## Forme Initiale Du Produit

Le premier artefact doit etre un workspace Rust, pas une application terminal complete.

Crates recommandees:

- `terminal-core`: integration parser VT, modele d'ecran, scrollback, curseur, modes, resize, snapshots.
- `terminal-pty`: abstraction POSIX PTY et Windows ConPTY.
- `terminal-protocol`: events structures, blocs de commande, marqueurs de prompt, hyperlinks, metadata image, format de replay.
- `terminal-render-model`: modele cell/line/damage neutre pour GPUI ou d'autres frontends.
- `terminal-fixtures`: tests de compatibilite, golden snapshots, replays issus de vraies sessions CLI.
- `terminal-cli`: petit outil debug pour injecter des bytes, inspecter l'etat, rejouer des sessions et benchmarker.

L'engine doit etre utilisable en headless avant l'existence du moindre renderer.

## Non-Objectifs Pour La Premiere Version

- Pas d'application desktop complete.
- Pas de systeme theme/config au-dela de ce qui est necessaire aux tests.
- Pas de renderer GPU custom.
- Pas de terminal multiplexer.
- Pas de shell integration magique avant que le modele terminal brut soit correct.
- Pas de promesses larges sur les protocoles avant le harness de compatibilite.

Le premier milestone est la correction et l'embeddability, pas la surface produit.

## Codebases De Reference

### References Rust

- `alacritty/alacritty`: architecture terminal Rust mature, grid, scrollback, input, config, renderer OpenGL.
- `alacritty/vte`: parser VT Rust base sur la state machine ANSI de Paul Williams.
- `wezterm/wezterm`: terminal Rust complet avec multiplexer, PTY, protocoles image, hyperlinks, etat terminal riche.
- `raphamorim/rio`: Rust plus WebGPU, utile pour penser un renderer moderne.
- `wezterm/portable-pty`: abstraction PTY cross-platform pratique, dans WezTerm.

### References Architecture

- `ghostty-org/ghostty`: terminal Zig moderne, libghostty, separation terminal/runtime/renderer, scrollback page-based.
- `ghostty-org/ghostling`: exemple minimal d'embedding de libghostty.
- `kovidgoyal/kitty`: protocoles avances, graphics, remote control, idees de performance.
- `contour-terminal/contour`: terminal C++ pour power users, utile pour les features et edge cases VT.
- `microsoft/terminal`: contraintes Windows, ConPTY, input, text buffer, renderer, memoire.
- `GNOME/vte`: widget terminal GTK mature et profondeur historique de compatibilite.
- `xtermjs/xterm.js`: API publique d'embedding, modele d'addons, renderer web, accessibilite, cas d'usage type VS Code.

### Libs Plus Petites

- `doy/vt100-rust`: modele simple byte stream vers representation memoire.
- `mmastrac/vt-push-parser`: idees de parser VT minimal-allocation.
- `libvterm`: modele terminal C callback-based et toolkit-agnostic.
- `akermu/emacs-libvterm`: embedding reel d'une lib terminal dans un host complexe.

## Principes De Design

### Coeur Renderer-Agnostic

Le coeur terminal ne doit pas connaitre GPUI, wgpu, OpenGL, Metal, DirectWrite, CoreText, Fontconfig ou un windowing system.

Il doit produire un modele de rendu compact:

- viewport visible
- alternate screen
- slices de scrollback
- dirty regions
- attributs de cellules
- etat du curseur
- etat de selection
- spans hyperlinks
- marqueurs semantiques

### Cross-Platform Par Construction

Linux, macOS et Windows sont des cibles de premiere classe.

Le code specifique plateforme doit vivre derriere des traits explicites:

- creation PTY
- resize
- lifecycle process
- signals et control events
- handles stdin/stdout/stderr
- detection du shell
- setup d'environnement

Le modele terminal core doit compiler et passer ses tests sur toutes les plateformes sans exposer les conditionnels de plateforme dans l'API publique.

### Sessions Enormes Sans Gaspillage Memoire

Le modele de scrollback doit etre une innovation centrale.

Pistes de recherche:

- scrollback logique par lignes pour une configuration predictable
- stockage budgete en bytes pour les plafonds memoire
- allocation par pages/chunks inspiree de Ghostty
- compression ou deduplication de l'historique froid
- acces aleatoire rapide pour search et viewport jumps
- indexes semantiques pour commandes, diffs, prompts et exits

La cible n'est pas le "scrollback infini". La cible est un historique borne, inspectable et previsible qui tient les vrais workflows agentiques.

### Snapshot Et Replay First

L'engine doit supporter des snapshots deterministes:

- etat terminal courant
- etat du scrollback
- curseur et modes
- viewport
- index d'events semantiques
- offsets de bytes bruts quand disponibles

Cela permet:

- sessions reconnectables
- reproduction de bugs
- generation de fixtures
- reload UI rapide
- partage de sessions
- terminal time travel

### Agent-Aware, Mais Terminal-Correct

Le terminal doit rester correct pour les shells et TUIs classiques. L'intelligence agentique doit etre une couche au-dessus du modele terminal, pas un hack dans le parsing VT.

Couches semantiques possibles:

- detection de frontieres de commandes
- detection de prompts
- capture d'exit status
- detection de blocs diff
- groupement tool calls et logs
- reconnaissance de markdown ou code blocks dans les sorties CLI
- export structure pour les sessions Paneflow

## Premiers Milestones

### M0: Research Map

Cloner les repos de reference et mapper:

- architecture parser
- representation grid
- representation scrollback
- comportement resize/reflow
- abstraction PTY
- handling Windows specifique
- support snapshot/replay
- frontiere renderer
- tests et fixtures

Output: `docs/research-map.md`.

### M1: Prototype Core Headless

Construire une crate Rust capable de:

- ingerer des bytes via un parser VT
- maintenir l'etat de l'ecran visible
- maintenir le scrollback
- gerer alternate screen
- resize de facon previsible
- exposer un viewport neutre pour le renderer
- serializer un snapshot

Output: `terminal-core` plus golden tests.

### M2: Harness PTY Reel

Executer de vraies commandes via PTY:

- Linux/macOS POSIX PTY
- Windows ConPTY
- startup shell
- resize
- EOF et exit handling
- backpressure safety

Output: `terminal-cli run <command>` avec dumps de snapshot.

### M3: Dogfood Paneflow

Ajouter une integration Paneflow derriere feature flag:

- garder le chemin UI existant intact
- envoyer les bytes PTY au nouvel engine
- render via un adapter GPUI
- comparer le comportement au chemin actuel base sur Alacritty
- capturer de longues sessions Codex CLI et Claude Code

Output: branche Paneflow avec validation side-by-side.

### M4: Public Proof

Publier l'engine quand il peut montrer:

- matrice de compatibilite
- benchmarks
- profil memoire pour 10k, 100k et 1M lignes
- demo de replay
- demo d'integration Paneflow
- exemple clair d'API

## Questions Dures

- Est-ce que le parser est owned, forked depuis `alacritty/vte`, ou wrapped?
- Est-ce que le scrollback se configure par lignes, bytes, ou politique hybride?
- Les snapshots stockent-ils bytes bruts, etat terminal, events semantiques, ou les trois?
- Le PTY doit-il etre dans le scope v1, ou la premiere crate publique doit-elle etre terminal-state only?
- Quelle part de Kitty/iTerm2/Sixel image support appartient au core model?
- Quelle est la plus petite API capable de servir Paneflow sans devenir Paneflow-specific?
- Quels tests prouvent la correction terminal au-dela de la confiance locale?

## Naming

Nom retenu: `kepler-terminal`.

Kepler communique precision, observation, invariants et systeme. C'est coherent avec la these du projet: un coeur terminal renderer-agnostic, correct, deterministe, embeddable et capable de rendre les longues sessions inspectables.

Avant publication publique, verifier au minimum:

- disponibilite crates.io
- disponibilite GitHub
- risque de confusion marque
- nom du package Rust et noms des crates internes

## Intention Du Dossier

Ce dossier commence comme brief projet et hub de recherche. Il peut devenir le vrai workspace Rust une fois le scope valide.

La prochaine etape utile est de cloner les codebases de reference sous `C:\dev\terminal-research`, puis de rediger `docs/research-map.md` avec des observations concretes sur chaque engine.
