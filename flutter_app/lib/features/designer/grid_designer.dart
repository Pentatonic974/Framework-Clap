/// Designer de grille GUI : rack du plugin, widgets posés par drag-and-drop
/// à la position du curseur, édition du label (double-clic), mapping vers
/// les paramètres CLAP exposés, suppression. Les visualiseurs pointent un
/// nœud d'analyse, les contrôles un paramètre "nodeId/paramId".
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/app_status.dart';
import '../../core/app_theme.dart';
import '../../core/node_registry.dart';
import '../../core/project_model.dart';
import '../editor/editor_state.dart';

/// Édition d'une cellule du rack (null = cellule vide, pas de widget).
class RackPos {
  final int col, row;
  const RackPos(this.col, this.row);
}

class GridDesigner extends ConsumerStatefulWidget {
  const GridDesigner({super.key});

  @override
  ConsumerState<GridDesigner> createState() => _GridDesignerState();
}

class _GridDesignerState extends ConsumerState<GridDesigner> {
  static const cellW = 40.0;
  static const cellH = 40.0;
  static const cols = 8;
  static const rows = 10;

  /// Cellule sous un offset local du rack (clamp aux bornes).
  static RackPos _cellFrom(Offset local) => RackPos(
        (local.dx / cellW).floor().clamp(0, cols - 1),
        (local.dy / cellH).floor().clamp(0, rows - 1),
      );

  void _addWidgetAt(String type, RackPos cell) {
    final notifier = ref.read(projectProvider.notifier);
    final label = _defaultLabel(type);
    final id = notifier.nextId('w');
    notifier.addWidget(GuiWidget(
      id: id,
      widgetType: type,
      label: label,
      col: cell.col,
      row: cell.row,
      colSpan: type == 'knob' ? 2 : 2,
      rowSpan: type == 'knob' ? 2 : 1,
    ));
    notifier.bumpIds();
    // S'il n'y a aucun paramètre exposé, on en propose dans le dialogue d'édition.
    final spec = ref.read(projectProvider).spec;
    if (seedParamCapable(type) && exposedParamsMap(spec).isEmpty) {
      setStatus(ref, 'Widget posé : mappez-le à un param CLAP (icône maillon).');
    }
  }

  static String _defaultLabel(String type) {
    for (final (t, name, _) in GuiRegistry.widgets) {
      if (t == type) return name.split(' (').first;
    }
    return type;
  }

