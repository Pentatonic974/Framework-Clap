/// Modèle de projet Dart — miroir exact du `GraphSpec` Rust
/// (crate graph-core, sérialisation camelCase). Ce JSON est le contrat
/// unique consommé par `build-service` et embarqué dans le `.clap`.

library;

import 'dart:convert';

/// Métadonnées du plugin.
class GraphMeta {
  String name;
  String id;
  String version;
  String vendor;
  String url;
  String feature;

  GraphMeta({
    this.name = 'MyClapPlugin',
    this.id = 'com.example.myclapplugin',
    this.version = '1.0.0',
    this.vendor = '',
    this.url = '',
    this.feature = 'audio-effect',
  });

  factory GraphMeta.fromJson(Map<String, dynamic> j) => GraphMeta(
        name: j['name'] ?? 'MyClapPlugin',
        id: j['id'] ?? 'com.example.myclapplugin',
        version: j['version'] ?? '1.0.0',
        vendor: j['vendor'] ?? '',
        url: j['url'] ?? '',
        feature: j['feature'] ?? 'audio-effect',
      );

  Map<String, dynamic> toJson() => {
        'name': name,
        'id': id,
        'version': version,
        'vendor': vendor,
        'url': url,
        'feature': feature,
      };
}

/// Définition d'un paramètre CLAP exposé par une instance de nœud.
class ParamDef {
  String paramId;
  String name;
  double defaultValue;
  double min, max, step;

  ParamDef({
    required this.paramId,
    required this.name,
    required this.defaultValue,
    this.min = 0,
    this.max = 1,
    this.step = 0.001,
  });

  factory ParamDef.fromJson(Map<String, dynamic> j) => ParamDef(
        paramId: j['paramId'],
        name: j['name'] ?? j['paramId'],
        defaultValue: (j['default'] ?? 0).toDouble(),
        min: (j['min'] ?? 0).toDouble(),
        max: (j['max'] ?? 1).toDouble(),
        step: (j['step'] ?? 0.001).toDouble(),
      );

  Map<String, dynamic> toJson() => {
        'paramId': paramId,
        'name': name,
        'default': defaultValue,
        'min': min,
        'max': max,
        'step': step,
      };
}

/// Instance d'un nœud posé sur le canvas nodal.
class NodeInstance {
  String id;
  String nodeType;
  NodePos pos;
  Map<String, double> params;
  List<ParamDef> exposedParams;

  NodeInstance({
    required this.id,
    required this.nodeType,
    this.pos = NodePos.zero,
    Map<String, double>? params,
    List<ParamDef>? exposedParams,
  })  : params = params ?? {},
        exposedParams = exposedParams ?? [];

  factory NodeInstance.fromJson(Map<String, dynamic> j) {
    final p = j['params'] as Map<String, dynamic>? ?? {};
    final ep = (j['exposedParams'] as List?) ?? [];
    return NodeInstance(
      id: j['id'],
      nodeType: j['nodeType'],
      pos: NodePos((j['x'] ?? 0).toDouble(), (j['y'] ?? 0).toDouble()),
      params: p.map((k, v) => MapEntry(k, (v as num).toDouble())),
      exposedParams: ep.map((e) => ParamDef.fromJson(e)).toList(),
    );
  }

  Map<String, dynamic> toJson() => {
        'id': id,
        'nodeType': nodeType,
        'x': pos.dx,
        'y': pos.dy,
        'params': params,
        'exposedParams': exposedParams.map((e) => e.toJson()).toList(),
      };
}

/// Connexion : port de sortie de `fromNode` -> port d'entrée de `toNode`.
class Wire {
  String fromNode;
  int fromPort;
  String toNode;
  int toPort;

  Wire({
    required this.fromNode,
    required this.fromPort,
    required this.toNode,
    required this.toPort,
  });

  factory Wire.fromJson(Map<String, dynamic> j) => Wire(
        fromNode: j['fromNode'],
        fromPort: j['fromPort'] as int,
        toNode: j['toNode'],
        toPort: j['toPort'] as int,
      );

  Map<String, dynamic> toJson() => {
        'fromNode': fromNode,
        'fromPort': fromPort,
        'toNode': toNode,
        'toPort': toPort,
      };

  bool same(Wire other) =>
      fromNode == other.fromNode &&
      fromPort == other.fromPort &&
      toNode == other.toNode &&
      toPort == other.toPort;
}

/// Widget GUI posé sur la grille du designer.
class GuiWidget {
  String id;
  String widgetType;
  String label;
  String paramId; // identifiant global du param piloté (ex. "g/gain_db")
  int col, row, colSpan, rowSpan;

  GuiWidget({
    required this.id,
    required this.widgetType,
    required this.label,
    this.paramId = '',
    this.col = 0,
    this.row = 0,
    this.colSpan = 2,
    this.rowSpan = 2,
  });

  factory GuiWidget.fromJson(Map<String, dynamic> j) => GuiWidget(
        id: j['id'],
        widgetType: j['widgetType'],
        label: j['label'] ?? j['widgetType'],
        paramId: j['paramId'] ?? '',
        col: (j['col'] ?? 0).toInt(),
        row: (j['row'] ?? 0).toInt(),
        colSpan: (j['colSpan'] ?? 2).toInt(),
        rowSpan: (j['rowSpan'] ?? 2).toInt(),
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'widgetType': widgetType,
        'label': label,
        'paramId': paramId,
        'col': col,
        'row': row,
        'colSpan': colSpan,
        'rowSpan': rowSpan,
      };
}

/// Enveloppe complète du projet (JSON sérialisable).
class GraphSpec {
  GraphMeta meta;
  List<NodeInstance> nodes;
  List<Wire> connections;
  List<GuiWidget> guiWidgets;

  GraphSpec({
    required this.meta,
    required this.nodes,
    required this.connections,
    required this.guiWidgets,
  });

  factory GraphSpec.fromJson(Map<String, dynamic> j) => GraphSpec(
        meta: GraphMeta.fromJson(j['meta'] ?? {}),
        nodes:
            ((j['nodes'] as List?) ?? []).map((e) => NodeInstance.fromJson(e)).toList(),
        connections: ((j['connections'] as List?) ?? [])
            .map((e) => Wire.fromJson(e))
            .toList(),
        guiWidgets: ((j['guiWidgets'] as List?) ?? [])
            .map((e) => GuiWidget.fromJson(e))
            .toList(),
      );

  Map<String, dynamic> toJson() => {
        'meta': meta.toJson(),
        'nodes': nodes.map((e) => e.toJson()).toList(),
        'connections': connections.map((e) => e.toJson()).toList(),
        'guiWidgets': guiWidgets.map((e) => e.toJson()).toList(),
      };

  String toPrettyJson() => const JsonEncoder.withIndent('  ').convert(toJson());

  NodeInstance? node(String id) {
    for (final n in nodes) {
      if (n.id == id) return n;
    }
    return null;
  }
}

/// Position 2D simple du projet (indépendante de dart:ui, réutilisable en tests).
class NodePos {
  final double dx, dy;
  const NodePos(this.dx, this.dy);
  static const zero = NodePos(0, 0);
  @override
  bool operator ==(Object other) => other is NodePos && other.dx == dx && other.dy == dy;
  @override
  int get hashCode => Object.hash(dx, dy);
}
