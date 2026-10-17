import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:claps_studio/core/node_registry.dart';
import 'package:claps_studio/core/project_model.dart';
import 'package:claps_studio/features/editor/draggable_node_card.dart';

void main() {
  testWidgets('drag : pan déclenche onMoved avec la bonne position absolue',
      (tester) async {
    final received = <NodePos>[];
    const start = NodePos(100, 100);

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: SizedBox(
          width: 1400, height: 900,
          child: Stack(children: [
            Positioned(
              left: start.dx, top: start.dy,
              child: DraggableNodeCard(
                key: const ValueKey('card'),
                node: NodeInstance(id: 'g', nodeType: 'gain_node', pos: start),
                def: ClapNodeRegistry.byType('gain_node'),
                highlightKind: null,
                onMoved: received.add,
                onPortTap: (_, _) {},
                onEdit: () {},
                onDelete: () {},
              ),
            ),
          ]),
        ),
      ),
    ));
    await tester.pump();

    // Geste simple drag (start→update→up géré par le tester).
    await tester.drag(find.byKey(const ValueKey('card')), const Offset(50, 40));
    await tester.pumpAndSettle();

    // OnPanUpdate = originPos + (global − originPointer) ; originPos = la
    // position du nœud au press (100,100) → le résultat doit être 150,140
    // (au moins une notification, et la DERNIÈRE doit être 150,140).
    expect(received, isNotEmpty, reason: 'onMoved appelé pendant le pan');
    final last = received.last;
    expect(last.dx, closeTo(150, 10), reason: 'x absolu : 100 + 50');
    expect(last.dy, closeTo(140, 10), reason: 'y absolu : 100 + 40');

    // Deuxième geste TRÈS court : le widget testé n'est PAS repositionné
    // (pas de Positioned mis à jour ici) donc widget.node reste (100,100) :
    // l'origine recapturée au press est (100,100) → -30/-20 => (70,80).
    // C'est le contrat : l'origine du geste = position props au press.
    await tester.drag(find.byKey(const ValueKey('card')), const Offset(-30, -20));
    await tester.pump(const Duration(milliseconds: 350));
    final last2 = received.last;
    expect(last2.dx, closeTo(70, 2), reason: '2e geste : origine props (100,100)');
    expect(last2.dy, closeTo(80, 2), reason: '2e geste : origine props (100,100)');
  });

  testWidgets('drag en 2 mouvements consécutifs du même geste (continuity)',
      (tester) async {
    final received = <NodePos>[];
    const start = NodePos(100, 100);

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: SizedBox(
          width: 1400, height: 900,
          child: Stack(children: [
            Positioned(
              left: start.dx, top: start.dy,
              child: DraggableNodeCard(
                key: const ValueKey('card'),
                node: NodeInstance(id: 'g', nodeType: 'gain_node', pos: start),
                def: ClapNodeRegistry.byType('gain_node'),
                highlightKind: null,
                onMoved: received.add,
                onPortTap: (_, _) {},
                onEdit: () {},
                onDelete: () {},
              ),
            ),
          ]),
        ),
      ),
    ));
    await tester.pump();

    // Un geste à la main pour les DEUX moves successifs sans up intercalé.
    final g = await tester.startGesture(
        tester.getCenter(find.byKey(const ValueKey('card'))));
    await tester.pump();
    await g.moveBy(const Offset(60, 30));
    await tester.pump();
    await g.moveBy(const Offset(-20, 10)); // delta depuis le point ACTUEL
    await tester.pump();
    await g.up();
    await tester.pump(const Duration(milliseconds: 350));

    expect(received.length, greaterThanOrEqualTo(1));
    // Origine = position DU PRESS (100,100) : les deux moves sont relatifs
    // AU CURSEUR cumulé (moveBy avance le curseur). Origine pointer = centre
    // du press ! (Le press est au CENTRE du widget, pas au coin.)
    // L'origine POS = (100,100) (coin du widget). Le press pointer est au
    // centre (190,~125). Après +60/+30 : delta cursor = +60/+30 →
    // pos = (160,130). Après -20/+10 : delta cursor = +40/+40 → pos = (140,140).
    final mid = received[received.length - 2];
    final last = received.last;
    expect(mid.dx, closeTo(160, 10), reason: 'pas 1 : x = 100+60');
    expect(mid.dy, closeTo(130, 10), reason: 'pas 1 : y = 100+30');
    expect(last.dx, closeTo(140, 10), reason: 'pas 2 : x = 100+40');
    expect(last.dy, closeTo(140, 10), reason: 'pas 2 : y = 100+40');
  });
}
