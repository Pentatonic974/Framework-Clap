# Flutter State Management

> Riverpod (primary) + Provider (legacy). Claude MUST enforce these rules.

---

## 0. Auto-Detect State Manager

```
Scan imports BEFORE writing any state code:

import 'package:flutter_riverpod/' → Apply RIVERPOD rules
import 'package:provider/'         → Apply PROVIDER rules
Neither found                      → Ask user → recommend Riverpod for new projects
```

---

## PART 1: RIVERPOD

### Mandatory Rules

| # | Rule | Anti-Pattern |
|---|------|-------------|
| 1 | `ConsumerWidget` / `ConsumerStatefulWidget` for Riverpod UI | `StatefulWidget` + manually calling ref |
| 2 | Providers at **file top-level** only | Provider inside `build()` or widget class |
| 3 | `ref.watch()` in `build()` | `ref.read()` in `build()` |
| 4 | `ref.read()` ONLY in callbacks/handlers | `ref.read()` to get reactive data |
| 5 | `AsyncNotifierProvider` for async, `NotifierProvider` for sync | `StateProvider` for complex state |
| 6 | Always `when(data:, loading:, error:)` on `AsyncValue` | Ignoring loading/error |
| 7 | `select()` to narrow rebuilds | Watching entire large state object |
| 8 | `family` for parameterized providers | Creating multiple similar providers |
| 9 | `ref.invalidate()` / `ref.refresh()` to refetch | Re-creating providers |
| 10 | `autoDispose` for screen-scoped state | Global provider for local state |
| 11 | `camelCaseProvider` naming (e.g., `userProfileProvider`) | `UserProfileProvider`, `user_profile` |
| 12 | `ProviderScope` at app root only | Nested `ProviderScope` unless intentional |

---

### Provider Types

| Type | Use For |
|------|---------|
| `Provider` | Computed values, services |
| `NotifierProvider` | Sync mutable state |
| `AsyncNotifierProvider` | Async data (API calls) |
| `StreamProvider` | Real-time data (WebSocket, Firestore) |
| `FutureProvider` | One-shot async |
| `StateProvider` | Simple single values only |

---

### Core Patterns

```dart
@riverpod
class UserNotifier extends _$UserNotifier {
  @override
  Future<User> build() => ref.watch(apiServiceProvider).getUser();

  Future<void> updateName(String name) async {
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() =>
      ref.read(apiServiceProvider).updateUser(name: name),
    );
  }
}

class ProfileScreen extends ConsumerWidget {
  const ProfileScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final user = ref.watch(userNotifierProvider);
    return user.when(
      data: (u) => Text(u.name),
      loading: () => const CircularProgressIndicator(),
      error: (e, stack) => ErrorView(error: e, onRetry: () =>
        ref.invalidate(userNotifierProvider)),
    );
  }
}

// select() — only rebuilds when avatarUrl changes
final avatarUrl = ref.watch(
  userNotifierProvider.select((u) => u.valueOrNull?.avatarUrl),
);

// family
@riverpod
Future<Product> product(Ref ref, String productId) =>
  ref.watch(apiServiceProvider).getProduct(productId);

final product = ref.watch(productProvider('abc-123'));

// Combining providers
@riverpod
Future<List<Review>> userReviews(Ref ref) async {
  final user = await ref.watch(userNotifierProvider.future);
  return ref.watch(apiServiceProvider).getReviews(userId: user.id);
}

// autoDispose — disposes when no listeners remain (screen unmounted)
@riverpod
class FormController extends _$FormController {
  @override
  FormState build() => FormState.initial();
}
```

---

### Testing

```dart
testWidgets('shows user name', (tester) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [userNotifierProvider.overrideWith(() => MockUserNotifier())],
      child: const MaterialApp(home: ProfileScreen()),
    ),
  );
  expect(find.text('John Doe'), findsOneWidget);
});
```

---

## PART 2: PROVIDER PACKAGE

### Mandatory Rules

| # | Rule | Anti-Pattern |
|---|------|-------------|
| 1 | `context.watch<T>()` in `build()` | `context.read<T>()` in `build()` |
| 2 | `context.read<T>()` ONLY in callbacks | Reactive reads in handlers |
| 3 | `Selector<T, R>` for partial state | `Consumer<T>` wrapping entire screen |
| 4 | `MultiProvider` at root | Nested `Provider` widgets in subtrees |
| 5 | `ProxyProvider` for dependent providers | Calling `context.read` inside a provider |
| 6 | Mutate first → then `notifyListeners()` | `notifyListeners()` before mutation |
| 7 | Resources cleaned in `ChangeNotifier.dispose()` | Leaking timers/streams/controllers |
| 8 | `FutureProvider`/`StreamProvider` for async | Async in `ChangeNotifier` constructor |
| 9 | `Provider.of<T>(context, listen: false)` in handlers | `listen: true` in event handlers |
| 10 | Never store `BuildContext` in `ChangeNotifier` | Context stored as field |

---

### Core Patterns

```dart
class CartModel extends ChangeNotifier {
  final _items = <Item>[];
  List<Item> get items => UnmodifiableListView(_items);
  int get totalCount => _items.length;

  void add(Item item) {
    _items.add(item);       // Mutate first
    notifyListeners();      // Then notify
  }

  @override
  void dispose() {
    // Clean up: timers, stream subscriptions, controllers
    super.dispose();
  }
}

MultiProvider(
  providers: [
    ChangeNotifierProvider(create: (_) => CartModel()),
    Provider(create: (_) => ApiService()),
    ProxyProvider<ApiService, UserRepository>(
      update: (_, api, __) => UserRepository(api),
    ),
  ],
  child: const MyApp(),
)

Selector<CartModel, int>(
  selector: (_, cart) => cart.totalCount,
  builder: (_, count, __) => Badge(label: Text('$count')),
)

final configProvider = FutureProvider<AppConfig>((ref) async {
  return ApiService().fetchConfig();
});
```

---

## STATE MANAGEMENT DECISION MATRIX

| Signal | Use |
|--------|-----|
| New project, greenfield | **Riverpod** |
| `import 'package:provider/'` exists | **Provider** (don't migrate unless asked) |
| User says "use provider" | **Provider** |
| User says "use riverpod" | **Riverpod** |
| `ChangeNotifier` classes exist | **Provider** (respect existing pattern) |
| Asked to migrate | **Riverpod** + show migration path |

---

## Migration: Provider → Riverpod

| Provider | Riverpod |
|---------|---------|
| `ChangeNotifier` + `ChangeNotifierProvider` | `Notifier` + `NotifierProvider` |
| `context.watch<T>()` | `ref.watch(myProvider)` |
| `context.read<T>()` | `ref.read(myProvider)` |
| `Selector<T, R>` | `ref.watch(myProvider.select(...))` |
| `ProxyProvider<A, B>` | `ref.watch(aProvider)` inside `bProvider` |
| `FutureProvider` | `FutureProvider` |
| `StreamProvider` | `StreamProvider` |
| `MultiProvider` | `ProviderScope` with overrides |
| `ChangeNotifier.dispose()` | `Notifier.dispose()` or `autoDispose` |
