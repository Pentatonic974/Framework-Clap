# Flutter Custom Paint

> Canvas, CustomPainter, pixel-perfect rendering, animation integration.

---

## 1. CustomPainter Structure

```dart
class MyPainter extends CustomPainter {
  const MyPainter({required this.value, required this.color});
  final double value;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..strokeWidth = 2
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;
    canvas.drawLine(Offset.zero, Offset(size.width * value, 0), paint);
  }

  @override
  bool shouldRepaint(MyPainter oldDelegate) =>
    value != oldDelegate.value || color != oldDelegate.color;
}

CustomPaint(
  painter: MyPainter(value: progress, color: Colors.blue),
  size: const Size(300, 4),
)
```

---

## 2. save() / restore() Discipline

Always pair — every `save()` must have a matching `restore()`.

```dart
@override
void paint(Canvas canvas, Size size) {
  canvas.save();
  canvas.translate(size.width / 2, size.height / 2);
  canvas.rotate(angle);
  _drawArm(canvas, length);
  canvas.restore();

  canvas.save();
  canvas.clipRRect(RRect.fromRectAndRadius(
    Rect.fromLTWH(0, 0, size.width, size.height),
    const Radius.circular(16),
  ));
  _drawBackground(canvas, size);
  canvas.restore();
}
```

---

## 3. Path Operations

```dart
final path = Path();

path.moveTo(x, y);           // lift pen
path.lineTo(x, y);           // line to point
path.close();                // close shape back to moveTo

path.quadraticBezierTo(cpX, cpY, endX, endY);
path.cubicTo(cp1X, cp1Y, cp2X, cp2Y, endX, endY);
path.conicTo(cpX, cpY, endX, endY, weight);

path.arcToPoint(Offset(endX, endY), radius: const Radius.circular(50), clockwise: true);
path.arcTo(Rect.fromCircle(center: Offset(cx, cy), radius: r), startAngle, sweepAngle, forceMoveTo);

final combined = Path.combine(
  PathOperation.difference, // union | intersection | xor | reverseDifference
  outerPath,
  innerPath,
);

canvas.drawPath(path, paint);
```

---

## 4. Paint Configuration

```dart
final paint = Paint()
  ..color = Colors.blue
  ..strokeWidth = 2.0
  ..style = PaintingStyle.stroke    // or .fill
  ..strokeCap = StrokeCap.round     // butt | round | square
  ..strokeJoin = StrokeJoin.round   // miter | round | bevel
  ..isAntiAlias = true
  ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 4.0)
  ..shader = RadialGradient(colors: [Colors.red, Colors.blue])
    .createShader(Rect.fromLTWH(0, 0, width, height));

canvas.drawShadow(path, Colors.black, 4.0, true);
```

---

## 5. Common Shapes

```dart
canvas.drawRRect(
  RRect.fromRectAndRadius(Rect.fromLTWH(x, y, width, height), const Radius.circular(12)),
  paint,
);

canvas.drawCircle(Offset(cx, cy), radius, paint);

canvas.drawArc(
  Rect.fromCircle(center: Offset(cx, cy), radius: radius),
  -math.pi / 2,           // start from top
  2 * math.pi * progress, // sweep angle
  false,                  // don't close to center
  paint,
);

canvas.drawLine(Offset(x1, y1), Offset(x2, y2), paint);

// Polygon
final path = Path();
for (int i = 0; i < sides; i++) {
  final angle = 2 * math.pi * i / sides - math.pi / 2;
  final x = cx + radius * math.cos(angle);
  final y = cy + radius * math.sin(angle);
  i == 0 ? path.moveTo(x, y) : path.lineTo(x, y);
}
path.close();
canvas.drawPath(path, paint);
```

---

## 6. Text on Canvas

```dart
final textPainter = TextPainter(
  text: TextSpan(
    text: 'Label',
    style: const TextStyle(color: Colors.white, fontSize: 14, fontWeight: FontWeight.w600),
  ),
  textDirection: TextDirection.ltr,
)..layout(maxWidth: size.width);

textPainter.paint(canvas, Offset(
  (size.width - textPainter.width) / 2,
  (size.height - textPainter.height) / 2,
));
```

---

## 7. Animation Integration

Pass `Animation<double>` as `repaint` listenable — triggers repaint on tick without calling `build()`.

```dart
class ProgressPainter extends CustomPainter {
  ProgressPainter({required this.progress, required Animation<double> repaint})
    : super(repaint: repaint); // repaint triggers on animation tick
  final double progress;

  @override
  void paint(Canvas canvas, Size size) { ... }

  @override
  bool shouldRepaint(ProgressPainter old) => progress != old.progress;
}

// Usage in widget
RepaintBoundary( // isolate to its own layer
  child: CustomPaint(
    painter: ProgressPainter(progress: _progress.value, repaint: _progress),
    size: const Size(100, 100),
  ),
)
```

---

## 8. Pixel-Perfect Rendering

Flutter canvas operates in logical pixels — framework handles DPR automatically.

```dart
// Crisp 1dp lines — offset by 0.5 to avoid sub-pixel blur
canvas.drawLine(
  Offset(x.roundToDouble() + 0.5, y),
  Offset(x2.roundToDouble() + 0.5, y),
  paint,
);

// High-DPR image capture
final recorder = PictureRecorder();
final canvas = Canvas(recorder);
final dpr = View.of(context).devicePixelRatio;
canvas.scale(dpr);
```

---

## 9. shouldRepaint Rules

```dart
// ✅ Compare fields — return false when unchanged
@override
bool shouldRepaint(MyPainter old) =>
  value != old.value || color != old.color || strokeWidth != old.strokeWidth;

// ❌ Always true — every-frame repaint
@override
bool shouldRepaint(MyPainter old) => true;
```

---

## 10. Semantic Accessibility

```dart
CustomPaint(
  painter: ChartPainter(data: chartData),
  child: Semantics(
    label: 'Bar chart showing monthly revenue. Highest: $maxValue in $peakMonth.',
    child: const SizedBox.expand(),
  ),
)
```
