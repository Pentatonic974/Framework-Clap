# Flutter Navigation

> GoRouter, PopScope, back logic, deep links, nested navigation.

---

## 1. Navigation Package Decision

```
Simple app (<5 screens, no deep links, no auth) → Navigator 1.0 is fine
Multi-screen + deep links + auth guards        → ✅ GoRouter (RECOMMENDED)
Enterprise + type-safe routes + codegen        → auto_route
```

---

## 2. GoRouter Setup

```dart
// Define at top level, NOT inside widget
final router = GoRouter(
  initialLocation: '/home',
  debugLogDiagnostics: true, // remove in prod
  redirect: (context, state) {
    final isLoggedIn = ref.read(authProvider).isLoggedIn;
    final isAuthRoute = state.matchedLocation == '/login';
    if (!isLoggedIn && !isAuthRoute) return '/login';
    if (isLoggedIn && isAuthRoute) return '/home';
    return null;
  },
  routes: [
    GoRoute(path: '/login', builder: (ctx, state) => const LoginScreen()),
    ShellRoute(
      builder: (ctx, state, child) => AppShell(child: child),
      routes: [
        GoRoute(path: '/home', builder: (ctx, state) => const HomeScreen()),
        GoRoute(
          path: '/product/:id',
          builder: (ctx, state) => ProductScreen(id: state.pathParameters['id']!),
        ),
      ],
    ),
  ],
);

MaterialApp.router(routerConfig: router)
```

---

## 3. go() vs push() vs replace()

| Method | Back Stack | Use When |
|--------|-----------|----------|
| `context.go('/path')` | Replaces entire stack | Tab switch, auth redirect |
| `context.push('/path')` | Adds to stack (can pop) | Drill-down |
| `context.replace('/path')` | Replaces current, no pop back | Login → Home |
| `context.pop()` | Removes current route | Back button, close modal |
| `context.pop(result)` | Removes + returns data | Form submit, picker |
| `context.canPop()` | Check if pop is possible | Conditional back behavior |

---

## 4. Back Button Handling

### PopScope (Flutter 3.12+ — replaces WillPopScope)

```dart
// Exit confirm dialog at root
PopScope(
  canPop: false,
  onPopInvokedWithResult: (didPop, result) async {
    if (didPop) return;
    final shouldExit = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Exit app?'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
          TextButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Exit')),
        ],
      ),
    );
    if (shouldExit == true && context.mounted) SystemNavigator.pop();
  },
  child: child,
)

// Intercept for unsaved changes
PopScope(
  canPop: false,
  onPopInvokedWithResult: (didPop, result) {
    if (didPop) return;
    _hasUnsavedChanges ? _showDiscardDialog() : context.pop();
  },
  child: child,
)

// Allow pop but track for analytics
PopScope(
  canPop: true,
  onPopInvokedWithResult: (didPop, result) {
    if (didPop) _analytics.logScreenExit();
  },
  child: child,
)
```

---

## 5. Persistent Tab Navigation (StatefulShellRoute)

```dart
StatefulShellRoute.indexedStack(
  builder: (ctx, state, navigationShell) => ScaffoldWithNavBar(navigationShell: navigationShell),
  branches: [
    StatefulShellBranch(routes: [GoRoute(path: '/home', builder: (_, __) => const HomeScreen())]),
    StatefulShellBranch(routes: [GoRoute(path: '/search', builder: (_, __) => const SearchScreen())]),
    StatefulShellBranch(routes: [GoRoute(path: '/profile', builder: (_, __) => const ProfileScreen())]),
  ],
)

class ScaffoldWithNavBar extends StatelessWidget {
  const ScaffoldWithNavBar({super.key, required this.navigationShell});
  final StatefulNavigationShell navigationShell;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: navigationShell,
    bottomNavigationBar: NavigationBar(
      selectedIndex: navigationShell.currentIndex,
      onDestinationSelected: (index) => navigationShell.goBranch(
        index,
        initialLocation: index == navigationShell.currentIndex,
      ),
      destinations: const [
        NavigationDestination(icon: Icon(Icons.home), label: 'Home'),
        NavigationDestination(icon: Icon(Icons.search), label: 'Search'),
        NavigationDestination(icon: Icon(Icons.person), label: 'Profile'),
      ],
    ),
  );
}
```

---

## 6. Deep Linking

```dart
// Path parameters
GoRoute(
  path: '/user/:userId/post/:postId',
  builder: (ctx, state) => PostScreen(
    userId: state.pathParameters['userId']!,
    postId: state.pathParameters['postId']!,
  ),
)

// Query parameters
GoRoute(
  path: '/search',
  builder: (ctx, state) => SearchScreen(
    query: state.uri.queryParameters['q'] ?? '',
    filter: state.uri.queryParameters['filter'],
  ),
)

// Extra — NOT deep-linkable (lost on cold start)
context.push('/details', extra: myObject);
```

**Gotchas:**
- `extra` lost on cold start — use path/query params for serializable data
- Always validate params, redirect on invalid values
- Test: `adb shell am start -W -a android.intent.action.VIEW -d "myapp://user/123"`

---

## 7. Route Guards (Auth)

```dart
final router = GoRouter(
  redirect: (BuildContext context, GoRouterState state) {
    final isAuth = ref.read(authProvider).isAuthenticated;
    final isOnAuth = state.matchedLocation.startsWith('/auth');
    if (!isAuth && !isOnAuth) return '/auth/login';
    if (isAuth && isOnAuth) return '/home';
    return null;
  },
  refreshListenable: ref.read(authProvider.notifier), // re-runs redirect on auth change
)
```

---

## 8. Back Stack Mental Model

```
Navigator stack (push/pop):
┌─────────────┐
│  Details    │ ← context.pop() removes this
├─────────────┤
│  List       │
├─────────────┤
│  Home       │ ← Root: PopScope(canPop: false) here
└─────────────┘

context.go() resets entire stack:
┌─────────────┐
│  New Screen │
└─────────────┘

Tab navigation (StatefulShellRoute):
Tab A Stack    Tab B Stack    Tab C Stack
┌─────────┐   ┌─────────┐   ┌─────────┐
│ Detail  │   │         │   │         │
├─────────┤   │ Search  │   │ Profile │
│ List    │   └─────────┘   └─────────┘
└─────────┘
  (active)    (preserved)   (preserved)
```

---

## 9. iOS Swipe-Back

```dart
// CupertinoPage preserves iOS swipe-back gesture
GoRoute(
  path: '/details',
  pageBuilder: (ctx, state) => CupertinoPage(key: state.pageKey, child: const DetailsScreen()),
)
// Prefer native transitions — avoid GestureDetector conflicts with swipe-back
```

---

## 10. Common Patterns

```dart
// Return result from pushed screen
context.pop({'confirmed': true, 'value': 42});
final result = await context.push<Map<String, dynamic>>('/confirm');
if (result?['confirmed'] == true) { ... }

// Redirect after async op — use go() not push()
Future<void> _onLogin() async {
  await ref.read(authProvider.notifier).login(email, password);
  if (mounted) context.go('/home');
}

// Named routes
GoRoute(name: 'product', path: '/product/:id', ...)
context.goNamed('product', pathParameters: {'id': productId});
```
