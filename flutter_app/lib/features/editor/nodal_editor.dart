/// Éditeur nodal : câblage (CustomPainter), palette drag-and-drop,
/// nœuds déplaçables, connexion interactive sortie -> entrée.
library;

import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/app_theme.dart';
import '../../core/node_registry.dart';
import '../../core/project_model.dart';
import 'editor_state.dart';

const _nodeWidth = 180.0;
const _portGap = 20.0;

/// Résultat du câblage en attente : (id nœud, index port, type port).
class PendingWire {
  final String nodeId;
  final int port;
  final PortKind kind;
  const PendingWire(this.nodeId, this.port, this.kind);
}

class NodalEditor extends ConsumerStatefulWidget {
  const NodalEditor({super.key});

  @override
  ConsumerState<NodalEditor> createState() => _NodalEditorState();
}

class _NodalEditorState extends ConsumerState<NodalEditor> {
  PendingWire? pendingOutput;

  NodeDef? _def(String? type) => type == null ? null : ClapNodeRegistry.byType(type);

  void _onPortTap(NodeInstance node, int portIndex, bool isOutput) {
    final notifier = ref.read(projectProvider.notifier);
    final pending = pendingOutput;
    if (isOutput) {
      final def = _def(node.nodeType);
      if (def == null) return;
      setState(() => pendingOutput =
          PendingWire(node.id, portIndex, def.outputs[portIndex].kind));
      return;
    }
    if (pending == null) return;
    final toDef = _def(node.nodeType);
    final fromDef = _def(ref.read(projectProvider).spec.node(pending.nodeId)?.nodeType);
    if (toDef == null || fromDef == null) return;
    if (toDef.inputs[portIndex].kind == pending.kind && pending.nodeId != node.id) {
      notifier.connect(Wire(
          fromNode: pending.nodeId,
          fromPort: pending.port,
          toNode: node.id,
          toPort: portIndex));
    }
    setState(() => pendingOutput = null);
  }

  @override
  Widget build(BuildContext context) {
    final spec = ref.watch(projectProvider).spec;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const _Palette(),
        VerticalDivider(width: 1, color: borderColor),
        Expanded(
          child: DragTarget<String>(
            onWillAcceptWithDetails: (_) => true,
            onAcceptWithDetails: (details) {
              final nodeType = details.data;
              ref
                  .read(projectProvider.notifier)
                  .addNode(nodeType, const NodePos(340, 230));
            },
            builder: (context, candidates, _) {
              final highlight = candidates.isNotEmpty;
              return Container(
                color: highlight
                    ? Color.lerp(bgCanvas, accentOrange, 0.06)
                    : bgCanvas,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    // motif pointillé de fond
                    const Positioned.fill(child: _DottedBackground()),
                    Positioned(
                      top: 10,
                      left: 10,
                      right: 10,
                      child: _HeaderChip(
                        pendingOutput == null
                            ? 'Graphe Nodal (DSP & Control Flow)'
                            : 'Câblage : cliquez le port d\u2019entrée cible…',
                      ),
                    ),
                    // câbles
                    CustomPaint(
                      size: Size.infinite,
                      painter: _WiresPainter(spec: spec, defOf: _def),
                    ),
                    // nœuds
                    for (final n in spec.nodes)
                      Positioned(
                        left: n.pos.dx,
                        top: n.pos.dy,
                        child: _NodeCard(
                          node: n,
                          def: _def(n.nodeType),
                          highlightKind:
                              pendingOutput?.kind, // entrées compatibles surlignées
                          onMoved: (pos) =>
                              ref.read(projectProvider.notifier).moveNode(n.id, pos),
                          onPortTap: (port, isOutput) => _onPortTap(n, port, isOutput),
                          onDelete: () =>
                              ref.read(projectProvider.notifier).removeNode(n.id),
                        ),
                      ),
                  ],
                ),
              );
            },
          ),
        ),
      ],
    );
  }
}

/// Fond pointillé (réplique du radial-gradient 20px de la maquette).
class _DottedBackground extends StatelessWidget {
  const _DottedBackground();

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      isComplex: true,
      willChange: false,
      painter: _DotsPainter(),
      child: const SizedBox.expand(),
    );
  }
}

class _DotsPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = const Color(0xFF222530);
    for (double x = 0; x < size.width + 20; x += 20) {
      for (double y = 0; y < size.height + 20; y += 20) {
        canvas.drawCircle(Offset(x, y), 1, paint);
      }
    }
  }

  @override
  bool shouldRepaint(covariant _DotsPainter oldDelegate) => false;
}

