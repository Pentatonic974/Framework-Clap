/// Palette dérivée de la maquette (`UI maquette.html`) : fond sombre,
/// accents orange / bleu / vert / violet.
library;

import 'package:flutter/material.dart';

const bgDark = Color(0xFF121316);
const bgPanel = Color(0xFF1A1C23);
const bgCanvas = Color(0xFF0D0E11);
const borderColor = Color(0xFF2A2D37);
const accentOrange = Color(0xFFFF7043);
const accentBlue = Color(0xFF29B6F6);
const accentGreen = Color(0xFF66BB6A);
const accentPurple = Color(0xFFAB47BC);
const textMain = Color(0xFFE0E0E0);
const textMuted = Color(0xFF8A8D9B);

ThemeData buildTheme() {
  final base = ThemeData(
    brightness: Brightness.dark,
    colorScheme: const ColorScheme.dark(
      primary: accentOrange,
      secondary: accentBlue,
      surface: bgPanel,
      error: Color(0xFFEF5350),
    ),
    scaffoldBackgroundColor: bgDark,
    useMaterial3: true,
  );
  return base.copyWith(
    textTheme: base.textTheme.apply(bodyColor: textMain, displayColor: textMain),
    inputDecorationTheme: const InputDecorationTheme(
      filled: true,
      fillColor: Color(0xFF222530),
      border: OutlineInputBorder(borderSide: BorderSide(color: borderColor), gapPadding: 4),
      isDense: true,
    ),
    dividerColor: borderColor,
    chipTheme: base.chipTheme.copyWith(
      backgroundColor: Color(0xFF222530),
      side: const BorderSide(color: borderColor),
      labelStyle: const TextStyle(color: textMain, fontSize: 11),
    ),
  );
}
