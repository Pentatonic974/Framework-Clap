/// Panneau "Build Plugin (.clap)" : sérialise le graphe -> appel du pipeline
/// Rust (`build-service`) -> affichage du rapport (artefact + install dir).
/// Le pont Dart<->Rust passe par un appel de processus pour ce jalon (robuste,
/// zéro codegen) ; la couche `rust/` FRB prête remplace ce pont automatiquement
/// dès l'activation de `flutter_rust_bridge_codegen generate`.
library;

import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';

import '../../core/app_theme.dart';
import '../editor/editor_state.dart';

class BuildReport {
  final bool ok;
  final List<String> formatOk, warnings, errors;
  final String? artifactPath, installPath;

  const BuildReport({
    required this.ok,
    required this.formatOk,
    required this.warnings,
    required this.errors,
    this.artifactPath,
    this.installPath,
  });
}

/// État du build (Notifier traitant le processus Rust).
class BuildController extends Notifier<BuildReport?> {
  bool running = false;

  @override
  BuildReport? build() => null;

  /// Cherche la racine du monorepo (contenant rust/Cargo.toml) en remontant
  /// depuis le répertoire temporaire de l'app (sync beats : 6 niveaux max).
  Future<String?> _workspaceRoot() async {
    final probeDirs = [
      (await getTemporaryDirectory()).path,
      Platform.resolvedExecutable,
      Directory.current.path,
    ];
    final candidates = <String>[];
    for (final p in probeDirs) {
      var d = p;
      for (int i = 0; i < 6; i++) {
        candidates.add(d);
        if (d.endsWith(':\\') || d == '/') break;
        d = Directory(d).parent.path;
      }
    }
    for (final c in candidates) {
      if (await File('$c/rust/Cargo.toml').exists()) return c;
    }
    return null;
  }

  Future<void> run(String projectJson) async {
    if (running) return;
    running = true;
    try {
      final tmp = await getTemporaryDirectory();
      final file = File('${tmp.path}/claps_project.json');
      await file.writeAsString(projectJson);
      final root = await _workspaceRoot();
      if (root == null) {
        state = BuildReport(
          ok: false,
          formatOk: const [],
          warnings: const [],
          errors: ['workspace Rust introuvable (rust/Cargo.toml)'],
        );
        return;
      }
      final res = await Process.run('cargo', [
        'run',
        '-q',
        '-p',
        'build-service',
        '--',
        file.path,
        '--formats',
        'clap',
        '--project-dir',
        '$root/rust',
      ], stdoutEncoding: utf8, stderrEncoding: utf8);
      final out = '${res.stdout}'.trim();
      final errout = '${res.stderr}'.trim();
      final jsonStart = out.lastIndexOf('{');
      final jsonEnd = out.lastIndexOf('}');
      final body = (jsonStart >= 0 && jsonEnd > jsonStart)
          ? out.substring(jsonStart, jsonEnd + 1)
          : out;
      try {
        final parse = jsonDecode(body) as Map<String, dynamic>;
        state = BuildReport(
          ok: (parse['ok'] ?? false) as bool,
          formatOk: (parse['format_ok'] as List?)?.cast<String>() ?? const [],
          warnings: (parse['warnings'] as List?)?.cast<String>() ?? const [],
          errors: (parse['errors'] as List?)?.cast<String>() ?? const [],
          artifactPath: parse['artifact_path'] as String?,
          installPath: parse['install_path'] as String?,
        );
      } catch (_) {
        state = BuildReport(
          ok: false,
          formatOk: const [],
          warnings: [if (errout.isNotEmpty) errout],
          errors: ['build-service : sortie illisible ou échec de compilation'],
        );
      }
    } catch (e) {
      state = BuildReport(
        ok: false,
        formatOk: const [],
        warnings: const [],
        errors: ['bridge : $e'],
      );
    } finally {
      running = false;
    }
  }
}

final buildProvider = NotifierProvider<BuildController, BuildReport?>(BuildController.new);

class BuildPanel extends ConsumerWidget {
  const BuildPanel({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final validation = ref.watch(validationProvider);
    final report = ref.watch(buildProvider);
    return Container(
      color: bgPanel,
      padding: const EdgeInsets.all(12),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const Text('BUILD PLUGIN', style: TextStyle(fontWeight: FontWeight.bold)),
            const Divider(color: borderColor),
            FilledButton.icon(
              style: FilledButton.styleFrom(backgroundColor: accentOrange),
              icon: report == null
                  ? const Icon(Icons.build_circle_outlined)
                  : const Icon(Icons.refresh),
              label: const Text('Build Plugin (.clap)'),
              onPressed: validation.errors.isNotEmpty
                  ? null
                  : () => ref.read(buildProvider.notifier).run(
                      ref.read(projectProvider).spec.toPrettyJson()),
            ),
            const SizedBox(height: 6),
            OutlinedButton.icon(
              icon: const Icon(Icons.copy, size: 16),
              label: const Text('Exporter le projet (JSON)'),
              onPressed: () {
                showDialog(
                  context: context,
                  builder: (_) => AlertDialog(
                    backgroundColor: bgPanel,
                    title: const Text('Projet (GraphSpec JSON)'),
                    content: SizedBox(
                      width: 480,
                      child: SelectableText(
                        ref.read(projectProvider).spec.toPrettyJson(),
                        style: const TextStyle(fontFamily: 'monospace', fontSize: 11),
                      ),
                    ),
                    actions: [
                      TextButton(
                          onPressed: () => Navigator.pop(context),
                          child: const Text('Fermer')),
                    ],
                  ),
                );
              },
            ),
            const SizedBox(height: 12),
            if (validation.errors.isNotEmpty) ...[
              const Text('Erreurs', style: TextStyle(color: Color(0xFFEF5350))),
              for (final e in validation.errors)
                Text('• $e', style: const TextStyle(fontSize: 11)),
              const SizedBox(height: 10),
            ],
            if (validation.warnings.isNotEmpty) ...[
              const Text('Avertissements', style: TextStyle(color: accentBlue)),
              for (final e in validation.warnings)
                Text('• $e', style: const TextStyle(fontSize: 11)),
              const SizedBox(height: 10),
            ],
            if (report != null) ...[
              if (report.ok)
                Text('✓ Build OK',
                    style: TextStyle(
                        color: report.ok ? accentGreen : null,
                        fontWeight: FontWeight.bold)),
              for (final f in report.formatOk) Text('format : $f'),
              if (report.artifactPath != null)
                Text('Artefact : ${report.artifactPath}'),
              if (report.installPath != null) Text('Installé : ${report.installPath}'),
              for (final e in report.errors)
                Text(e, style: const TextStyle(color: Color(0xFFEF5350), fontSize: 11)),
              for (final w in report.warnings)
                Text(w, style: const TextStyle(color: textMuted, fontSize: 11)),
            ],
          ],
        ),
      ),
    );
  }
}
