# Flutter Theme System

> Material 3, ColorScheme, TextTheme, ThemeExtension, dark mode.

---

## 1. Material 3 Setup

```dart
MaterialApp(
  theme: ThemeData(
    useMaterial3: true,
    colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF6750A4)),
  ),
  darkTheme: ThemeData(
    useMaterial3: true,
    colorScheme: ColorScheme.fromSeed(
      seedColor: const Color(0xFF6750A4),
      brightness: Brightness.dark,
    ),
  ),
  themeMode: ThemeMode.system,
)

// Override specific roles after seed generation
colorScheme: ColorScheme.fromSeed(seedColor: brandColor).copyWith(
  primary: const Color(0xFF4A90D9),
),
```

---

## 2. ColorScheme Roles

| Role | Use For |
|------|---------|
| `colorScheme.primary` | Primary brand color, key buttons |
| `colorScheme.onPrimary` | Content on top of primary |
| `colorScheme.secondary` | Secondary actions, accents |
| `colorScheme.tertiary` | Contrasting accent, decorative |
| `colorScheme.error` | Errors, destructive actions |
| `colorScheme.surface` | Card, dialog, sheet backgrounds |
| `colorScheme.onSurface` | Text/icons on surface |
| `colorScheme.outline` | Borders, dividers |
| `colorScheme.outlineVariant` | Subtle borders |
| `colorScheme.surfaceContainerHighest` | Input field fills |

```dart
final scheme = Theme.of(context).colorScheme;
```

---

## 3. TextTheme Scale

| Style | Size | Use For |
|-------|------|---------|
| `displayLarge` | 57sp | Hero text, marketing |
| `displayMedium` | 45sp | Large headers |
| `displaySmall` | 36sp | Section headers |
| `headlineLarge` | 32sp | Screen titles |
| `headlineMedium` | 28sp | Card headers |
| `headlineSmall` | 24sp | Subsection titles |
| `titleLarge` | 22sp | AppBar title |
| `titleMedium` | 16sp | List tile title |
| `titleSmall` | 14sp | Tab labels |
| `bodyLarge` | 16sp | Primary body text |
| `bodyMedium` | 14sp | Secondary body |
| `bodySmall` | 12sp | Captions, metadata |
| `labelLarge` | 14sp | Buttons |
| `labelMedium` | 12sp | Tags, chips |
| `labelSmall` | 11sp | Overlines |

```dart
Text('Title', style: Theme.of(context).textTheme.headlineMedium)
Text('Body', style: Theme.of(context).textTheme.bodyLarge?.copyWith(
  color: Theme.of(context).colorScheme.onSurfaceVariant,
))
```

---

## 4. Custom Font Integration

```dart
// pubspec.yaml
fonts:
  - family: YourFont
    fonts:
      - asset: assets/fonts/YourFont-Regular.ttf
      - asset: assets/fonts/YourFont-Bold.ttf
        weight: 700

// ThemeData
ThemeData(
  useMaterial3: true,
  textTheme: GoogleFonts.interTextTheme(),
)
```

---

## 5. ThemeExtension (Custom Design Tokens)

```dart
@immutable
class AppColors extends ThemeExtension<AppColors> {
  const AppColors({
    required this.brandGold,
    required this.successGreen,
    required this.warningAmber,
  });

  final Color brandGold;
  final Color successGreen;
  final Color warningAmber;

  @override
  AppColors copyWith({Color? brandGold, Color? successGreen, Color? warningAmber}) =>
    AppColors(
      brandGold: brandGold ?? this.brandGold,
      successGreen: successGreen ?? this.successGreen,
      warningAmber: warningAmber ?? this.warningAmber,
    );

  @override
  AppColors lerp(AppColors? other, double t) => AppColors(
    brandGold: Color.lerp(brandGold, other?.brandGold, t)!,
    successGreen: Color.lerp(successGreen, other?.successGreen, t)!,
    warningAmber: Color.lerp(warningAmber, other?.warningAmber, t)!,
  );
}

ThemeData(
  extensions: const [
    AppColors(
      brandGold: Color(0xFFFFD700),
      successGreen: Color(0xFF4CAF50),
      warningAmber: Color(0xFFFFC107),
    ),
  ],
)

final appColors = Theme.of(context).extension<AppColors>()!;
Container(color: appColors.brandGold)
```

---

## 6. Spacing Tokens

```dart
@immutable
class AppSpacing extends ThemeExtension<AppSpacing> {
  const AppSpacing({
    this.xs = 4.0, this.sm = 8.0, this.md = 16.0,
    this.lg = 24.0, this.xl = 32.0, this.xxl = 48.0,
  });

  final double xs, sm, md, lg, xl, xxl;

  @override
  AppSpacing copyWith({...}) => AppSpacing(...);

  @override
  AppSpacing lerp(AppSpacing? other, double t) => this;
}

final spacing = Theme.of(context).extension<AppSpacing>()!;
Padding(padding: EdgeInsets.all(spacing.md))
```

---

## 7. Dark Mode

```dart
// ✅
color: Theme.of(context).colorScheme.surface

// ❌ Breaks dark mode
color: Colors.white
color: const Color(0xFFF5F5F5)

final isDark = Theme.of(context).brightness == Brightness.dark;
final imagePath = isDark ? 'assets/logo_dark.png' : 'assets/logo_light.png';
```

---

## 8. Component Theme Overrides

```dart
ThemeData(
  filledButtonTheme: FilledButtonThemeData(
    style: FilledButton.styleFrom(
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      minimumSize: const Size.fromHeight(48),
    ),
  ),
  inputDecorationTheme: const InputDecorationTheme(
    border: OutlineInputBorder(),
    filled: true,
  ),
  cardTheme: const CardTheme(
    elevation: 0,
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.all(Radius.circular(12)),
    ),
  ),
  appBarTheme: AppBarTheme(
    centerTitle: true,
    backgroundColor: colorScheme.surface,
    foregroundColor: colorScheme.onSurface,
    elevation: 0,
    scrolledUnderElevation: 1,
  ),
)
```
