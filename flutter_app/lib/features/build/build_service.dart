/// Service de build CLAP : sérialise le graphe -> `build-service` (pipeline
/// Rust) -> rapport. Appelé depuis le menu du header ; résultat affiché dans
/// le footer + dialogue de fin.
library;

import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/app_status.dart';
import '../../core/project_io.dart';

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

  Future<BuildReport?> run(String projectJson) async {
    if (running) return null;
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
        return const BuildReport(
          ok: false,
          formatOk: [],
          warnings: [],
          errors: ['workspace Rust introuvable (rust/Cargo.toml)'],
        );
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
        return report;
      } catch (_) {
        final report = BuildReport(
          ok: false,
          formatOk: const [],
          warnings: [if (errout.isNotEmpty) errout],
          errors: ['build-service : sortie illisible ou échec de compilation'],
        );
        state = report;
        status.set('Build échoué (sortie illisible) — voir le rapport');
        return report;
      }
    } catch (e) {
      final report = BuildReport(
        ok: false,
        formatOk: const [],
        warnings: const [],
        errors: ['bridge : $e'],
      );
      state = report;
      status.set('Erreur du pont de build : $e');
      return report;
    } finally {
      running = false;
    }
  }
}

final buildProvider =
    NotifierProvider<BuildController, BuildReport?>(BuildController.new);