/// Palette latérale par catégories, drag-and-drop vers le canvas.
class _Palette extends ConsumerWidget {
  const _Palette();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final categories = ClapNodeRegistry.categories();
    return Container(
      width: 250,
      color: bgPanel,
      child: ListView(
        padding: const EdgeInsets.all(12),
        children: [
          const Text('BIBLIOTHÈQUE NODALE',
              style: TextStyle(fontSize: 12, color: textMuted)),
          const SizedBox(height: 6),
          for (final cat in categories)
            _CategoryGroup(
              category: cat,
              nodes: [
                for (final d in ClapNodeRegistry.defs)
                  if (d.category == cat) d,
              ],
            ),
        ],
      ),
    );
  }
}

class _CategoryGroup extends StatelessWidget {
  final String category;
  final List<NodeDef> nodes;

  const _CategoryGroup({required this.category, required this.nodes});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const SizedBox(height: 10),
        Text(
          category.toUpperCase(),
          style: const TextStyle(
              fontWeight: FontWeight.bold,
              fontSize: 11,
              color: accentBlue,
              letterSpacing: 0.8),
        ),
        Container(
            height: 1, color: borderColor, margin: const EdgeInsets.only(bottom: 6)),
        for (final d in nodes)
          Draggable<String>(
            key: ValueKey('palette_${d.nodeType}'),
            data: d.nodeType,
            feedback: _PaletteItem(def: d, elevated: true),
            childWhenDragging: Opacity(opacity: 0.4, child: _PaletteItem(def: d, elevated: false)),
            child: _PaletteItem(def: d, elevated: false),
          ),
      ],
    );
  }
}

class _PaletteItem extends StatelessWidget {
  final NodeDef def;
  final bool elevated;

  const _PaletteItem({required this.def, required this.elevated});

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 4),
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
      decoration: BoxDecoration(
        color: elevated ? bgPanel : const Color(0xFF222530),
        border: Border.all(
            color: elevated ? accentOrange : borderColor,
            width: elevated ? 1.4 : 1),
        borderRadius: BorderRadius.circular(4),
        boxShadow: elevated
            ? const [BoxShadow(blurRadius: 8, color: Colors.black38)]
            : null,
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(def.displayName, style: const TextStyle(fontSize: 12)),
          ),
          const Text('Nœud', style: TextStyle(fontSize: 9, color: textMuted)),
        ],
      ),
    );
  }
}

/// Peinture des câbles : bezier cubique entre ports alignés.
class _WiresPainter extends CustomPainter {
  final GraphSpec spec;
  final NodeDef? Function(String?) defOf;

  const _WiresPainter({required this.spec, required this.defOf});

  @override
  void paint(Canvas canvas, Size size) {
    final wireColor = Paint()
      ..strokeWidth = 2.0
      ..style = PaintingStyle.stroke
      ..color = accentBlue;
    final glowPaint = Paint()
      ..strokeWidth = 5.0
      ..color = const Color(0x1F29B6F6)
      ..style = PaintingStyle.stroke;

    for (final w in spec.connections) {
      final from = spec.node(w.fromNode);
      final to = spec.node(w.toNode);
      if (from == null || to == null) continue;
      final start = _portPosition(from, w.fromPort, true);
      final end = _portPosition(to, w.toPort, false);
      if (start == null || end == null) continue;
      final cp = (start.dx - end.dx).abs() / 2 + 24;
      final path = Path()
        ..moveTo(start.dx, start.dy)
        ..cubicTo(start.dx + cp, start.dy, end.dx - cp, end.dy, end.dx, end.dy);
      canvas.drawPath(path, glowPaint);
      canvas.drawPath(path, wireColor);
      canvas.drawCircle(start, 4, Paint()..color = accentOrange);
      canvas.drawCircle(end, 4, Paint()..color = accentBlue);
    }
  }

  Offset? _portPosition(NodeInstance n, int port, bool output) {
    final def = defOf(n.nodeType);
    if (def == null) return null;
    final y = n.pos.dy + _portYFor(def, port, output);
    final x = output ? n.pos.dx + _nodeWidth : n.pos.dx;
    return Offset(x, y);
  }

