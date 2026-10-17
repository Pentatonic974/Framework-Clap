/// État du studio : graph exact et mutations immuables (Riverpod Notifier).
/// La validation miroir Rust se fait via build-service --check, l'UI garde
/// une validation légère (ids + ports + types de câbles).
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/node_registry.dart';
import '../../core/project_model.dart';

class ProjectState {
  final GraphSpec spec;
  final int nodeCounter;

  const ProjectState({required this.spec, this.nodeCounter = 3});

  ProjectState copy({GraphSpec? spec, int? nodeCounter}) =>
      ProjectState(spec: spec ?? this.spec, nodeCounter: nodeCounter ?? this.nodeCounter);
}

class ProjectController extends Notifier<ProjectState> {
  @override
  ProjectState build() {
    // Graphe de départ : audio-in -> gain (param CLAP exposé) -> audio-out,
    // miroir du projet démo embarqué dans le runtime Rust.
    final gain = NodeInstance(
      id: 'g1',
      nodeType: 'gain_node',
      pos: const NodePos(240, 140),
      exposedParams: [
        ParamDef(paramId: 'gain_db', name: 'Gain (dB)', defaultValue: 0, min: -60, max: 24, step: 0.1),
      ],
    );
    return ProjectState(
      spec: GraphSpec(meta: GraphMeta(), nodes: [
        NodeInstance(id: 'in1', nodeType: 'clap_audio_in', pos: const NodePos(60, 140)),
        gain,
        NodeInstance(id: 'out1', nodeType: 'clap_audio_out', pos: const NodePos(460, 140)),
      ], connections: [
        Wire(fromNode: 'in1', fromPort: 0, toNode: 'g1', toPort: 0),
        Wire(fromNode: 'g1', fromPort: 0, toNode: 'out1', toPort: 0),
        Wire(fromNode: 'in1', fromPort: 1, toNode: 'out1', toPort: 1),
      ], guiWidgets: [
        GuiWidget(
          id: 'w1', widgetType: 'knob', label: 'Gain (dB)', paramId: 'g1/gain_db',
          col: 0, row: 0, colSpan: 2, rowSpan: 2,
        ),
        GuiWidget(
          id: 'w2', widgetType: 'vu_meter', label: 'Out Level', paramId: 'g1',
          col: 2, row: 0, colSpan: 1, rowSpan: 2,
        ),
      ]),
    );
  }

  void setMeta(GraphMeta meta) =>
      state = state.copy(spec: state.spec..meta = meta);

  void moveNode(String id, NodePos pos) {
    final nodes = [...state.spec.nodes];
    final i = nodes.indexWhere((n) => n.id == id);
    if (i < 0) return;
    nodes[i] = NodeInstance(
      id: nodes[i].id, nodeType: nodes[i].nodeType, pos: pos,
      params: nodes[i].params, exposedParams: nodes[i].exposedParams,
    );
    state = state.copy(spec: state.spec..nodes = nodes);
  }

  void addNode(String nodeType, NodePos pos) {
    final def = ClapNodeRegistry.byType(nodeType);
    if (def == null) return;
    final id = 'n${state.nodeCounter + 1}';
    // Paramètres rendus réglables par défaut : premier paramètre exposé.
    List<ParamDef> exposed = const [];
    if (def.params.isNotEmpty) {
      exposed = [ParamDef(
        paramId: def.params.first.paramId,
        name: def.params.first.name,
        defaultValue: def.params.first.defaultValue,
        min: def.params.first.min, max: def.params.first.max, step: def.params.first.step,
      )];
    }
    state = state.copy(
      spec: state.spec..nodes = [...state.spec.nodes, NodeInstance(
        id: id, nodeType: nodeType, pos: pos, exposedParams: exposed,
      )],
      nodeCounter: state.nodeCounter + 1,
    );
  }

  void removeNode(String id) {
    final s = state.spec;
    s.nodes.removeWhere((n) => n.id == id);
    s.connections.removeWhere((w) => w.fromNode == id || w.toNode == id);
    s.guiWidgets.removeWhere((w) {
      final owner = w.paramId.split('/').first;
      return owner == id;
    });
    state = state.copy(spec: s);
  }

  void connect(Wire wire) {
    if (state.spec.connections.any((w) => w.same(wire))) return;
    // Un seul câble par port d'entrée (v1 : le premier câble est utilisé).
    state = state.copy(
      spec: state.spec..connections = [
        ...state.spec.connections..removeWhere((w) => w.toNode == wire.toNode && w.toPort == wire.toPort),
        wire,
      ],
    );
  }

