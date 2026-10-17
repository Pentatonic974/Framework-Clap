/// Menu d'actions du header : Build Plugin (.clap), Enregistrer le projet,
/// Ouvrir un projet. Remplace l'ex-panneau central (retour utilisateur :
/// « le panneau du milieu n'a rien à faire là »).
///
/// Le rapport de build s'affiche dans un dialogue + le footer ; les
/// éventuels avertissements du graphe bloquent le build (validation locale).
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/app_status.dart';
import '../../core/app_theme.dart';
import '../../core/project_io.dart';
import '../editor/editor_state.dart';
import 'build_service.dart';

class BuildActionsMenu extends ConsumerWidget {
  const BuildActionsMenu({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final validation = ref.watch(validationProvider);
    final report = ref.watch(buildProvider);
    final blocked = validation.errors.isNotEmpty;

    return PopupMenuButton<String>(
      tooltip: 'Actions du plugin',
      position: PopupMenuPosition.under,
      style: const ButtonStyle(
        padding: WidgetStatePropertyAll(EdgeInsets.symmetric(horizontal: 14, vertical: 8)),
        backgroundColor: WidgetStatePropertyAll(Color(0xFFFF7043)),
        foregroundColor: WidgetStatePropertyAll(Colors.white),
      ),
      icon: const Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.build_circle, size: 18),
          SizedBox(width: 6),
          Text('Actions', style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600)),
          Icon(Icons.arrow_drop_down, size: 18),
        ],
      ),
      onSelected: (v) async {
        switch (v) {
          case 'build':
            final rep = await ref.read(buildProvider.notifier).run(
                ref.read(projectProvider).spec.toPrettyJson());
            if (context.mounted) _showReport(context, ref, rep);
          case 'save':
            final path = await ProjectIO.defaultProjectPath(
                ref.read(projectProvider).spec.meta.name);
            if (path == null) {
              ref.read(clapsStatusProvider.notifier)
                  .set('Racine du projet introuvable (rust/Cargo.toml)');
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
          case 'copy_json':
            final spec = ref.read(projectProvider).spec.toPrettyJson();
            if (context.mounted) {
              showDialog(
                context: context,
                builder: (_) => AlertDialog(
                  backgroundColor: bgPanel,
                  title: const Text('Projet (GraphSpec JSON)'),
                  content: SizedBox(
                    width: 480,
                    child: SelectableText(spec,
                        style: const TextStyle(
                            fontFamily: 'monospace', fontSize: 11)),
                  ),
                  actions: [
                    TextButton(
                        onPressed: () => Navigator.pop(context),
                        child: const Text('Fermer')),
                  ],
                ),
              );
            }
        }
      },
      itemBuilder: (_) => [
        const PopupMenuItem(
          value: 'build',
          height: 36,
          child: Row(children: [
            Icon(Icons.build_circle_outlined, size: 16, color: accentOrange),
            SizedBox(width: 8),
            Text('Build Plugin (.clap)'),
          ]),
        ),
        PopupMenuItem(
          value: 'build',
          enabled: false,
          height: 20,
          child: Text(
            blocked ? 'graphe invalide — build bloqué' : '',
            style: const TextStyle(fontSize: 9, color: Color(0xFFEF5350)),
          ),
        ),
        const PopupMenuDivider(),
        const PopupMenuItem(
          value: 'save',
          height: 36,
          child: Row(children: [
            Icon(Icons.save, size: 16, color: accentBlue),
            SizedBox(width: 8),
            Text('Enregistrer le projet (JSON)'),
          ]),
        ),
        const PopupMenuItem(
          value: 'copy_json',
          height: 36,
          child: Row(children: [
            Icon(Icons.copy, size: 16, color: textMuted),
            SizedBox(width: 8),
            Text('Exporter le JSON (courant)'),
          ]),
        ),
        if (report != null) ...[
          const PopupMenuDivider(),
          PopupMenuItem(
            enabled: false,
            height: 24,
            child: Text(
              report.ok ? 'Dernier build : OK' : 'Dernier build : échec',
              style: TextStyle(
                  fontSize: 10,
                  color: report.ok ? accentGreen : const Color(0xFFEF5350)),
            ),
          ),
        ],
      ],
    );
  }

  void _showReport(BuildContext context, WidgetRef ref, BuildReport? rep) {
    if (rep == null) return;
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        backgroundColor: bgPanel,
        title: Text(rep.ok ? 'Build OK ✓' : 'Build échoué',
            style: TextStyle(
                color: rep.ok ? accentGreen : const Color(0xFFEF5350))),
        content: SizedBox(
          width: 460,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final f in rep.formatOk) Text('format : $f'),
                if (rep.artifactPath != null) Text('Artefact : ${rep.artifactPath}'),
                if (rep.installPath != null)
                  Text('Installé : ${rep.installPath}',
                      style: const TextStyle(color: accentGreen)),
                for (final e in rep.errors)
                  Text('• $e',
                      style: const TextStyle(color: Color(0xFFEF5350), fontSize: 11)),
                for (final w in rep.warnings)
                  Text('• $w', style: const TextStyle(fontSize: 11, color: textMuted)),
              ],
            ),
          ),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context), child: const Text('Fermer')),
        ],
      ),
    );
  }
}
