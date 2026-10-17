# CLAP Plugin Studio Framework — Flutter + Rust

Framework No-Code / Visual Scripting de plugins audio **CLAP** : conception DSP
en graphe de nœuds, interface GUI sur grille, compilation vers un binaire
`.clap` (VST3/AUvia `clap-wrapper`).

```
┌─ flutter_app/          UI Designer (Dart/Flutter, Riverpod)
│   ├─ lib/features/editor/      Éditeur nodal (CustomPainter, câblage)
│   ├─ lib/features/designer/    Designer de grille GUI
│   ├─ lib/features/build/       Panneau Build → pipeline Rust
│   └─ lib/core/                 GraphSpec (miroir Dart), registre nœuds
└─ rust/                 Workspace Cargo
    ├─ crates/graph-core/       Modèle de graphe sérialisé + registre + validation
    ├─ crates/dsp/              Bibliothèque de nœuds DSP temps réel (+ tests)
    ├─ crates/engine/           Runtime lock-free : scheduler topologique,
    │                           params via AtomicU32, notes via rtrb SPSC,
    │                           zéro alloc / zéro lock dans `render()`
    ├─ crates/clap-runtime/     clap_entry (clap-sys 0.5) : params, audio-ports,
    │                           state, events, — cdylib → `.clap`
    └─ crates/build-service/    Pipeline CLI : GraphSpec → cargo release →
                                `<Nom>.clap` → déploiement système
```

## Règles d'or (respectées par le code)

1. **Isolation du thread audio** — le rendre `GraphRuntime::render` n'effectue
   aucune allocation (scratch préalloués + fenêtrage swap atomique du pointeur
   du contexte audio), aucun lock (aucun Mutex/RwLock dans le chemin audio),
   aucun syscall ni FFI bloquant. La communication UI→audio passe par :
   - `AtomicU32` (bits f32) par paramètre — lissage anti-zipper par échantillon
     (1 ms) côté moteur + lissage interne des nœuds DSP (20 ms) ;
   - `rtrb` SPSC non bloquant pour les notes/UI events ;
   - `RtMeterCell` (atomic) et `ScopeRing` à seqlock pour les mètres/oscilloscope
     lus par l'UI sans jamais bloquer le moteur.
2. **`CLAP_EXT_GUI`** — volontairement déclaré absent à ce jalon : l'intégration
   Flutter native (attachement `set_parent` HWND/NSView — voir `research/`)
   fait l'objet d'étapes dédiées ; un plugin sans GUI est conforme, un GUI
   cassé ne l'est pas.
3. **Extensions CLAP implémentées** — `clap.audio-ports` (stéréo princip),
   `clap.params` (count/get_info/get_value/value_to_text/text_to_value/flush),
   `clap.state` (sauvegarde/chargement du `GraphSpec` JSON).

## Conventions de nommage des paramètres

Identifiant stable = **FNV-1a 32** de `"{nodeId}/{paramId}"` — déterministe
entre les builds, réutilisé par Flutter (mapping des widgets) et par les hosts.

Le câblage d'un paramètre exposé vers un port de contrôle se fait par
correspondance des noms normalisés : `"Gain dB"` ⇄ `"gain_db"` (minuscules +
alphanumérique uniquement). Sans câble, le moteur injecte automatiquement
un « câble masqué » vers le port.

## Build d'un plugin

```bash
cd rust
cargo run -p build-service -- demo-project.json [--formats clap,vst3] [--no-install]
```

- formats : `.clap` (défaut) ; `vst3` requiert la feature `clap-runtime/vst3`
  (clap-wrapper-rs) ; `au` est réservé macOS (feature `auv2`).
- déploiement : `%COMMONPROGRAMFILES%\CLAP` (Win), `~/Library/Audio/Plug-Ins/CLAP`
  (macOS), `~/.clap` (Linux) — débrayable avec `--no-install`.
- sortie : JSON (stdout) consommé tel quel par le panneau Flutter.

## Tests

```bash
cd rust && cargo test           # 12 tests : registre, validation, DSP, moteur
cd ../flutter_app && flutter test && flutter analyze && flutter build windows
```

## Prochaines étapes suggérées

- Remplacer le pont par processus par **flutter_rust_bridge 2.13** :
  `cargo install flutter_rust_bridge_codegen@2.13.0`, `frb generate`,
  puis API typée (`listen_meters` par stream) au lieu d'un spawn CLI.
- Attache native Flutter (`CLAP_EXT_GUI`) : création du view controller
  Flutter, `FlutterDesktopViewGetHWND` (Win) puis `set_parent` du host.
- Implémenter les nœuds restants de la maquette : `pitch_shifter`,
  `reverb_convolution`, `fft_analyzer`, `osc_wavetable`, `env_mseg`,
  `transient_shaper`, `filter_formant`, `lufs_meter`, `goniometer`, clavier.
- Events nœud par nœud (index de port CLAP) au lieu du broadcast au v1.
- Persistance / `state load` recompilant le runtime avec le nouveau graphe.
