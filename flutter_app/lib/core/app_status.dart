/// Statut global du footer (monospace vert, style maquette), partagé par
/// tous les modules UI sans dépendance circulaire.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

final clapsStatusProvider =
    NotifierProvider<StatusController, String>(StatusController.new);

class StatusController extends Notifier<String> {
  @override
  String build() => '> Framework prêt. Registres DSP/GUI et système de Build prêts.';

  void set(String v) => state = '> $v';
}