  @override
  Widget build(BuildContext context) {
    final spec = ref.watch(projectProvider).spec;
    final params = exposedParamsMap(spec);
    // Nœuds d'analyse disponibles pour les visualiseurs.
    final analysisNodes = <String, String>{
      for (final n in spec.nodes)
        if (_isAnalysisNode(n.nodeType)) n.id : n.nodeType,
    };

    final rack = SizedBox(
      width: cols * cellW,
      height: rows * cellH,
      child: DragTarget<(String, Offset)>(
        onWillAcceptWithDetails: (_) => true,
        onAcceptWithDetails: (details) {
          // Le contexte ici EST le DragTarget (iframe rack exact) : le
          // décalage vient du builder externe — clés inutiles dans ce scope.
          final box = context.findRenderObject() as RenderBox?;
          if (box == null) return;
          final local = box.globalToLocal(details.offset);
          final cell = _cellFrom(local);
          _addWidgetAt(details.data.$1, cell);
        },
        builder: (context, candidates, _) {
          final highlight = candidates.isNotEmpty;
          return Stack(
            clipBehavior: Clip.none,
            children: [
              Positioned.fill(
                child: CustomPaint(
                  painter: _GridPainter(
                    cols: cols, rows: rows, cellW: cellW, cellH: cellH,
                    highlight: highlight,
                  ),
                ),
              ),
              for (final w in spec.guiWidgets)
                Positioned(
                  left: w.col * cellW,
                  top: w.row * cellH,
                  width: _spanWidth(w, cols) * cellW,
                  height: _spanHeight(w, rows) * cellH,
                  child: RackWidget(
                    widget: w,
                    params: params,
                    analysisNodes: analysisNodes,
                    onMoved: (col, row) =>
                        ref.read(projectProvider.notifier).moveWidget(w.id, col, row),
                    onEdited: () => _editWidget(w, params, analysisNodes),
                    onAssignParam: (paramId) =>
                        ref.read(projectProvider.notifier).assignParam(w.id, paramId),
                    onDelete: () =>
                        ref.read(projectProvider.notifier).removeWidget(w.id),
                  ),
                ),
            ],
          );
        },
      ),
    );

    return Container(
      color: const Color(0xFF15161B),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            child: Align(
              alignment: Alignment.centerLeft,
              child: _HeaderChip('Interface Plugin (GUI Designer Grid)'),
            ),
          ),
          // Palette drag-and-drop des widgets GUI (miroir GUI_REGISTRY).
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            child: Row(
              children: [
                for (final (type, name, _) in GuiRegistry.widgets)
                  Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: Draggable<(String, Offset)>(
                      data: (type, Offset.zero),
                      feedback: MouseRegion(
                        cursor: SystemMouseCursors.grabbing,
                        child: Material(
                          color: Colors.transparent,
                          child: _PaletteChip(label: name, dragging: true),
                        ),
                      ),
                      dragAnchorStrategy: pointerDragAnchorStrategy,
                      child: _PaletteChip(label: name, dragging: false),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 8),
          // Titre du rack (plugin-title de la maquette : orange, centré).
          Text(
            '${spec.meta.name.toUpperCase()}.CLAP',
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 13, fontWeight: FontWeight.w700, color: accentOrange),
          ),
          const SizedBox(height: 10),
          Expanded(child: Center(child: rack)),
        ],
      ),
    );
  }

  Future<void> _editWidget(
    GuiWidget w,
    Map<String, (NodeInstance, ParamDef)> params,
    Map<String, String> analysisNodes,
  ) async {
    final result = await showDialog<_WidgetEdit>(
      context: context,
      builder: (_) => _EditDialog(widget: w, params: params, analysisNodes: analysisNodes),
    );
    if (result == null) return;
    final notifier = ref.read(projectProvider.notifier);
    notifier.assignParam(w.id, result.paramId);
    if (result.label != w.label) {
      final widgets = [...ref.read(projectProvider).spec.guiWidgets];
      final i = widgets.indexWhere((x) => x.id == w.id);
      if (i >= 0) {
        final old = widgets[i];
        widgets[i] = GuiWidget(
          id: old.id, widgetType: old.widgetType, label: result.label,
          paramId: old.paramId, col: old.col, row: old.row,
          colSpan: old.colSpan, rowSpan: old.rowSpan,
        );
        // Injection directe via le modèle du state :
        final spec = ref.read(projectProvider).spec;
        spec.guiWidgets
          ..clear()
          ..addAll(widgets);
      }
    }
  }
}

class _WidgetEdit {
  final String paramId;
  final String label;
  const _WidgetEdit(this.paramId, this.label);
}

bool _isAnalysisNode(String nodeType) =>
    const ['peak_rms_meter', 'oscilloscope', 'lufs_meter', 'goniometer', 'fft_analyzer']
        .contains(nodeType);

/// Un widget contrôleur a besoin d'un paramètre CLAP exposé.
bool seedParamCapable(String type) => GuiRegistry.paramCapable(type);

int _spanWidth(GuiWidget w, int cols) => w.colSpan.clamp(1, cols - w.col);
int _spanHeight(GuiWidget w, int rows) => w.rowSpan.clamp(1, rows - w.row);

void setStatus(WidgetRef ref, String v) {
  // Injection douce dans le footer global (défini dans main.dart).
  // On passe par un Provider nommé pour éviter une dépendance circulaire.
  ref.read(clapsStatusProvider.notifier).set(v);
}

Map<String, (NodeInstance, ParamDef)> exposedParamsMap(GraphSpec spec) {
  final out = <String, (NodeInstance, ParamDef)>{};
  for (final n in spec.nodes) {
    for (final p in n.exposedParams) {
      out['${n.id}/${p.paramId}'] = (n, p);
    }
  }
  return out;
}

