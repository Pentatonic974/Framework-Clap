/// Éditeur nodal : câblage (CustomPainter), palette drag-and-drop,
/// nœuds déplaçables, connexion interactive sortie -> entrée.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/app_theme.dart';
import '../../core/node_registry.dart';
import '../../core/project_model.dart';
import 'draggable_node_card.dart';
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
  /// Clé de la surface de dépôt (le Stack du canvas) : coordonnées exactes.
  final GlobalKey _canvasKey = GlobalKey();

  NodeDef? _def(String? type) => type == null ? null : ClapNodeRegistry.byType(type);

  /// Double-clic sur la carte nœud : dialogue d'édition des réglages du
  /// registre (params intrinsèques) + exposition des paramètres CLAP.
  Future<void> _editNode(NodeInstance node) async {
    final def = _def(node.nodeType);
    if (def == null) return;
    final result = await showDialog<_NodeEdit>(
      context: context,
      builder: (_) => _NodeEditDialog(node: node, def: def),
    );
    if (result == null) return;
    ref.read(projectProvider.notifier).applyNodeEdit(
        node.id, NodeEditResult(result.params, result.exposed));
  }

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
          child: DragTarget<(String, Offset)>(
            onWillAcceptWithDetails: (_) => true,
            onAcceptWithDetails: (details) {
              final nodeType = details.data.$1;
              // Position de dépôt = curseur, en coordonnées du Stack du
              // canvas (clé dédiée : le contexte du builder peut différer).
              final box = _canvasKey.currentContext?.findRenderObject() as RenderBox?;
              if (box == null) return;
              final local = box.globalToLocal(details.offset);
              ref
                  .read(projectProvider.notifier)
                  .addNode(nodeType, NodePos(
                      local.dx.clamp(0, (box.size.width - 220).clamp(0, double.infinity)),
                      local.dy.clamp(0, (box.size.height - 120).clamp(0, double.infinity))));
            },
            builder: (context, candidates, _) {
              final highlight = candidates.isNotEmpty;
              return Container(
                key: _canvasKey,
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
                    // nœuds (DraggableNodeCard : widget à état, le drag
                    // survit aux rebuilds déclenchés par onMoved)
                    for (final n in spec.nodes)
                      Positioned(
                        key: ValueKey('node_${n.id}'),
                        left: n.pos.dx,
                        top: n.pos.dy,
                        child: DraggableNodeCard(
                          key: ValueKey('card_${n.id}'),
                          node: n,
                          def: _def(n.nodeType),
                          highlightKind:
                              pendingOutput?.kind, // entrées compatibles surlignées
                          onMoved: (pos) =>
                              ref.read(projectProvider.notifier).moveNode(n.id, pos),
                          onPortTap: (port, isOutput) => _onPortTap(n, port, isOutput),
                          onEdit: () => _editNode(n),
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
class _Palette extends ConsumerStatefulWidget {
  const _Palette();

  @override
  ConsumerState<_Palette> createState() => _PaletteState();
}

class _PaletteState extends ConsumerState<_Palette> {
  final TextEditingController _filter = TextEditingController();
  String _filterText = '';

  @override
  void dispose() {
    _filter.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final categories = ClapNodeRegistry.categories();
    return SizedBox(
      width: 250,
      child: Material(
      color: bgPanel,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Barre de recherche (filtre direct du registre).
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 12, 12, 4),
            child: TextField(
              controller: _filter,
              onChanged: (v) => setState(() => _filterText = v.toLowerCase()),
              decoration: const InputDecoration(
                hintText: 'Rechercher un nœud…',
                prefixIcon: Icon(Icons.search, size: 18),
                border: OutlineInputBorder(),
                isDense: true,
              ),
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.fromLTRB(12, 4, 12, 12),
              children: [
                for (final cat in categories)
                  if (_nodesIn(cat).isNotEmpty)
                    _CategoryGroup(
                      category: cat,
                      nodes: _nodesIn(cat),
                    ),
              ],
            ),
          ),
        ],
      ),
      ),
    );
  }

  List<NodeDef> _nodesIn(String cat) {
    return [
      for (final d in ClapNodeRegistry.defs)
        if (d.category == cat &&
            (_filterText.isEmpty ||
                d.displayName.toLowerCase().contains(_filterText) ||
                d.nodeType.contains(_filterText)))
          d,
    ];
  }
}

class _CategoryGroup extends StatelessWidget {
  final String category;
  final List<NodeDef> nodes;

  const _CategoryGroup({required this.category, required this.nodes});

  @override
  Widget build(BuildContext context) {
    return ExpansionTile(
      tilePadding: const EdgeInsets.symmetric(horizontal: 8),
      childrenPadding: const EdgeInsets.only(bottom: 6),
      initiallyExpanded: true,
      iconColor: textMuted,
      collapsedIconColor: textMuted,
      backgroundColor: Colors.transparent,
      collapsedBackgroundColor: Colors.transparent,
      title: Text(
        category.toUpperCase(),
        style: const TextStyle(
            fontWeight: FontWeight.bold,
            fontSize: 11,
            color: accentBlue,
            letterSpacing: 0.8),
      ),
      children: [
        for (final d in nodes)
          Draggable<(String, Offset)>(
            key: ValueKey('palette_${d.nodeType}'),
            data: (d.nodeType, Offset.zero),
            feedback: MouseRegion(
              cursor: SystemMouseCursors.grabbing,
              child: Material(
                color: Colors.transparent,
                child: _PaletteItem(def: d, elevated: true),
              ),
            ),
            dragAnchorStrategy: pointerDragAnchorStrategy,
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

/// Résultat d'édition d'un nœud (réglages + exposés).
class _NodeEdit {
  final Map<String, double> params;
  final List<ParamDef> exposed;
  const _NodeEdit(this.params, this.exposed);
}

/// Dialogue Inspector : réglage des paramètres intrinsèques du registre
/// (valeur numérique) et cases « exposé au host (CLAP) ».
class _NodeEditDialog extends StatefulWidget {
  final NodeInstance node;
  final NodeDef def;

  const _NodeEditDialog({required this.node, required this.def});

  @override
  State<_NodeEditDialog> createState() => _NodeEditDialogState();
}

class _NodeEditDialogState extends State<_NodeEditDialog> {
  late final Map<String, double> _values;
  late final Set<String> _exposed;

  @override
  void initState() {
    super.initState();
    _values = {
      for (final p in widget.def.params)
        p.paramId: (widget.node.params[p.paramId] ?? p.defaultValue)
            .clamp(p.min, p.max),
    };
    _exposed = {
      for (final e in widget.node.exposedParams) e.paramId,
    };
  }

  @override
  Widget build(BuildContext context) {
    final def = widget.def;
    return AlertDialog(
      backgroundColor: bgPanel,
      title: Text(def.displayName, style: const TextStyle(fontSize: 16)),
      content: SizedBox(
        width: 460,
        child: def.params.isEmpty
            ? const Text('Ce nœud n\'a aucun réglage.',
                style: TextStyle(fontSize: 12, color: textMuted))
            : SingleChildScrollView(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (final p in def.params) ...[
                      Row(
                        children: [
                          Expanded(
                            child: Text(p.name,
                                style: const TextStyle(fontSize: 12)),
                          ),
                          Checkbox(
                            value: _exposed.contains(p.paramId),
                            onChanged: (v) => setState(() =>
                                v == true
                                    ? _exposed.add(p.paramId)
                                    : _exposed.remove(p.paramId)),
                          ),
                          const Text('CLAP',
                              style:
                                  TextStyle(fontSize: 10, color: textMuted)),
                        ],
                      ),
                      Row(
                        children: [
                          Expanded(
                            child: Slider(
                              value: _values[p.paramId]!.clamp(p.min, p.max),
                              min: p.min,
                              max: p.max,
                              divisions: p.step <= 0
                                  ? null
                                  : ((p.max - p.min) / p.step).round().clamp(1, 2000),
                              onChanged: (v) =>
                                  setState(() => _values[p.paramId] = v),
                            ),
                          ),
                          SizedBox(
                            width: 74,
                            child: Text(
                              _values[p.paramId]!.toStringAsFixed(2),
                              textAlign: TextAlign.right,
                              style: const TextStyle(
                                  fontFamily: 'monospace',
                                  fontSize: 11,
                                  color: accentGreen),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 4),
                    ],
                  ],
                ),
              ),
      ),
      actions: [
        TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Annuler')),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: accentOrange),
          onPressed: () {
            final exposed = [
              for (final id in _exposed)
                () {
                  final p =
                      def.params.firstWhere((x) => x.paramId == id);
                  return ParamDef(
                    paramId: p.paramId,
                    name: p.name,
                    defaultValue: _values[p.paramId] ?? p.defaultValue,
                    min: p.min,
                    max: p.max,
                    step: p.step,
                  );
                }(),
            ];
            Navigator.pop(
                context,
                _NodeEdit(
                  {
                    for (final p in def.params)
                      p.paramId: _values[p.paramId] ?? p.defaultValue,
                  },
                  exposed,
                ));
          },
          child: const Text('Appliquer'),
        ),
      ],
    );
  }
}
