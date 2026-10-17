# Flutter Layout System

> Responsive, adaptive layouts — LayoutBuilder, MediaQuery, Slivers, Flex.

---

## 1. Responsive Breakpoints

```dart
class Breakpoints {
  static const double mobile = 600;
  static const double tablet = 900;
  static const double desktop = 1200;
}

// LayoutBuilder — responds to parent constraints (use in reusable widgets)
LayoutBuilder(
  builder: (context, constraints) {
    if (constraints.maxWidth < Breakpoints.mobile) return const MobileLayout();
    if (constraints.maxWidth < Breakpoints.tablet) return const TabletLayout();
    return const DesktopLayout();
  },
)

// MediaQuery — full screen dimensions (use for screen-level layout)
final width = MediaQuery.sizeOf(context).width; // sizeOf not .size
final isTablet = width >= Breakpoints.mobile;
```

---

## 2. Adaptive Layout Patterns

```dart
LayoutBuilder(
  builder: (context, constraints) {
    if (constraints.maxWidth >= 600) {
      return Row(children: [
        SizedBox(width: 320, child: const SidePanel()),
        const Expanded(child: MainContent()),
      ]);
    }
    return const MainContent();
  },
)

// Rail (tablet) + Bottom Nav (mobile)
LayoutBuilder(
  builder: (context, constraints) {
    final isWide = constraints.maxWidth >= 600;
    return Scaffold(
      body: isWide
        ? Row(children: [
            NavigationRail(selectedIndex: _index, destinations: [...]),
            const Expanded(child: _pages[_index]),
          ])
        : _pages[_index],
      bottomNavigationBar: isWide ? null : NavigationBar(
        selectedIndex: _index,
        destinations: [...],
      ),
    );
  },
)
```

---

## 3. Flex Layout

| Widget | Use When |
|--------|---------|
| `Expanded` | Child takes ALL remaining space |
| `Flexible` | Child takes UP TO its flex share |
| `Spacer` | Empty flex space |
| `SizedBox` | Fixed space or constraints |
| `Align` | Position child within parent |
| `FractionallySizedBox` | Child as % of parent |

```dart
Row(children: [
  const Icon(Icons.star),
  const SizedBox(width: 8),
  const Expanded(child: Text('...')),
  TextButton(onPressed: ..., child: ...),
])

// Flexible — won't overflow, won't force full width
Row(children: [
  const Flexible(child: Text('Potentially long label')),
  const SizedBox(width: 8),
  const Text('Fixed'),
])
```

---

## 4. Common Overflow Fixes

| Problem | Cause | Fix |
|---------|-------|-----|
| `RenderFlex overflow` in Row | Too many fixed children | Wrap text in `Flexible` |
| `ListView` in `Column` overflow | Unbounded height | Wrap `ListView` in `Expanded` |
| Text cuts off | Too long | `overflow: TextOverflow.ellipsis, maxLines: 1` |
| Image overflow | Unconstrained | `BoxFit.cover` + sized parent |
| `Unbounded height` | Nested `Column` without constraints | `Expanded` or fixed heights |

```dart
Column(children: [
  const Header(),
  Expanded( // wrap ListView in Expanded inside Column
    child: ListView.builder(
      itemCount: items.length,
      itemBuilder: (ctx, i) => Item(items[i]),
    ),
  ),
])
```

---

## 5. SafeArea and Edge-to-Edge

```dart
Scaffold(body: SafeArea(child: content))

// Edge-to-edge (extends under status bar)
Scaffold(
  extendBodyBehindAppBar: true,
  appBar: AppBar(backgroundColor: Colors.transparent, elevation: 0),
  body: content,
)

final topPadding = MediaQuery.paddingOf(context).top;
final bottomPadding = MediaQuery.paddingOf(context).bottom;
```

---

## 6. Slivers (Complex Scroll UIs)

```dart
CustomScrollView(
  slivers: [
    SliverAppBar(
      expandedHeight: 200,
      pinned: true,
      flexibleSpace: FlexibleSpaceBar(
        title: const Text('My App'),
        background: Image.network(headerImage, fit: BoxFit.cover),
      ),
    ),
    const SliverToBoxAdapter(child: SectionHeader()),
    SliverList.builder(
      itemCount: items.length,
      itemBuilder: (ctx, i) => ItemTile(items[i]),
    ),
    SliverGrid.builder(
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: 2, childAspectRatio: 3 / 4,
        crossAxisSpacing: 8, mainAxisSpacing: 8,
      ),
      itemCount: items.length,
      itemBuilder: (ctx, i) => GridItem(items[i]),
    ),
    // Most performant — fixed height skips layout measurement
    SliverFixedExtentList(
      itemExtent: 64,
      delegate: SliverChildBuilderDelegate(
        (ctx, i) => ListTile(title: Text(items[i].name)),
        childCount: items.length,
      ),
    ),
  ],
)
```

---

## 7. Proportional Sizing

```dart
FractionallySizedBox(widthFactor: 0.8, child: ElevatedButton(...))

AspectRatio(aspectRatio: 16 / 9, child: VideoPlayer(...))

FittedBox(fit: BoxFit.contain, child: Text('Auto-scaling text'))
```

---

## 8. Intrinsic Sizing (Use Sparingly)

`IntrinsicHeight` / `IntrinsicWidth` are O(N²) — never use in lists.

```dart
// Prefer fixed heights
ListView.builder(itemExtent: 72, ...)

// Or SliverFixedExtentList
```

---

## 9. Spacing Constants

```dart
const pageHorizontal = 16.0;
const cardInner = 16.0;
const itemGap = 8.0;
const sectionGap = 24.0;

Padding(padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8), child: child)
```

---

## 10. Responsive Image

```dart
AspectRatio(
  aspectRatio: 16 / 9,
  child: Image.network(
    url,
    fit: BoxFit.cover,
    cacheWidth: (MediaQuery.sizeOf(context).width * MediaQuery.devicePixelRatioOf(context)).round(),
  ),
)

CircleAvatar(
  radius: 24,
  backgroundImage: NetworkImage(avatarUrl),
  backgroundColor: Theme.of(context).colorScheme.surfaceVariant,
)
```