  double _portYFor(NodeDef def, int port, bool output) {
    final ports = output ? def.outputs : def.inputs;
    if (port >= ports.length) return 30;
    return 30.0 + _portGap * port;
  }

  @override
  bool shouldRepaint(covariant _WiresPainter oldDelegate) =>
      oldDelegate.spec != spec;
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

/// Carte de nœud : en-tête déplacable (Pan), ports cliquables, suppression.
class _NodeCard extends StatelessWidget {
  final NodeInstance node;
  final NodeDef? def;
  final PortKind? highlightKind;
  final ValueChanged<NodePos> onMoved;
  final void Function(int port, bool isOutput) onPortTap;
  final VoidCallback onDelete;

  const _NodeCard({
    required this.node,
    required this.def,
    required this.highlightKind,
    required this.onMoved,
    required this.onPortTap,
    required this.onDelete,
  });

  @override
  Widget build(BuildContext context) {
    final d = def;
    if (d == null) return const SizedBox();
    final rows = math.max(d.inputs.length, d.outputs.length);
    final height = 34.0 + math.max(rows, 1) * _portGap;
    return Container(
      width: _nodeWidth,
      height: height,
      decoration: BoxDecoration(
        color: bgPanel,
        border: Border.all(color: borderColor),
        borderRadius: BorderRadius.circular(6),
        boxShadow: const [BoxShadow(blurRadius: 12, color: Colors.black54)],
      ),
      child: Column(
        children: [
          GestureDetector(
            behavior: HitTestBehavior.opaque,
            onPanUpdate: (delta) {
              onMoved(NodePos(node.pos.dx + delta.delta.dx,
                  node.pos.dy + delta.delta.dy));
            },
            child: Container(
              height: 24,
              padding: const EdgeInsets.symmetric(horizontal: 8),
              decoration: const BoxDecoration(
                color: Color(0xFF252834),
                borderRadius: BorderRadius.only(
                    topLeft: Radius.circular(6), topRight: Radius.circular(6)),
              ),
              child: Row(
                children: [
                  Expanded(
                    child: Text(d.displayName,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                            fontWeight: FontWeight.w600, fontSize: 11)),
                  ),
                  InkWell(onTap: onDelete,
                    child: const Icon(Icons.close, size: 14, color: textMuted),
                  ),
                ],
              ),
            ),
          ),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        for (final (i, port) in d.inputs.indexed)
                          _PortDot(
                            label: port.name,
                            kind: port.kind,
                            isOutput: false,
                            highlighted:
                                highlightKind != null && port.kind == highlightKind,
                            onTap: () => onPortTap(i, false),
                          ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.end,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        for (final (i, port) in d.outputs.indexed)
                          _PortDot(
                            label: port.name,
                            kind: port.kind,
                            isOutput: true,
                            highlighted: false,
                            onTap: () => onPortTap(i, true),
                          ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Port cliquable : dot coloré + libellé (couleur = type de signal).
class _PortDot extends StatelessWidget {
  final String label;
  final PortKind kind;
  final bool isOutput;
  final bool highlighted;
  final VoidCallback onTap;

  const _PortDot({
    required this.label,
    required this.kind,
    required this.isOutput,
    required this.highlighted,
    required this.onTap,
  });

  static Color colorFor(PortKind kind) {
    switch (kind) {
      case PortKind.audio:
        return accentOrange;
      case PortKind.control:
        return accentBlue;
      case PortKind.event:
        return accentPurple;
      case PortKind.data:
        return accentGreen;
    }
  }

  @override
  Widget build(BuildContext context) {
    final dot = GestureDetector(
      onTap: onTap,
      child: MouseRegion(
        cursor: SystemMouseCursors.precise,
        child: Container(
          width: 10,
          height: 10,
          margin: highlighted
              ? const EdgeInsets.all(1)
              : EdgeInsets.zero,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: colorFor(kind),
            border: highlighted
                ? Border.all(color: Colors.white, width: 1.2)
                : null,
          ),
        ),
      ),
    );

    final label = Expanded(
      child: Text(this.label,
          overflow: TextOverflow.ellipsis,
          textAlign: isOutput ? TextAlign.right : TextAlign.left,
          style: const TextStyle(fontSize: 10, color: textMuted)),
    );

    final row = isOutput
        ? Row(children: [label, const SizedBox(width: 4), dot])
        : Row(children: [dot, const SizedBox(width: 4), label]);

    return SizedBox(height: _portGap, child: Align(child: row));
  }
}
