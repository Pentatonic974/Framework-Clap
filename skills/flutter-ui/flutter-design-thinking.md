# Flutter Design Thinking

> Forces context-first thinking. Prevents AI from applying memorized patterns.

---

## 🧠 Mandatory Protocol (Before Every Task)

```
1️⃣ CONTEXT SCAN
   └── What is this screen's PRIMARY purpose?
   └── What does the user DO here?

2️⃣ ANTI-DEFAULT ANALYSIS
   └── Am I reaching for a memorized widget?
   └── Does it ACTUALLY fit this problem?

3️⃣ LAYOUT DECOMPOSITION
   └── Column/Row/Stack/CustomPaint — which is correct?
   └── What are the layout constraints?

4️⃣ ANIMATION INTENT
   └── Functional feedback, delight, or branding?
   └── Which type/duration/curve serves the intent?

5️⃣ STATE SCOPE
   └── Local vs screen vs global?
   └── Which provider type fits this lifecycle?
```

---

## 🚫 Forbidden Defaults

Never apply these without questioning:

**Widget defaults**
- `Container` for everything → `SizedBox` for sizing, `Padding` for padding, `Align` for alignment
- `Scaffold` on every screen → Does this modal/overlay need AppBar?
- `Card` for every list item → Does it need elevation? Often just a `ListTile`
- `StatefulWidget` by default → Is local state actually needed?
- `GestureDetector` wrapping `Column` → Use `InkWell` or `Ink` for Material ripple

**Navigation defaults**
- `Navigator.push` for everything → Is GoRouter in use?
- `BottomNavigationBar` → Material 3 uses `NavigationBar`
- AppBar back button only → Android system back button tested?

**State defaults**
- `setState` for everything → Is this truly local UI state?
- Riverpod for every tiny detail → Widget-local is fine for toggles
- Global provider for screen-local state → Use `autoDispose`

**Animation defaults**
- `AnimatedContainer` for everything → Right tool for job?
- `300ms` always → Match: 150ms micro, 250ms standard, 400ms large
- Linear curve → `easeInOut` minimum
- Opacity fade for everything → `Transform` is smoother, GPU-accelerated

---

## 🎯 Context Variables (Identify First)

| Variable | Questions |
|----------|-----------|
| **Screen type** | Entry? Detail? Modal? Overlay? Fullscreen? |
| **Primary action** | One clear CTA? What does user DO? |
| **Data lifecycle** | One-time load? Real-time stream? User-driven? |
| **Navigation context** | Pushed? Replaced? In tab? At root? |
| **Animation purpose** | Feedback? Delight? Orientation? Branding? |
| **State scope** | Widget-local? Screen? Cross-screen? Global? |
| **Platform behavior** | iOS back swipe? Android back? Both? |
| **Density** | Phone? Tablet? How does layout adapt? |

---

## 🔍 Widget Selection

### Container vs SizedBox vs Padding vs Align

```
Fixed size only (no decoration) → SizedBox(width: x, height: y)
Spacing between widgets         → SizedBox(height: 8) or Spacer
Padding only                    → Padding(padding: EdgeInsets.all(16))
Center a child                  → Center() or Align(alignment: ...)
Color + border + shadow         → Container (now justified)
Clip to shape                   → ClipRRect, ClipOval, ClipPath
```

### Widget Type Decision

```
Pure display (no state, no providers) → StatelessWidget
Animation controller/focus node       → StatefulWidget
Reads Riverpod provider only          → ConsumerWidget
Riverpod + animation/focus            → ConsumerStatefulWidget
```

---

## ❓ Design Intent Questions (Ask Every Time)

1. "Is this widget reused or single-use?" → Single-use: simplicity > abstraction
2. "What happens when data is loading?" → Define loading state first
3. "What happens when data fails?" → Error state + retry always
4. "What happens on 320dp width?" → Test minimum width
5. "What happens at 200% text scale?" → Accessibility overflow
6. "What does pressing back do?" → Test ALL back paths
7. "Is this animation communicating something?" → If not, remove it
8. "Does this need to work offline?" → Define caching strategy

---

> Best Flutter widget = does exactly what's needed, nothing more. Question every default. Own every pixel with intention, not habit.
