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

import '../../core/app_status.dart';
import '../../core/app_theme.dart';
import '../../core/project_io.dart';
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

  Future<void> run(String projectJson) async {
    if (running) return;
    running = true;
    final status = ref.read(clapsStatusProvider.notifier);
    try {
      final tmp = Directory.systemTemp;
      final file = File('${tmp.path}/claps_project.json');
      await file.writeAsString(projectJson);
      final root = await ProjectIO.workspaceRoot();
      if (root == null) {
        status.set(
            'workspace Rust introuvable (rust/Cargo.toml) — lancez l\'app depuis le dépôt');
        state = BuildReport(
          ok: false,
          formatOk: const [],
          warnings: const [],
          errors: ['workspace Rust introuvable (rust/Cargo.toml)'],
        );
        return;
      }
      status.set('Build CLAP en cours (cargo build --release)…');
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
        final report = BuildReport(
          ok: (parse['ok'] ?? false) as bool,
          formatOk: (parse['format_ok'] as List?)?.cast<String>() ?? const [],
          warnings: (parse['warnings'] as List?)?.cast<String>() ?? const [],
          errors: (parse['errors'] as List?)?.cast<String>() ?? const [],
          artifactPath: parse['artifact_path'] as String?,
          installPath: parse['install_path'] as String?,
        );
        state = report;
        if (report.ok) {
          status.set(
              'Build OK : ${report.artifactPath ?? '?'}${report.installPath != null ? ' → ${report.installPath}' : ''}');
        } else {
          status.set('Build échoué : ${report.errors.join(' | ')}');
        }
      } catch (_) {
        state = BuildReport(
          ok: false,
          formatOk: const [],
          warnings: [if (errout.isNotEmpty) errout],
          errors: ['build-service : sortie illisible ou échec de compilation'],
        );
        status.set('Build échoué (sortie illisible) — voir le panneau');
      }
    } catch (e) {
      state = BuildReport(
        ok: false,
        formatOk: const [],
        warnings: const [],
        errors: ['bridge : $e'],
      );
      status.set('Erreur du pont de build : $e');
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
    final project = ref.watch(projectProvider);
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
              icon: const Icon(Icons.build_circle_outlined),
              label: const Text('Build Plugin (.clap)'),
              onPressed: validation.errors.isNotEmpty
                  ? null
                  : () => ref.read(buildProvider.notifier).run(
                      ref.read(projectProvider).spec.toPrettyJson()),
            ),
            const SizedBox(height: 6),
            OutlinedButton.icon(
              icon: const Icon(Icons.save, size: 16),
              label: const Text('Enregistrer le projet (JSON)'),
              onPressed: () async {
                final path = await ProjectIO.defaultProjectPath(
                    ref.read(projectProvider).spec.meta.name);
                if (path == null) {
                  ref.read(clapsStatusProvider.notifier)
                      .set('Impossible de trouver la racine du projet (Cargo.toml)');
                  return;
                }
                final ok = await ProjectIO.writeProject(
                    path, ref.read(projectProvider).spec.toPrettyJson());
                if (ok) {
                  ref.read(projectProvider.notifier).setFilePath(path);
                  ref.read(clapsStatusProvider.notifier).set('Projet enregistré : $path');
                } else {
                  ref.read(clapsStatusProvider.notifier).set('Échec d\'écriture : $path');
                }
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
              if (report.installPath == null && report.ok)
                const Text('(--no-install ou chemin refusé — copier manuellement)',
                    style: TextStyle(fontSize: 10, color: textMuted)),
              for (final e in report.errors)
                Text(e, style: const TextStyle(color: Color(0xFFEF5350), fontSize: 11)),
              for (final w in report.warnings)
                Text(w, style: const TextStyle(color: textMuted, fontSize: 11)),
            ],
            const SizedBox(height: 10),
            Text(
              'Chemin du projet : ${project.filePath.isEmpty ? '(non enregistré)' : project.filePath}',
              style: const TextStyle(fontSize: 10, color: textMuted),
            ),
          ],
        ),
      ),
    );
  }
}
