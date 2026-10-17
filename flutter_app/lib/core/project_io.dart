/// E/S de projet : chemin par défaut, lecture et écriture du JSON GraphSpec.
/// Partagé par le header (Ouvrir/Enregistrer) et le panneau Build
/// (découverte de la racine du monorepo).
library;

import 'dart:io';

class ProjectIO {
  /// Racine du monorepo (contenant `rust/Cargo.toml`), cherchée en remontant
  /// depuis l'exécutable puis le répertoire courant (6 niveaux max).
  static Future<String?> workspaceRoot() async {
    final probeDirs = <String>[
      Platform.resolvedExecutable,
      Directory.current.path,
    ];
    final candidates = <String>[];
    for (final p in probeDirs) {
      var d = p;
      for (int i = 0; i < 6; i++) {
        candidates.add(d);
        final parent = Directory(d).parent.path;
        if (parent == d) break;
        d = parent;
      }
    }
    for (final c in candidates) {
      if (await File('$c/rust/Cargo.toml').exists()) return c;
    }
    return null;
  }

  /// Chemin par défaut : `rust/projects/<nom>.json` (créé au besoin).
  static Future<String?> defaultProjectPath(String name) async {
    final root = await workspaceRoot();
    if (root == null) return null;
    final dir = Directory('$root/rust/projects');
    if (!await dir.exists()) {
      try {
        await dir.create(recursive: true);
      } catch (_) {
        return root;
      }
    }
    return '${dir.path}/${_safe(name)}.json';
  }

  static Future<String?> readProject(String path) async {
    try {
      return await File(path).readAsString();
    } catch (_) {
      return null;
    }
  }

  static Future<bool> writeProject(String path, String json) async {
    try {
      final f = File(path);
      await f.parent.create(recursive: true);
      await f.writeAsString(json);
      return true;
    } catch (_) {
      return false;
    }
  }

  static String _safe(String name) =>
      name.replaceAll(RegExp(r'[^\w\-]'), '_').isEmpty
          ? 'projet'
          : name.replaceAll(RegExp(r'[^\w\-]'), '_');
}