class _GridPainter extends CustomPainter {
  final int cols, rows;
  final double cellW, cellH;
  final bool highlight;

  const _GridPainter({
    required this.cols,
    required this.rows,
    required this.cellW,
    required this.cellH,
    required this.highlight,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..strokeWidth = 1.0
      ..color = highlight
          ? borderColor.withValues(alpha: 0.8)
          : borderColor.withValues(alpha: 0.5);
    for (int c = 0; c <= cols; c++) {
      canvas.drawLine(Offset(c * cellW, 0), Offset(c * cellW, rows * cellH), paint);
    }
    for (int r = 0; r <= rows; r++) {
      canvas.drawLine(Offset(0, r * cellH), Offset(cols * cellW, r * cellH), paint);
    }
  }

  @override
  bool shouldRepaint(covariant _GridPainter oldDelegate) =>
      oldDelegate.highlight != highlight || oldDelegate.cols != cols;
}

class _HeaderChip extends StatelessWidget {
  final String label;

  const _HeaderChip(this.label);

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.topLeft,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        decoration: BoxDecoration(
          color: bgPanel.withValues(alpha: 0.85),
          border: Border.all(color: borderColor),
          borderRadius: BorderRadius.circular(4),
        ),
        child: Text(label, style: const TextStyle(fontSize: 12, color: textMuted)),
      ),
    );
  }
}

/// Widget du rack : rendu selon type, interactions (drag déplacement,
/// double-clic édition, mapping param, delete).
class RackWidget extends ConsumerStatefulWidget {
  final GuiWidget widget;
  final Map<String, (NodeInstance, ParamDef)> params;
  final Map<String, String> analysisNodes;
  final void Function(int col, int row) onMoved;
  final VoidCallback onEdited;
  final void Function(String paramId) onAssignParam;
  final VoidCallback onDelete;

  const RackWidget({
    super.key,
    required this.widget,
    required this.params,
    required this.analysisNodes,
    required this.onMoved,
    required this.onEdited,
    required this.onAssignParam,
    required this.onDelete,
  });

  @override
  ConsumerState<RackWidget> createState() => _RackWidgetState();
}

class _RackWidgetState extends ConsumerState<RackWidget> {
  RackPos? dragOriginCell;
  Offset? pointerOrigin;

