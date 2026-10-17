# memory.md — Mémoire de session · CLAP Plugin Studio

> Fichier de mémoire inter-sessions. Chaque session : lire CE fichier en premier,
> puis consulter [journal/journal.md](journal/journal.md) pour chercher une date précise
> (le journal indexe des entrées datées détaillées dans `journal/YYYY-MM-DD.md`).
> Règle : consigner ici tout état, décision, piège et reste-à-faire utile.

---

## 1. Identité du projet

- **But** : framework « CLAP Plugin Studio » — un éditeur visuel qui permet de
  construire un graphe DSP nodal, de dessiner la GUI sur une grille, puis de
  compiler un vrai plugin `.clap` (+ VST3/AU) déployable dans un DAW (cible de
  test utilisateur : **REAPER**, qui supporte CLAP nativement depuis REAPER 6.41).
- **Maquette de référence UI** : [UI maquette.html](UI%20maquette.html) (403 lignes, HTML+JS).
  Serveur de vérité visuel : couleurs, layout, registres (NODE_REGISTRY / GUI_REGISTRY).
- **Langue de travail** : FRANÇAIS pour tout échange utilisateur, commentaires de code
  et messages de commit.

## 2. Cartographie des fichiers

```
C:/Repo/Framework Clap/
├── UI maquette.html               ← maquette HTML de référence (UI + registres)
├── README.md                      ← architecture, invariants RT, prochaines étapes
├── memory.md                      ← CE fichier
├── journal/
│   ├── journal.md                 ← index des entrées datées
│   └── 2026-10-17.md              ← entrée détaillée du 17 oct 2026
├── rust/                          ← workspace Cargo (5 crates)
│   ├── Cargo.toml
│   ├── demo-project.json          ← projet démo (DemoGain) prouvé de bout en bout
│   ├── crates/graph-core/         ← GraphSpec sérialisable + validation + NodeRegistry (50 nœuds)
│   ├── crates/dsp/                ← nœuds DSP RT (commons, filters, generators, dynamics,
│   │                                 effects, math_nodes, rhythm, analysers, factory)
│   ├── crates/engine/             ← GraphRuntime (compilation, topo, wires, params atomics)
│   ├── crates/clap-runtime/       ← clap_entry FFI (clap-sys 0.5) + extensions + build .clap
│   │                                 (le JSON du graphe est injecté via env CLAPS_GRAPH_JSON
│   │                                  et inclus par include_str! à la compilation)
│   └── crates/build-service/      ← CLI pipeline : JSON → validation → cargo build → .clap → install
└── flutter_app/                   ← app Flutter (Windows)
    ├── lib/core/                  ← app_theme.dart, node_registry.dart (miroir Rust), project_model.dart
    ├── lib/features/editor/       ← nodal_editor.dart + editor_state.dart (Riverpod)
    ├── lib/features/designer/     ← grid_designer.dart (rack 8×10)
    ├── lib/features/build/        ← build_panel.dart (pont processus vers build-service)
    └── test/core_test.dart        ← 3 tests Dart
```

## 3. Contrats et invariants (NE PAS CASSER)

- **RT-safety DSP** : aucune allocation, aucun lock, aucun syscall, aucun FFI
  bloquant dans `process`. Instanciation (`create_node`) hors thread audio.
- **Sérialisation** : JSON **camelCase** (ids uniques, ports typés Audio/Control/Event/Data).
  Le registre Dart `flutter_app/lib/core/node_registry.dart` doit rester **isomorphe**
  au `NodeRegistry` Rust (graph-core) — tout ajout d'un côté engage l'autre.
- **Params** : engine = `AtomicU32` encodé + lissage anti-zipper 1 ms ; câbles masqués
  automatiques `exposedParams` → port de contrôle par nom normalisé
  (ex. `"gain_db"` ⇄ « Gain dB »). Les `NodeInputs` sont alignés sur les **ports du
  registre** (pas la liste compactée des contrôles).
- **clap-sys 0.5** : sémantique 2024 — `unsafe_op_in_unsafe_fn` autorisé au niveau
  module du clap-runtime (convention des crates FFI).
- **Formats chemins d'installation CLAP** (ordre de repli, géré par build-service) :
  Windows `%COMMONPROGRAMFILES%\CLAP` (admin) → `%LOCALAPPDATA%\Programs\Common\CLAP`
  (par utilisateur) → `~/.clap` ; macOS `~/Library/Audio/Plug-Ins/CLAP`, Linux `~/.clap`.
  `--no-install` désarme le déploiement.

