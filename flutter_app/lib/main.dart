library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'core/app_status.dart';
import 'core/app_theme.dart';
import 'core/project_model.dart';
import 'core/resizable_split.dart';
import 'features/build/build_actions.dart';
import 'features/editor/editor_state.dart';
import 'features/editor/nodal_editor.dart';
import 'features/designer/grid_designer.dart';

void main() {
  runApp(const ProviderScope(child: ClapsStudioApp()));
}

class ClapsStudioApp extends StatelessWidget {
  const ClapsStudioApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'CLAP Plugin Studio',
      debugShowCheckedModeBanner: false,
      theme: buildTheme(),
      home: const Scaffold(body: StudioShell()),
    );
  }
}

class StudioShell extends ConsumerWidget {
  const StudioShell({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final project = ref.watch(projectProvider);
    final status = ref.watch(clapsStatusProvider);
    final validation = ref.watch(validationProvider);

    return Column(
      children: [
        // ------------- header (50 px, style maquette) -------------
        Container(
          height: 50,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          decoration: const BoxDecoration(
            color: bgPanel,
            border: Border(bottom: BorderSide(color: borderColor)),
          ),
          child: Row(
            children: [
              const Icon(Icons.tune, color: accentOrange),
              const SizedBox(width: 8),
              Text(
                '${project.spec.meta.name}.clap  ·  ${project.spec.meta.id}',
                style: const TextStyle(
                    fontWeight: FontWeight.w700, color: accentOrange, fontSize: 14),
              ),
              const SizedBox(width: 14),
              // Menu d'actions : Build / Enregistrer / Exporter JSON
              const BuildActionsMenu(),
              const SizedBox(width: 14),
              // Édition meta (dialogs, pas de TextField inline qui vole le focus).
              _MetaButton(
                label: 'nom',
                value: project.spec.meta.name,
                onDone: (v) {
                  final m = project.spec.meta;
                  ref.read(projectProvider.notifier).setMeta(GraphMeta(
                        name: v.isEmpty ? m.name : v,
                        id: m.id, version: m.version,
                        vendor: m.vendor, url: m.url, feature: m.feature));
                },
              ),
              const SizedBox(width: 6),
              _MetaButton(
                label: 'id',
                value: project.spec.meta.id,
                onDone: (v) {
                  final m = project.spec.meta;
                  ref.read(projectProvider.notifier).setMeta(GraphMeta(
                        name: m.name, id: v.isEmpty ? m.id : v,
                        version: m.version,
                        vendor: m.vendor, url: m.url, feature: m.feature));
                },
              ),
              const Spacer(),
              if (validation.errors.isEmpty)
                const Icon(Icons.check_circle, color: accentGreen, size: 16)
              else
                const Icon(Icons.error, color: Color(0xFFEF5350), size: 16),
              Tooltip(
                message: validation.ok
                    ? 'Graphe valide'
                    : validation.errors.take(3).join('\n'),
                child: Text(
                  validation.ok ? 'graphe valide' : '${validation.errors.length} erreur(s)',
                  style: const TextStyle(fontSize: 11, color: textMuted),
                ),
              ),
            ],
          ),
        ),
        // ------------- workspace : nodal (gauche) ⇄ designer (droite) --
        // Séparation redimensionnable via la poignée du ResizableSplit.
        Expanded(
          child: ResizableSplit(
            initialFraction: 0.6,
            left: const NodalEditor(),
            right: const GridDesigner(),
          ),
        ),
        // ------------- footer (35 px, monospace vert) -------------
        Container(
          height: 30,
          color: const Color(0xFF0A0B0D),
          alignment: Alignment.centerLeft,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          child: Text(
            status,
            style: const TextStyle(fontFamily: 'monospace', fontSize: 11, color: accentGreen),
          ),
        ),
      ],
    );
  }
}

/// Bouton meta : affiche la valeur, ouvre une boîte de saisie à la demande
/// (n'intercepte jamais les clics de travail en cours).
class _MetaButton extends StatelessWidget {
  final String label;
  final String value;
  final ValueChanged<String> onDone;

  const _MetaButton({required this.label, required this.value, required this.onDone});

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: 'Cliquez pour modifier le $label du plugin',
      child: ActionChip(
        label: Text('$label : ${_short(value, 22)}',
            style: const TextStyle(fontSize: 11)),
        backgroundColor: const Color(0xFF222530),
        side: const BorderSide(color: borderColor),
        onPressed: () async {
          final c = TextEditingController(text: value);
          final v = await showDialog<String>(
            context: context,
            builder: (ctx) => AlertDialog(
              backgroundColor: bgPanel,
              title: Text('Modifier le $label',
                  style: const TextStyle(fontSize: 16)),
              content: TextField(controller: c, autofocus: true),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(ctx), child: const Text('Annuler')),
                FilledButton(
                  style: FilledButton.styleFrom(backgroundColor: accentOrange),
                  onPressed: () => Navigator.pop(ctx, c.text),
                  child: const Text('OK'),
                ),
              ],
            ),
          );
          if (v != null) onDone(v);
        },
      ),
    );
  }

  static String _short(String s, int n) => s.length <= n ? s : '${s.substring(0, n - 1)}…';
}