  void disconnect(Wire wire) {
    final s = state.spec;
    s.connections.removeWhere((w) => w.same(wire));
    state = state.copy(spec: s);
  }

  void addWidget(GuiWidget widget) =>
      state = state.copy(spec: state.spec..guiWidgets = [...state.spec.guiWidgets, widget]);

  void moveWidget(String id, int col, int row) {
    final widgets = [...state.spec.guiWidgets];
    final i = widgets.indexWhere((w) => w.id == id);
    if (i < 0) return;
    widgets[i] = GuiWidget(
      id: widgets[i].id, widgetType: widgets[i].widgetType, label: widgets[i].label,
      paramId: widgets[i].paramId, col: col, row: row,
      colSpan: widgets[i].colSpan, rowSpan: widgets[i].rowSpan,
    );
    state = state.copy(spec: state.spec..guiWidgets = widgets);
  }

  void removeWidget(String id) {
    final s = state.spec;
    s.guiWidgets.removeWhere((w) => w.id == id);
    state = state.copy(spec: s);
  }

  void assignParam(String widgetId, String paramId) {
    final widgets = [...state.spec.guiWidgets];
    final i = widgets.indexWhere((w) => w.id == widgetId);
    if (i < 0) return;
    widgets[i] = GuiWidget(
      id: widgets[i].id, widgetType: widgets[i].widgetType, label: widgets[i].label,
      paramId: paramId, col: widgets[i].col, row: widgets[i].row,
      colSpan: widgets[i].colSpan, rowSpan: widgets[i].rowSpan,
    );
    state = state.copy(spec: state.spec..guiWidgets = widgets);
  }

  String toProjectJson() => state.spec.toPrettyJson();
}

final projectProvider =
    NotifierProvider<ProjectController, ProjectState>(ProjectController.new);

/// Résultat de validation légère (structurelle) du graphe courant.
class LocalReport {
  final List<String> errors, warnings;
  const LocalReport(this.errors, this.warnings);

  bool get ok => errors.isEmpty;
}

LocalReport validateLocally(GraphSpec spec) {
  final errors = <String>[];
  final warnings = <String>[];
  final ids = <String>{};
  for (final n in spec.nodes) {
    if (!ids.add(n.id)) errors.add('id dupliqué : ${n.id}');
    if (ClapNodeRegistry.byType(n.nodeType) == null) {
      errors.add('type inconnu : ${n.nodeType}');
    }
  }
  for (final w in spec.connections) {
    final to = spec.node(w.toNode);
    final from = spec.node(w.fromNode);
    if (from == null || to == null) {
      errors.add('câble vers nœud absent : ${w.fromNode}->${w.toNode}');
      continue;
    }
    final fd = ClapNodeRegistry.byType(from.nodeType);
    final td = ClapNodeRegistry.byType(to.nodeType);
    if (fd == null || td == null) continue;
    if (w.fromPort >= fd.outputs.length) {
      errors.add('port de sortie inexistant : ${fd.nodeType}#${w.fromPort}');
    } else if (w.toPort >= td.inputs.length) {
      errors.add("port d'entrée inexistant : ${td.nodeType}#${w.toPort}");
    } else if (fd.outputs[w.fromPort].kind != td.inputs[w.toPort].kind) {
      warnings.add('câble mixte ${fd.nodeType}-${w.fromPort} -> ${td.nodeType}-${w.toPort} : cast implicite');
    }
  }
  if (!spec.nodes.any((n) => n.nodeType == 'clap_audio_out')) {
    warnings.add("aucune sortie audio : le plugin n'émettra pas de signal");
  }
  return LocalReport(errors, warnings);
}

final validationProvider = Provider<LocalReport>((ref) {
  return validateLocally(ref.watch(projectProvider).spec);
});

/// Paramètres exposés du graphe, agglomérés : clé = nodeId/paramId.
Map<String, (NodeInstance, ParamDef)> exposedParams(GraphSpec spec) {
  final out = <String, (NodeInstance, ParamDef)>{};
  for (final n in spec.nodes) {
    for (final p in n.exposedParams) {
      out['${n.id}/${p.paramId}'] = (n, p);
    }
  }
  return out;
}

@visibleForTesting
List<String> debugCategoryLabels() => ClapNodeRegistry.categories();
