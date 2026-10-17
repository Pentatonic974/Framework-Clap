/// Designer de grille GUI : rack du plugin, widgets posés sur une grille,
/// mapping dynamique vers les identifiants de paramètres CLAP.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/app_theme.dart';
import '../../core/node_registry.dart';
import '../../core/project_model.dart';
import '../editor/editor_state.dart';

/// Éditeur de rack : encadré 320x420, widgets drag-and-drop, édition du label
/// et du param mappé. Les widgets pointent vers "nodeId/paramId" quand ils
/// deviennent un contrôle, ou vers nodeId pour la mesure.
class GridDesigner extends ConsumerStatefulWidget {
  const GridDesigner({super.key});

  @override
  ConsumerState<GridDesigner> createState() => _GridDesignerState();
}

class _GridDesignerState extends ConsumerState<GridDesigner> {
  @override
  Widget build(BuildContext context) {
    final spec = ref.watch(projectProvider).spec;
    final params = exposedParamsMap(spec);
    const cellW = 40.0;
    const cellH = 40.0;
    const cols = 8;
    const rows = 10;

    final rack = SizedBox(
      width: cols * cellW,
      height: rows * cellH,
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          const _GridBackground(cols: cols, rows: rows, cellW: cellW, cellH: cellH),
          for (final w in spec.guiWidgets)
            Positioned(
              left: w.col * cellW,
              top: w.row * cellH,
              width: _spanWidth(w, cols) * cellW,
              height: _spanHeight(w, rows) * cellH,
              child: RackWidget(
                widget: w,
                params: params,
                onMoved: (col, row) =>
                    ref.read(projectProvider.notifier).moveWidget(w.id, col, row),
                onAssignParam: (paramId) =>
                    ref.read(projectProvider.notifier).assignParam(w.id, paramId),
                onDelete: () =>
                    ref.read(projectProvider.notifier).removeWidget(w.id),
              ),
            ),
        ],
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
                    child: Draggable<String>(
                      data: type,
                      feedback: Material(
                        color: Colors.transparent,
                        child: _PaletteChip(label: name, dragging: true),
                      ),
                      child: _PaletteChip(label: name, dragging: false),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 8),
          // Titre du rack (plugin-title de la maquette : orange, centré).
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                '${spec.meta.name.toUpperCase()}.CLAP',
                textAlign: TextAlign.center,
                style: const TextStyle(
                    fontSize: 13, fontWeight: FontWeight.w700, color: accentOrange),
              ),
              const SizedBox(height: 10),
              rack,
            ],
          ),
        ],
      ),
    );
  }
}

int _spanWidth(GuiWidget w, int cols) => w.colSpan.clamp(1, cols - w.col);
int _spanHeight(GuiWidget w, int rows) => w.rowSpan.clamp(1, rows - w.row);

Map<String, (NodeInstance, ParamDef)> exposedParamsMap(GraphSpec spec) {
  final out = <String, (NodeInstance, ParamDef)>{};
  for (final n in spec.nodes) {
    for (final p in n.exposedParams) {
      out['${n.id}/${p.paramId}'] = (n, p);
    }
  }
  return out;
}

class _GridBackground extends StatelessWidget {
  final int cols, rows;
  final double cellW, cellH;

  const _GridBackground(
      {required this.cols, required this.rows, required this.cellW, required this.cellH});

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: cols * cellW,
      height: rows * cellH,
      child: CustomPaint(
        painter: _GridPainter(cols: cols, rows: rows, cellW: cellW, cellH: cellH),
        child: const SizedBox.expand(),
      ),
    );
  }
}

class _GridPainter extends CustomPainter {
  final int cols, rows;
  final double cellW, cellH;

  const _GridPainter({required this.cols, required this.rows, required this.cellW, required this.cellH});

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..strokeWidth = 1.0
      ..color = borderColor.withValues(alpha: 0.5);
    for (int c = 0; c <= cols; c++) {
      canvas.drawLine(Offset(c * cellW, 0), Offset(c * cellW, rows * cellH), paint);
    }
    for (int r = 0; r <= rows; r++) {
      canvas.drawLine(Offset(0, r * cellH), Offset(cols * cellW, r * cellH), paint);
    }
  }

  @override
  bool shouldRepaint(covariant _GridPainter oldDelegate) =>
      oldDelegate.cols != cols || oldDelegate.cellW != cellW;
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

/// Widget du rack : rendu selon type, interactions (drag, mapping, delete).
class RackWidget extends ConsumerStatefulWidget {
  final GuiWidget widget;
  final Map<String, (NodeInstance, ParamDef)> params;
  final void Function(int col, int row) onMoved;
  final void Function(String paramId) onAssignParam;
  final VoidCallback onDelete;

  const RackWidget({
    super.key,
    required this.widget,
    required this.params,
    required this.onMoved,
    required this.onAssignParam,
    required this.onDelete,
  });

  @override
  ConsumerState<RackWidget> createState() => _RackWidgetState();
}

class _RackWidgetState extends ConsumerState<RackWidget> {

  @override
  Widget build(BuildContext context) {
    final w = widget.widget;
    return GestureDetector(
      onPanUpdate: (delta) {
        const cell = 40.0;
        final target = ((w.col + delta.delta.dx / cell).round()).clamp(0, 7);
        final targetRow = ((w.row + delta.delta.dy / cell).round()).clamp(0, 9);
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
                  PopupMenuButton<String>(
                    icon: const Icon(Icons.link, size: 12, color: accentOrange),
                    tooltip: 'Mapper le paramètre CLAP',
                    onSelected: widget.onAssignParam,
                    itemBuilder: (_) => [
                      for (final entry in widget.params.entries)
                        PopupMenuItem<String>(
                          value: entry.key,
                          height: 32,
                          child: Text(
                              '${entry.value.$2.name} (${entry.value.$1.id})',
                              style: const TextStyle(fontSize: 12)),
                        ),
                    ],
                  ),
                  InkWell(onTap: widget.onDelete,
                    child: const Icon(Icons.close, size: 12, color: textMuted),
                  ),
                ],
              ),
              Expanded(child: _renderBody())
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
        return Center(child: Icon(Icons.toggle_on_rounded, size: 30, color: accentGreen));
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
    return Row(
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
