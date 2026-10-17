/// Split horizontal redimensionnable : panneau gauche + poignée + panneau
/// droite. La largeur du panneau gauche est bornée (fraction de l'écran).
library;

import 'package:flutter/material.dart';

class ResizableSplit extends StatefulWidget {
  final Widget left, right;
  /// Fraction initiale du panneau gauche (0..1).
  final double initialFraction;
  /// Bornes de la fraction.
  final double minFraction, maxFraction;

  const ResizableSplit({
    super.key,
    required this.left,
    required this.right,
    this.initialFraction = 0.6,
    this.minFraction = 0.3,
    this.maxFraction = 0.8,
  });

  @override
  State<ResizableSplit> createState() => _ResizableSplitState();
}

class _ResizableSplitState extends State<ResizableSplit> {
  late double _fraction = widget.initialFraction;
  double? _dragStartFraction;
  Offset? _dragStartPointer;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, constraints) {
      final width = constraints.maxWidth;
      final leftWidth = (width * _fraction).clamp(240.0, width - 240.0);
      final handle = MouseRegion(
        cursor: SystemMouseCursors.resizeLeftRight,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onPanStart: (d) {
            _dragStartFraction = _fraction;
            _dragStartPointer = d.globalPosition;
          },
          onPanUpdate: (d) {
            if (_dragStartFraction == null || _dragStartPointer == null) return;
            final delta = d.globalPosition.dx - _dragStartPointer!.dx;
            final f = (_dragStartFraction! + delta / width)
                .clamp(widget.minFraction, widget.maxFraction);
            setState(() => _fraction = f);
          },
          child: Container(
            width: 6,
            color: Colors.transparent,
          ),
        ),
      );
      return Row(
        children: [
          SizedBox(width: leftWidth, child: widget.left),
          handle,
          Expanded(child: widget.right),
        ],
      );
    });
  }
}
