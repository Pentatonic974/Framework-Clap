/// Carte de nœud déplacable : widget À ÉTAT. Les origines de drag vivent
/// dans le State (pas dans build() — un sans état écrasait la capture du
/// geste à chaque rebuild, d'où « le module ne bouge pas »).
///
/// Mécanisme : onPanStart capture (position nœud, position curseur) dans
/// le State ; onPanUpdate = origine + (globalPosition − origine) : suivi
/// absolu fidèle, reconstruit via onMoved à l'extérieur (Riverpod).
library;

import 'dart:math' as math;

import 'package:flutter/gestures.dart' show kPrimaryMouseButton;
import 'package:flutter/material.dart';

import '../../core/app_theme.dart';
import '../../core/node_registry.dart';
import '../../core/project_model.dart';

const kNodeWidth = 180.0;
const kPortGap = 20.0;

class DraggableNodeCard extends StatefulWidget {
  final NodeInstance node;
  final NodeDef? def;
  final PortKind? highlightKind;
  final ValueChanged<NodePos> onMoved;
  final void Function(int port, bool isOutput) onPortTap;
  final VoidCallback onEdit;
  final VoidCallback onDelete;

  const DraggableNodeCard({
    super.key,
    required this.node,
    required this.def,
    required this.highlightKind,
    required this.onMoved,
    required this.onPortTap,
    required this.onEdit,
    required this.onDelete,
  });

  @override
  State<DraggableNodeCard> createState() => _DraggableNodeCardState();
}

class _DraggableNodeCardState extends State<DraggableNodeCard> {
  /// Position du nœud à la prise du geste (référence stable pendant le drag).
  NodePos? _originPos;
  /// Position curseur à la prise du geste.
  Offset? _originPointer;
  bool _dragging = false;

  void _endDrag() {
    _dragging = false;
    _originPos = null;
    _originPointer = null;
  }

  @override
  Widget build(BuildContext context) {
    final d = widget.def;
    if (d == null) return const SizedBox.shrink();
    final rows = math.max(d.inputs.length, d.outputs.length);
    final height = 34.0 + math.max(rows, 1) * kPortGap;

    // Listener : saisie au pixel (aucun touch slop de ~20 px), le module
    // suit la souris DS LE premier déplacement. Le double-clic d'édition
    // reste géré par le GestureDetector interne.
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: (e) {
        if (e.buttons != kPrimaryMouseButton) return;
        _dragging = true;
        _originPos = NodePos(widget.node.pos.dx, widget.node.pos.dy);
        _originPointer = e.position;
      },
      onPointerMove: (e) {
        if (!_dragging || _originPos == null || _originPointer == null) return;
        final delta = e.position - _originPointer!;
        widget.onMoved(
            NodePos(_originPos!.dx + delta.dx, _originPos!.dy + delta.dy));
      },
      onPointerUp: (_) => _endDrag(),
      onPointerCancel: (_) => _endDrag(),
      child: GestureDetector(
      onDoubleTap: widget.onEdit,
      child: Container(
        width: kNodeWidth,
        height: height,
        decoration: BoxDecoration(
          color: bgPanel,
          border: Border.all(color: borderColor),
          borderRadius: BorderRadius.circular(6),
          boxShadow: const [BoxShadow(blurRadius: 12, color: Colors.black54)],
        ),
        child: Column(
          children: [
            Container(
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
                  InkWell(
                    onTap: widget.onEdit,
                    child: const Icon(Icons.edit, size: 13, color: textMuted),
                  ),
                  InkWell(
                    onTap: widget.onDelete,
                    child: const Icon(Icons.close, size: 14, color: textMuted),
                  ),
                ],
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
                              highlighted: widget.highlightKind != null &&
                                  port.kind == widget.highlightKind,
                              onTap: () => widget.onPortTap(i, false),
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
                              onTap: () => widget.onPortTap(i, true),
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
      ),
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
          margin: highlighted ? const EdgeInsets.all(1) : EdgeInsets.zero,
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

    return SizedBox(height: kPortGap, child: Align(child: row));
  }
}