## 4. Commands de vérification (statuts vérifiés au dernier run)

```bash
# Lanceur utilisateur (double-clic, ou "rebuild" pour forcer la compilation)
lancer_framework.bat              # racine du projet — app claps_studio.exe

# Vérifications manuelles :
cd "C:/Repo/Framework Clap/rust"
cargo build --workspace          # ✓
cargo test --workspace           # ✓ 12/12 (graph-core 8, dsp 2, engine 2)
cargo run -q -p build-service -- demo-project.json --formats clap --no-install
                                 # ✓ exit 0 → rust/target/claps/DemoGain.clap

```bash
cd ../flutter_app
flutter analyze
flutter test
flutter build windows --debug
```

## 5. Pièges connus / leçons

- rtrb 0.4 : `pop()` retourne `Option<T>` (pas `(T,bool)`).
- `one_pole_coef(time_ms)` = constante de temps à 50 % (PAS temps à −60 dB ×8).
- Lissage param (1 ms) + lissage interne GainNode se composent — « 20 ms + interne »
  donnait une convergence trop lente dans les tests (corrigé : 4 blocs / 2048 smp).
- Mixage multi-câbles : `split_at_mut`/`chunks_mut` pour l'aliasing sans unsafe.
- `include_str!` du graphe : le JSON est copié dans `$OUT_DIR`/temp par le pipeline;
  changer le graphe ⇒ **nouvelle compilation** (c'est voulu : graphe figé au build).
- Le panneau Build Flutter cherche la racine du monorepo en remontant depuis
  getTemporaryDirectory / resolvedExecutable / cwd — fonctionne en dev local;
  à revoir pour install utilisateur (voir reste-à-faire FRB).

## 6. Écarts assumés (documentés)

- Pont Dart↔Rust du BuildPanel = **processus** `build-service`, pas encore FRB 2.13 codegen.
- `CLAP_EXT_GUI` non annoncé au host (attachement Flutter natif HWND/NSView = jalon séparé).
- VST3/AU : compilables via features, la mise en bundle `.vst3`/`.component` reste à faire.
- jamais validé sur un DAW réel avant le 17/10 → le lot du jour le démarre (REAPER).

## 7. État au 2026-10-17 (dernière session)

- **Tout compile** : cargo build/test workspace ✓ (12 tests), flutter analyze ✓ 0 issue
  (11 issues dormantes corrigées : code mort + APIs déconseillées), flutter test 3/3 ✓.
- **UI vs maquette** : conforme sauf écarts assumés (palette + BuildPanel) et
  titre du rack ajouté (plugin-title). Backlog : « Exporter Code C++ ».
- **Premier plugin de test REAPER** : `ClapsTestNoise` (bruit → gain, knob [rust/test-plugin.json])
  construit et **installé** dans `%LOCALAPPDATA%\Programs\Common\CLAP\ClapsTestNoise.clap`.
  Test utilisateur REAPER attendu (feedback = prochaine étape).
- **Git/GitHub** : repo `https://github.com/Pentatonic974/Framework-Clap.git`
  (créé le 17/10 via API avec le jeton du credential manager), branche de travail
  **`devel`** (init `-b devel`, upstream tracké). Identité git locale :
  Pentatonic974 / Pentatonic974@users.noreply.github.com. `.gitignore` exclut
  rust/target, flutter build, .freebuff.

## 8. Ouvert (backlog)

- migration FRB du pont Build (codegen).
- `CLAP_EXT_GUI` natif.
- reste des nœuds DSP de la maquette (voir factory.rs vs node_registry.rs —
  `filter_formant`, `reverb_convolution`, `pitch_shifter`, `env_mseg`,
  `sample_player`, `lufs_meter`, `goniometer`, `quantizer`, `math_expr`… sont
  **déclarés dans le registre mais pas tous implémentés côté DSP**).
- emballage bundle VST3.
- ~~validation REAPER du premier plugin de test~~ lot 17/10 fait côté build : le test
  dans REAPER lui-même reste à faire par l'utilisateur (feedback attendu).
- « Exporter Code C++ » de la maquette (générateur C++13/17) — non démarré.

## 9. Rituels de session

1. Lire `memory.md` (ce fichier).
2. Lire `journal/journal.md` (index) puis l'entrée de la dernière date pour le contexte détaillé.
3. À la FIN de la session : créer/mettre à jour `journal/YYYY-MM-DD.md` + rafraîchir
   `journal/journal.md` + la section « État » de ce fichier.
