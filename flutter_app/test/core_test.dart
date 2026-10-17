import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:claps_studio/core/node_registry.dart';
import 'package:claps_studio/core/project_model.dart';

void main() {
  test('le modèle GraphSpec sérialise en camelCase conforme au produit Rust', () {
    final spec = GraphSpec(
      meta: GraphMeta(name: 'P1', id: 'com.x.p1'),
      nodes: [
        NodeInstance(id: 'g', nodeType: 'gain_node', pos: const NodePos(1, 2)),
      ],
      connections: [Wire(fromNode: 'g', fromPort: 0, toNode: 'x', toPort: 0)],
      guiWidgets: [GuiWidget(id: 'w', widgetType: 'knob', label: 'L', paramId: 'g/gain_db')],
    );
    final json = spec.toJson();
    expect(json['meta']['name'], 'P1');
    expect((json['nodes'] as List).first['nodeType'], 'gain_node');
    expect((json['nodes'] as List).first['x'], 1.0);
    expect((json['connections'] as List).first['fromNode'], 'g');
    expect((json['guiWidgets'] as List).first['widgetType'], 'knob');
    // aller-retour
    final back = GraphSpec.fromJson(jsonDecode(jsonEncode(json)));
    expect(back.node('g')!.nodeType, 'gain_node');
  });

  test('le registre Dart miroir contient les nœuds de la maquette', () {
    for (final t in [
      'clap_audio_in', 'clap_audio_out', 'clap_note_in', 'clap_param',
      'osc_va', 'filter_svf', 'gain_node', 'env_adsr', 'delay_stereo',
      'peak_rms_meter', 'math_op', 'smooth_slew',
    ]) {
      expect(ClapNodeRegistry.byType(t), isNotNull, reason: t);
    }
    // cohérence des kinds de ports : SVF = 3 sorties audio
    final svf = ClapNodeRegistry.byType('filter_svf')!;
    expect(svf.outputs.length, 3);
    expect(svf.outputs.map((p) => p.kind), everyElement(PortKind.audio));
  });

  test('défauts de paramètres du registre', () {
    final gain = ClapNodeRegistry.byType('gain_node')!;
    expect(gain.params.first.paramId, 'gain_db');
    expect(gain.params.first.defaultValue, 0.0);
    expect(gain.params.first.max, 24.0);
  });
}