  @override
  Widget build(BuildContext context) {
    final w = widget.widget;
    return GestureDetector(
      onDoubleTap: widget.onEdited,
      // Déplacement absolu par le pointeur : cellule finale = cellule de
      // départ + (curseur − curseur départ), fidèle à la souris.
      onPanStart: (details) {
        dragOriginCell = RackPos(w.col, w.row);
        pointerOrigin = details.globalPosition;
      },
      onPanUpdate: (details) {
        if (dragOriginCell == null || pointerOrigin == null) return;
        final d = details.globalPosition - pointerOrigin!;
        final target = (dragOriginCell!.col + (d.dx / 40.0).round()).clamp(0, 7);
        final targetRow = (dragOriginCell!.row + (d.dy / 40.0).round()).clamp(0, 9);
        widget.onMoved(target, targetRow);
      },
      child: MouseRegion(
        cursor: SystemMouseCursors.grab,
        child: Container(
          margin: const EdgeInsets.all(2),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: accentBlue.withValues(alpha: 0.5)),
            color: const Color(0x0829B6F6),
          ),
          child: Column(
            children: [
              const SizedBox(height: 2),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Expanded(
                    child: Text(w.label, overflow: TextOverflow.ellipsis,
                        textAlign: TextAlign.center,
                        style: const TextStyle(fontSize: 10, color: textMuted)),
                  ),
                  if (GuiRegistry.paramCapable(w.widgetType))
                    PopupMenuButton<String>(
                      icon: const Icon(Icons.link, size: 12, color: accentOrange),
                      tooltip: 'Mapper le paramètre CLAP',
                      onSelected: widget.onAssignParam,
                      itemBuilder: (_) => [
                        if (widget.params.isEmpty)
                          const PopupMenuItem<String>(
                            enabled: false,
                            height: 32,
                            child: Text('Aucun param exposé (exposez un nœud)',
                                style: TextStyle(fontSize: 11)),
                          ),
                        for (final entry in widget.params.entries)
                          PopupMenuItem<String>(
                            value: entry.key,
                            height: 32,
                            child: Text(
                                '${entry.value.$2.name} (${entry.key})',
                                style: const TextStyle(fontSize: 12)),
                          ),
                      ],
                    ),
                  InkWell(onTap: widget.onEdited,
                    child: const Icon(Icons.edit, size: 12, color: textMuted),
                  ),
                  InkWell(onTap: widget.onDelete,
                    child: const Icon(Icons.close, size: 12, color: textMuted),
                  ),
                ],
              ),
              Expanded(child: _renderBody()),
            ],
          ),
        ),
      ),
    );
  }



  Widget _renderBody() {
    final w = widget.widget;
    switch (w.widgetType) {
      case 'knob':
        return const Center(child: _KnobGlyph());
      case 'slider':
        return const Center(child: _SliderGlyph());
      case 'vu_meter':
        return const Center(child: _VuMeterGlyph());
      case 'toggle':
        return const Center(child: Icon(Icons.toggle_on_rounded, size: 30, color: accentGreen));
      case 'value_display':
        return const Center(child: _ValueDisplayGlyph());
      case 'waveform_display':
        return const Center(child: _WaveformGlyph());
      case 'text_logo':
        return Center(
          child: Text(w.label.isEmpty ? 'PLUGIN' : w.label.toUpperCase(),
              style: const TextStyle(
                  color: accentOrange, fontSize: 16, fontWeight: FontWeight.bold)),
        );
      case 'piano_keyboard':
        return const Center(child: Icon(Icons.piano, color: textMuted));
      case 'xy_pad':
        return const Center(child: Icon(Icons.control_camera_rounded, color: textMuted, size: 28));
      case 'panel_group':
        return const Center(child: Icon(Icons.crop_din_outlined, color: textMuted));
      case 'tab_container':
        return const Center(child: Icon(Icons.tab_rounded, color: textMuted));
      case 'curve_editor':
        return const Center(child: Icon(Icons.insights_rounded, color: textMuted));
      case 'dropdown':
        return const Center(child: Icon(Icons.expand_more, color: textMuted));
      default:
        return const Center(child: Icon(Icons.help_outline, color: textMuted));
    }
  }
}

/// Dialogue d'édition widget : label + cible (param exposé ou nœud d'analyse).
class _EditDialog extends StatefulWidget {
  final GuiWidget widget;
  final Map<String, (NodeInstance, ParamDef)> params;
  final Map<String, String> analysisNodes;

  const _EditDialog({
    required this.widget,
    required this.params,
    required this.analysisNodes,
  });

  @override
  State<_EditDialog> createState() => _EditDialogState();
}

class _EditDialogState extends State<_EditDialog> {
  late final TextEditingController _label =
      TextEditingController(text: widget.widget.label);
  String? _target;

  @override
  void initState() {
    super.initState();
    _target = widget.widget.paramId.isEmpty ? null : widget.widget.paramId;
  }

