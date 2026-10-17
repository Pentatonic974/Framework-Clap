# Flutter Performance

> 60fps rules: const, RepaintBoundary, shader warmup, targeted rebuilds.

---

## 1. const Everywhere (Most Impactful)

```dart
class MyCard extends StatelessWidget {
  const MyCard({super.key, required this.title});
  final String title;

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.all(16),
      child: Column(
        children: [
          Icon(Icons.star),
          SizedBox(height: 8),
        ],
      ),
    );
  }
}
```

Find missing const: `flutter analyze`

**Rules:**
- Every `StatelessWidget` → `const MyWidget({super.key})`
- Every widget with compile-time args → prefix `const`
- `EdgeInsets.all(16)` → `const EdgeInsets.all(16)`
- `SizedBox(height: 8)` → `const SizedBox(height: 8)`

---

## 2. Targeted Rebuilds

```dart
ValueListenableBuilder<int>(
  valueListenable: _counter,
  builder: (context, value, child) => Text('$value'),
  child: const ExpensiveStaticWidget(),
)

AnimatedBuilder(
  animation: _animation,
  builder: (context, child) => Transform.scale(scale: _animation.value, child: child),
  child: const MyExpensiveChild(),
)

// Riverpod — rebuild only on field change
final name = ref.watch(userProvider.select((u) => u.name));

// Provider
Selector<UserModel, String>(
  selector: (_, user) => user.name,
  builder: (_, name, __) => Text(name),
)
```

---

## 3. RepaintBoundary

Isolates a subtree into its own compositing layer — prevents animation from repainting the whole screen.

```dart
RepaintBoundary(child: AnimatedWidget(...))

RepaintBoundary(
  child: CustomPaint(painter: ComplexChartPainter(data), size: const Size(300, 200)),
)
```

✅ Use: animations next to static content, complex `CustomPainter`, frequently updating widgets
❌ Avoid: wrapping every widget — each boundary adds a compositing layer

---

## 4. List Performance

```dart
// Lazy — only renders visible items
ListView.builder(
  itemCount: items.length,
  itemBuilder: (context, index) =>
    ItemTile(key: ValueKey(items[index].id), item: items[index]),
)

// Fixed extent — most performant, skips layout measurement
ListView.builder(
  itemExtent: 72.0,
  itemCount: items.length,
  itemBuilder: (ctx, i) => ItemTile(items[i]),
)

// In CustomScrollView
SliverFixedExtentList(
  itemExtent: 72.0,
  delegate: SliverChildBuilderDelegate(
    (ctx, i) => ItemTile(items[i]),
    childCount: items.length,
  ),
)

GridView.builder(
  gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
    crossAxisCount: 2, childAspectRatio: 1,
  ),
  itemCount: items.length,
  itemBuilder: (ctx, i) => GridItem(items[i]),
)

// ❌ NEVER — renders ALL items
ListView(children: items.map((i) => ItemTile(i)).toList())
```

---

## 5. Image Performance

```dart
Image.network(url, cacheWidth: 300, cacheHeight: 200)

await precacheImage(NetworkImage(url), context);

CachedNetworkImage(
  imageUrl: url,
  placeholder: (ctx, url) => const ShimmerPlaceholder(),
  errorWidget: (ctx, url, err) => const Icon(Icons.broken_image),
  memCacheWidth: 300,
)

Image(image: ResizeImage(AssetImage('assets/large_image.png'), width: 150))
```

---

## 6. Shader Warmup

```dart
void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await const DefaultShaderWarmUp().execute();
  runApp(const MyApp());
}

// Or bundle SkSL:
// flutter run --profile --cache-sksl --purge-persistent-cache
// flutter build apk --bundle-sksl-path=flutter_01.sksl.json
```

---

## 7. Heavy Computation Off UI Thread

```dart
Future<List<Item>> parseItems(String json) => compute(_parse, json);

List<Item> _parse(String json) =>
  (jsonDecode(json) as List).map((e) => Item.fromJson(e)).toList();

// Flutter 3+
final result = await Isolate.run(() => heavyComputation(data));
```

---

## 8. DevTools

```
Performance Overlay   → flutter run --profile → enable in DevTools
Raster thread spike   → add RepaintBoundary
UI thread spike       → move work to compute()

Widget Rebuild Tracker → DevTools → Widget → "Track widget rebuilds"
  Blue = rebuilt once | Red = rebuilt many times (investigate)

Memory → watch for monotonic growth → check dispose() / cancel()
```

---

## 9. Build Method Rules

`build()` must be pure and fast:

| ✅ OK in build() | ❌ NOT in build() |
|-----------------|------------------|
| `Theme.of(context)` | async operations |
| `ref.watch()` | heavy computation |
| const widget creation | side effects |
| `MediaQuery.sizeOf()` | creating controllers/notifiers |

```dart
// ❌ New controller instance every build
Widget build(BuildContext context) {
  final controller = TextEditingController();
  return TextField(controller: controller);
}

// ✅
late final _controller = TextEditingController();
```

---

## 10. Key Rules for Lists

```dart
ListView.builder(
  itemBuilder: (ctx, i) => Dismissible(
    key: ValueKey(items[i].id), // stable ID from data
    child: ItemTile(items[i]),
  ),
)

// ❌ key: ValueKey(i)       — index shifts on reorder
// ❌ ObjectKey(items[i])    — new key on rebuild
// ❌ UniqueKey()            — new key every build, breaks animations
```
