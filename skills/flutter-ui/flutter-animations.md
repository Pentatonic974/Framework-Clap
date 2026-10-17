# Flutter Animations

> Implicit, explicit, Hero, page transitions, physics.

---

## 1. Animation Type Selection

```
Simple property change (size, color, opacity, position)
  └── IMPLICIT: AnimatedContainer, AnimatedOpacity, TweenAnimationBuilder

Complex sequence, stagger, or precise control
  └── EXPLICIT: AnimationController + CurvedAnimation + AnimatedBuilder

Shared element across screens   → Hero widget + unique tag
Page enter/exit                 → GoRouter CustomTransitionPage or pageBuilder
Spring / bounce / friction      → SpringSimulation, FrictionSimulation
Complex vector/sprite           → Rive or Lottie package
```

---

## 2. GPU-Safe Properties

```
GPU-ACCELERATED (smooth):      CPU-BOUND (jank):
├── Transform.translate         ├── width, height
├── Transform.scale             ├── margin, padding
├── Transform.rotate            ├── top, left (Positioned)
└── Opacity                     └── border, decoration changes

RULE: Changing layout? Use Transform instead.
```

---

## 3. Implicit Animations

```dart
AnimatedContainer(
  duration: const Duration(milliseconds: 250),
  curve: Curves.easeInOut,
  width: isExpanded ? 200 : 100,
  color: isActive ? Colors.blue : Colors.grey,
  child: child,
)

AnimatedOpacity(
  opacity: isVisible ? 1.0 : 0.0,
  duration: const Duration(milliseconds: 200),
  child: child,
)

TweenAnimationBuilder<double>(
  tween: Tween(begin: 0.0, end: targetValue),
  duration: const Duration(milliseconds: 300),
  curve: Curves.easeOut,
  builder: (context, value, child) => Transform.scale(scale: value, child: child),
  child: const ExpensiveWidget(), // built once, not rebuilt during animation
)
```

---

## 4. Explicit Animations

```dart
class _MyWidgetState extends State<MyWidget> with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late final Animation<double> _scaleAnim;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(vsync: this, duration: const Duration(milliseconds: 300));
    _scaleAnim = Tween<double>(begin: 0.8, end: 1.0).animate(
      CurvedAnimation(parent: _controller, curve: Curves.elasticOut),
    );
    _controller.forward();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: _scaleAnim,
    builder: (context, child) => Transform.scale(scale: _scaleAnim.value, child: child),
    child: const MyContent(), // static — built once
  );
}
```

---

## 5. Staggered Animations

```dart
final anim1 = Tween<double>(begin: 0, end: 1).animate(
  CurvedAnimation(parent: _controller, curve: const Interval(0.0, 0.4, curve: Curves.easeOut)),
);
final anim2 = Tween<double>(begin: 0, end: 1).animate(
  CurvedAnimation(parent: _controller, curve: const Interval(0.3, 0.7, curve: Curves.easeOut)),
);
final anim3 = Tween<double>(begin: 0, end: 1).animate(
  CurvedAnimation(parent: _controller, curve: const Interval(0.6, 1.0, curve: Curves.easeOut)),
);
```

---

## 6. Hero Animations

```dart
// Same tag on source and destination
Hero(tag: 'product-image-${product.id}', child: Image.network(product.imageUrl))

// Custom flight
Hero(
  tag: 'product-image-${product.id}',
  flightShuttleBuilder: (ctx, anim, direction, fromCtx, toCtx) =>
    ScaleTransition(scale: anim, child: fromCtx.widget),
  child: child,
)
```

- Tag must be unique across all simultaneously visible heroes
- Wrap complex heroes in `RepaintBoundary`

---

## 7. Page Transitions (GoRouter)

```dart
GoRoute(
  path: '/details/:id',
  pageBuilder: (context, state) => CustomTransitionPage(
    key: state.pageKey,
    child: DetailsScreen(id: state.pathParameters['id']!),
    transitionDuration: const Duration(milliseconds: 300),
    transitionsBuilder: (context, animation, secondaryAnimation, child) =>
      FadeTransition(
        opacity: CurveTween(curve: Curves.easeInOut).animate(animation),
        child: child,
      ),
  ),
)

// Slide from right
transitionsBuilder: (context, animation, secondaryAnimation, child) =>
  SlideTransition(
    position: Tween<Offset>(begin: const Offset(1.0, 0.0), end: Offset.zero)
      .animate(CurvedAnimation(parent: animation, curve: Curves.easeInOut)),
    child: child,
  ),
```

---

## 8. Physics-Based Animations

```dart
final spring = SpringDescription(mass: 1, stiffness: 100, damping: 10);
_controller.animateWith(SpringSimulation(spring, 0, 1, 0));

_controller.animateWith(FrictionSimulation(0.135, position, velocity));
```

---

## 9. Lottie / Rive

```dart
Lottie.asset(
  'assets/animations/loading.json',
  onLoaded: (composition) {
    _controller.duration = composition.duration;
    _controller.repeat();
  },
)

RiveAnimation.asset(
  'assets/animations/character.riv',
  stateMachines: const ['State Machine 1'],
  onInit: (artboard) {
    final ctrl = StateMachineController.fromArtboard(artboard, 'State Machine 1');
    artboard.addController(ctrl!);
    _isPlaying = ctrl.findInput<bool>('isPlaying') as SMIBool;
  },
)
```

---

## 10. Curve Reference

| Curve | Use Case |
|-------|---------|
| `Curves.easeInOut` | Most transitions (default) |
| `Curves.easeOut` | Elements entering |
| `Curves.easeIn` | Elements leaving |
| `Curves.elasticOut` | Bouncy/playful entrances |
| `Curves.fastOutSlowIn` | Material page transitions |
| `Curves.bounceOut` | Game-like feedback |

**Duration:** 100–150ms micro · 200–300ms standard · 350–500ms large · never >500ms

---

## 11. AnimationController Checklist

```
✅ Created in initState()
✅ vsync: this (SingleTickerProviderStateMixin)
✅ Disposed in dispose()
✅ Never recreated in build()
✅ AnimatedBuilder used — not addListener + setState
✅ child param used for non-animating subtrees
```