  @override
  Widget build(BuildContext context) {
    final w = widget.widget;
    final isViewer = GuiRegistry.isViewer(w.widgetType);
    final options = <String, String>{
      // key = paramId ou nodeId ; value = libellé affiché
      for (final e in widget.params.entries)
        e.key : 'param : ${e.value.$2.name} (${e.key})',
      for (final e in widget.analysisNodes.entries)
        e.key : 'mesure : ${e.value} (${'${e.key}/'})',
    };
    return AlertDialog(
      backgroundColor: bgPanel,
      title: Text('Éditer ${w.widgetType}',
          style: const TextStyle(fontSize: 16)),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              controller: _label,
              decoration: const InputDecoration(labelText: 'Label'),
            ),
            const SizedBox(height: 12),
            if (options.isEmpty)
              Text(
                isViewer
                    ? 'Aucun nœud d\'analyse dans le graphe (ajoutez peak_rms_meter…).'
                    : 'Aucun paramètre CLAP exposé : double-cliquez d\'abord un nœud\npour en exposer.',
                style: const TextStyle(fontSize: 11, color: textMuted),
              )
            else ...[
              const Text('Cible',
                  style: TextStyle(fontSize: 11, color: textMuted)),
              RadioGroup<String>(
                groupValue: _target ?? '',
                onChanged: (v) => setState(() => _target = v),
                child: Column(
                  children: [
                    for (final e in options.entries)
                      RadioListTile<String>(
                        value: e.key,
                        dense: true,
                        title: Text(e.value,
                            style: const TextStyle(fontSize: 12)),
                      ),
                  ],
                ),
              ),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
            onPressed: () => Navigator.pop(context), child: const Text('Annuler')),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: accentOrange),
          onPressed: () {
            Navigator.pop(context, _WidgetEdit(_target ?? '', _label.text));
          },
          child: const Text('Appliquer'),
        ),
      ],
    );
  }
}

/// Pastille de palette (drag-and-drop des widgets GUI).
class _PaletteChip extends StatelessWidget {
  final String label;
  final bool dragging;

  const _PaletteChip({required this.label, required this.dragging});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: dragging ? bgPanel : const Color(0xFF222530),
        border: Border.all(
            color: dragging ? accentPurple : borderColor, width: dragging ? 1.4 : 1),
        borderRadius: BorderRadius.circular(4),
        boxShadow: dragging ? const [BoxShadow(blurRadius: 8, color: Colors.black26)] : null,
      ),
      child: Text(label, style: const TextStyle(fontSize: 11)),
    );
  }
}

// ---------------------------------------------------------------- glyphs --

class _KnobGlyph extends StatelessWidget {
  const _KnobGlyph();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 34,
      height: 34,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        border: Border.all(color: textMain, width: 3),
      ),
      child: const Padding(
        padding: EdgeInsets.all(3),
        child: DecoratedBox(
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            border: Border.fromBorderSide(BorderSide(color: accentOrange, width: 2)),
          ),
        ),
      ),
    );
  }
}

class _SliderGlyph extends StatelessWidget {
  const _SliderGlyph();

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Stack(
          alignment: const Alignment(0, 0),
          children: [
            Container(width: 34, height: 6, color: const Color(0xFF3A3E4E)),
            Align(
              alignment: const Alignment(-0.4, 0),
              child: Container(width: 12, height: 16, color: accentOrange),
            ),
          ],
        ),
      ],
    );
  }
}

class _VuMeterGlyph extends StatelessWidget {
  const _VuMeterGlyph();

  @override
  Widget build(BuildContext context) {
    return FittedBox(
      fit: BoxFit.scaleDown,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          for (final (i, lit) in [true, true, true, true, true, false, false, false].indexed)
            Container(
              width: 3,
              height: 4 + i * 0.8,
              margin: const EdgeInsets.symmetric(horizontal: 1),
              color: lit ? accentGreen : const Color(0xFF3A3E4E),
            ),
        ],
      ),
    );
  }
}

class _ValueDisplayGlyph extends StatelessWidget {
  const _ValueDisplayGlyph();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      color: Colors.black,
      child: const Text(
        '-12.4 dB',
        style: TextStyle(fontFamily: 'monospace', fontSize: 12, color: accentGreen),
      ),
    );
  }
}

class _WaveformGlyph extends StatelessWidget {
  const _WaveformGlyph();

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      size: const Size(60, 34),
      painter: _WavePainter(),
    );
  }
}

class _WavePainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = accentBlue
      ..strokeWidth = 1.4
      ..style = PaintingStyle.stroke;
    final path = Path();
    for (int i = 0; i <= 24; i++) {
      final x = i / 24 * size.width;
      final y = size.height / 2 + (i % 2 == 0 ? 9 : -9) * 0.6;
      if (i == 0) {
        path.moveTo(x, y);
      } else {
        path.lineTo(x, y);
      }
    }
    canvas.drawPath(path, paint);
  }

  @override
  bool shouldRepaint(covariant _WavePainter oldDelegate) => false;
}
