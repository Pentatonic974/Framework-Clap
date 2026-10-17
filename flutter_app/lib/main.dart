library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'core/app_status.dart';
import 'core/app_theme.dart';
import 'core/project_model.dart';
import 'features/build/build_panel.dart';
import 'features/designer/grid_designer.dart';
import 'features/editor/editor_state.dart';
import 'features/editor/nodal_editor.dart';

// Le provider de statut du footer est partagé via core/app_status.dart.

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
    const horizontalColWidth = 250.0;

    return Column(
      children: [
        // ------------- header -------------
        Container(
          height: 50,
          padding: const EdgeInsets.symmetric(horizontal: 16),
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
              const Spacer(),
              _MetaField(
                width: 110,
                label: 'nom',
                initial: project.spec.meta.name,
                onChanged: (v) {
                  final meta = project.spec.meta;
                  ref.read(projectProvider.notifier).setMeta(GraphMeta(
                        name: v, id: meta.id, version: meta.version, vendor: meta.vendor,
                        url: meta.url, feature: meta.feature));
                },
              ),
              const SizedBox(width: 8),
              _MetaField(
                width: 170,
                label: 'id',
                initial: project.spec.meta.id,
                onChanged: (v) {
                  final meta = project.spec.meta;
                  ref.read(projectProvider.notifier).setMeta(GraphMeta(
                        name: meta.name, id: v, version: meta.version, vendor: meta.vendor,
                        url: meta.url, feature: meta.feature));
                },
              ),
              const SizedBox(width: 12),
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
        // ------------- workspace -------------
        Expanded(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Expanded(child: NodalEditor()),
              VerticalDivider(width: 1, color: borderColor),
              SizedBox(
                width: horizontalColWidth + 60,
                child: const BuildPanel(),
              ),
              VerticalDivider(width: 1, color: borderColor),
              Expanded(
                flex: 1,
                child: GridDesigner(),
              ),
            ],
          ),
        ),
        // ------------- footer -------------
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

class _MetaField extends StatelessWidget {
  final double width;
  final String label;
  final String initial;
  final ValueChanged<String> onChanged;

  const _MetaField({
    required this.width,
    required this.label,
    required this.initial,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: width,
      height: 34,
      child: TextField(
        controller: TextEditingController(text: initial),
        onChanged: onChanged,
        textAlignVertical: TextAlignVertical.center,
        style: const TextStyle(fontSize: 12),
        decoration: InputDecoration(
          labelText: label,
          labelStyle: const TextStyle(fontSize: 9, color: textMuted),
          filled: true,
          fillColor: const Color(0xFF222530),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(4),
            borderSide: const BorderSide(color: borderColor),
          ),
        ),
      ),
    );
  }
}
