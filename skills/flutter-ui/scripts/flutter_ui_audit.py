#!/usr/bin/env python3
"""
Flutter UI Audit Tool
Scans Flutter project for common UI, state, navigation, and animation issues.
Usage: python flutter_ui_audit.py <project_path>
       python flutter_ui_audit.py .
"""

import os
import re
import sys
import argparse
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Tuple


@dataclass
class Finding:
    severity: str  # RED, YELLOW, GREEN
    file: str
    line: int
    rule: str
    message: str
    fix: str


@dataclass
class AuditResult:
    findings: List[Finding] = field(default_factory=list)

    def add(self, severity: str, file: str, line: int, rule: str, message: str, fix: str):
        self.findings.append(Finding(severity, file, line, rule, message, fix))

    @property
    def red(self) -> List[Finding]:
        return [f for f in self.findings if f.severity == "RED"]

    @property
    def yellow(self) -> List[Finding]:
        return [f for f in self.findings if f.severity == "YELLOW"]

    @property
    def green(self) -> List[Finding]:
        return [f for f in self.findings if f.severity == "GREEN"]


def read_dart_files(project_path: str) -> List[Tuple[str, List[str]]]:
    """Returns list of (filepath, lines) for all .dart files in lib/."""
    files = []
    lib_path = Path(project_path) / "lib"
    if not lib_path.exists():
        lib_path = Path(project_path)

    for dart_file in lib_path.rglob("*.dart"):
        try:
            with open(dart_file, "r", encoding="utf-8") as f:
                lines = f.readlines()
            files.append((str(dart_file), lines))
        except Exception:
            pass
    return files


def detect_state_manager(all_files: List[Tuple[str, List[str]]]) -> str:
    """Detect which state manager is used."""
    for _, lines in all_files:
        content = "".join(lines)
        if "flutter_riverpod" in content or "hooks_riverpod" in content:
            return "riverpod"
        if "package:provider/" in content:
            return "provider"
    return "unknown"


def check_animation_issues(result: AuditResult, filepath: str, lines: List[str]):
    """Check for animation anti-patterns."""
    content = "".join(lines)

    # AnimationController without dispose
    if "AnimationController(" in content and ".dispose()" not in content:
        result.add(
            "RED", filepath, 0,
            "ANIM_NO_DISPOSE",
            "AnimationController created but no dispose() found",
            "Add _controller.dispose() in the dispose() method"
        )

    for i, line in enumerate(lines, 1):
        stripped = line.strip()

        # WillPopScope (deprecated)
        if "WillPopScope" in stripped:
            result.add(
                "RED", filepath, i,
                "NAV_WILL_POP_SCOPE",
                "WillPopScope is deprecated (Flutter 3.12+)",
                "Replace with PopScope(canPop: ..., onPopInvokedWithResult: ...)"
            )

        # addListener with setState
        if "addListener" in stripped and "setState" in stripped:
            result.add(
                "RED", filepath, i,
                "ANIM_SET_STATE_LISTENER",
                "addListener with setState causes full widget rebuild",
                "Use AnimatedBuilder(animation: ..., builder: ..., child: staticChild)"
            )

        # Animating width/height directly
        if re.search(r"Tween<double>\s*\(", stripped) and ("width:" in stripped or "height:" in stripped):
            result.add(
                "YELLOW", filepath, i,
                "ANIM_LAYOUT_PROP",
                "Potentially animating layout property — causes jank",
                "Animate Transform.scale/translate instead of width/height"
            )

        # No curve in CurvedAnimation check
        if "Duration(milliseconds: 0)" in stripped or "Duration.zero" in stripped:
            result.add(
                "YELLOW", filepath, i,
                "ANIM_ZERO_DURATION",
                "Zero-duration animation is meaningless",
                "Remove the animation or use minimum 150ms"
            )


def check_navigation_issues(result: AuditResult, filepath: str, lines: List[str]):
    """Check for navigation anti-patterns."""
    content = "".join(lines)

    # Detect if GoRouter is used
    uses_go_router = "go_router" in content or "GoRouter" in content

    for i, line in enumerate(lines, 1):
        stripped = line.strip()

        # Navigator.push in GoRouter app
        if uses_go_router and "Navigator.of(context).push(" in stripped:
            result.add(
                "YELLOW", filepath, i,
                "NAV_PUSH_IN_GO_ROUTER",
                "Navigator.push bypasses GoRouter — breaks deep links",
                "Use context.go() or context.push() instead"
            )

        # Hardcoded route strings
        if re.search(r'Navigator\.(push|pop|pushReplacement).*["\']/', stripped):
            result.add(
                "YELLOW", filepath, i,
                "NAV_HARDCODED_ROUTE",
                "Hardcoded route string — typo-prone, no autocomplete",
                "Use centralized route constants or GoRouter named routes"
            )

        # Back button with no handling at root
        if "PopScope" not in content and "canPop" not in content and "WillPopScope" not in content:
            # Only flag once per file if it looks like a screen
            if "Scaffold(" in content and i == 1:
                result.add(
                    "GREEN", filepath, i,
                    "NAV_NO_POP_SCOPE",
                    "No PopScope found — back button behavior may be unhandled",
                    "Add PopScope at root screen with canPop: false if needed"
                )


def check_state_issues_riverpod(result: AuditResult, filepath: str, lines: List[str]):
    """Check Riverpod-specific anti-patterns."""
    content = "".join(lines)

    for i, line in enumerate(lines, 1):
        stripped = line.strip()

        # ref.read() in build
        if "ref.read(" in stripped and "build(" not in stripped:
            # Heuristic: check if this line is inside a build method
            # Look for ref.read at the start of a line that's not in a callback
            pass  # Complex to detect accurately without AST

        # Provider created inside widget class body (simplified check)
        if re.search(r"^\s*(final|var)\s+\w+Provider\s*=\s*(Provider|NotifierProvider|AsyncNotifierProvider)", line):
            if "class " not in content[max(0, content.find(line)-500):content.find(line)]:
                pass  # Outside class, OK
            else:
                result.add(
                    "RED", filepath, i,
                    "RIVERPOD_PROVIDER_IN_CLASS",
                    "Provider defined inside a class — creates new instance per build",
                    "Move provider definition to file top-level"
                )

        # .when() without error
        if ".when(" in stripped and "error:" not in stripped:
            # Check next few lines for error handler
            window = "".join(lines[i-1:min(i+10, len(lines))])
            if "error:" not in window:
                result.add(
                    "RED", filepath, i,
                    "RIVERPOD_NO_ERROR_HANDLER",
                    "AsyncValue.when() called without error: handler",
                    "Add error: (e, stack) => ErrorWidget(error: e, onRetry: ...)"
                )

        # StatefulWidget with ref (should be ConsumerStatefulWidget)
        if "extends StatefulWidget" in stripped:
            # Check surrounding context for ref usage
            class_end = content.find("}", content.find(stripped))
            class_content = content[content.find(stripped):class_end+500]
            if "WidgetRef" in class_content or "ref." in class_content:
                result.add(
                    "RED", filepath, i,
                    "RIVERPOD_WRONG_WIDGET_TYPE",
                    "StatefulWidget uses ref — should be ConsumerStatefulWidget",
                    "Change to ConsumerStatefulWidget / ConsumerState<>"
                )


def check_state_issues_provider(result: AuditResult, filepath: str, lines: List[str]):
    """Check Provider package anti-patterns."""
    content = "".join(lines)

    for i, line in enumerate(lines, 1):
        stripped = line.strip()

        # context.read in build (Provider)
        if "context.read<" in stripped or "Provider.of(" in stripped and "listen: true" in stripped:
            # Simplified — detect listen: true in handler context
            if "onPressed" not in "".join(lines[max(0,i-3):i]) and \
               "onTap" not in "".join(lines[max(0,i-3):i]):
                result.add(
                    "YELLOW", filepath, i,
                    "PROVIDER_READ_IN_BUILD",
                    "context.read<T>() in build() — not reactive, shows stale data",
                    "Use context.watch<T>() in build(), context.read<T>() only in callbacks"
                )

        # BuildContext in ChangeNotifier
        if "BuildContext" in stripped and "ChangeNotifier" in content:
            if "class " not in stripped:  # Field declaration
                result.add(
                    "RED", filepath, i,
                    "PROVIDER_CONTEXT_IN_NOTIFIER",
                    "BuildContext stored in ChangeNotifier — dangling context after navigation",
                    "Pass data (not context) to ChangeNotifier methods"
                )

        # notifyListeners before mutation (hard to detect accurately, flag pattern)
        if "notifyListeners()" in stripped:
            # Check if any mutation happens after this line in same method
            pass  # Would need AST for accurate detection


def check_performance_issues(result: AuditResult, filepath: str, lines: List[str]):
    """Check performance anti-patterns."""
    content = "".join(lines)

    for i, line in enumerate(lines, 1):
        stripped = line.strip()

        # ListView without builder (with children)
        if re.search(r"ListView\s*\(", stripped) and "builder" not in stripped:
            result.add(
                "RED", filepath, i,
                "PERF_LIST_VIEW_CHILDREN",
                "ListView() with children renders ALL items eagerly",
                "Use ListView.builder(itemCount: n, itemBuilder: ...)"
            )

        # Hardcoded color
        if re.search(r"Color\(0x[A-Fa-f0-9]+\)", stripped) and "scheme" not in stripped:
            result.add(
                "YELLOW", filepath, i,
                "PERF_HARDCODED_COLOR",
                "Hardcoded Color() — breaks theming and dark mode",
                "Use Theme.of(context).colorScheme.* or ThemeExtension"
            )

        # Hardcoded fontSize
        if re.search(r"fontSize:\s*\d+", stripped):
            result.add(
                "YELLOW", filepath, i,
                "PERF_HARDCODED_FONT_SIZE",
                "Hardcoded fontSize — breaks accessibility and text scaling",
                "Use Theme.of(context).textTheme.bodyLarge or similar"
            )

        # Opacity(opacity: 0) to hide
        if re.search(r"Opacity\s*\(\s*opacity:\s*0", stripped):
            result.add(
                "YELLOW", filepath, i,
                "PERF_OPACITY_HIDE",
                "Opacity(opacity: 0) widget still in tree and painting",
                "Use Visibility(visible: false) or conditional rendering"
            )

        # Image.network without cacheWidth
        if "Image.network(" in stripped and "cacheWidth" not in content[max(0, content.find(stripped)-200):content.find(stripped)+200]:
            result.add(
                "GREEN", filepath, i,
                "PERF_IMAGE_NO_CACHE_SIZE",
                "Image.network without cacheWidth/cacheHeight — decodes at full size",
                "Add cacheWidth/cacheHeight or use cached_network_image package"
            )

        # IntrinsicHeight/Width in list
        if ("IntrinsicHeight" in stripped or "IntrinsicWidth" in stripped):
            result.add(
                "YELLOW", filepath, i,
                "PERF_INTRINSIC_SIZE",
                "IntrinsicHeight/Width is O(N²) — avoid in lists",
                "Use fixed heights or SliverFixedExtentList instead"
            )

        # Missing const on StatelessWidget
        if re.search(r"class\s+\w+\s+extends\s+StatelessWidget", stripped):
            class_name = re.search(r"class\s+(\w+)", stripped)
            if class_name:
                name = class_name.group(1)
                # Check if constructor is const
                ctor_pattern = f"{name}({{super.key}}"
                ctor_area = content[content.find(stripped):content.find(stripped)+500]
                if "const " not in ctor_area[:ctor_area.find("@override")] and \
                   "{super.key}" in ctor_area[:ctor_area.find("@override")]:
                    result.add(
                        "YELLOW", filepath, i,
                        "PERF_MISSING_CONST_CTOR",
                        f"{name} StatelessWidget constructor should be const",
                        f"Add const keyword: const {name}({{super.key}});"
                    )


def print_results(result: AuditResult, show_all: bool = False):
    """Print audit results formatted."""
    severity_icon = {"RED": "🔴", "YELLOW": "🟡", "GREEN": "🟢"}
    severity_label = {"RED": "CRITICAL", "YELLOW": "WARNING", "GREEN": "INFO"}

    total = len(result.findings)
    if total == 0:
        print("✅ No issues found! Flutter UI audit passed.")
        return

    print(f"\n{'='*60}")
    print(f"FLUTTER UI AUDIT RESULTS")
    print(f"{'='*60}")
    print(f"🔴 Critical: {len(result.red)}  🟡 Warning: {len(result.yellow)}  🟢 Info: {len(result.green)}")
    print(f"{'='*60}\n")

    # Print by severity
    for severity in ["RED", "YELLOW", "GREEN"]:
        findings = [f for f in result.findings if f.severity == severity]
        if not findings:
            continue
        if severity == "GREEN" and not show_all:
            print(f"🟢 INFO: {len(findings)} info findings (use --all to show)\n")
            continue

        print(f"{severity_icon[severity]} {severity_label[severity]} ({len(findings)} findings):")
        print("-" * 40)
        for f in findings:
            loc = f":{f.line}" if f.line > 0 else ""
            short_file = f.file.split("/lib/")[-1] if "/lib/" in f.file else f.file
            print(f"  [{f.rule}] {short_file}{loc}")
            print(f"  ⚠  {f.message}")
            print(f"  ✅  Fix: {f.fix}")
            print()

    print(f"{'='*60}")
    if result.red:
        print("❌ Fix all 🔴 CRITICAL issues before shipping.")
    elif result.yellow:
        print("⚠️  No critical issues. Review 🟡 warnings.")
    else:
        print("✅ Only info findings. Good to go!")


def main():
    parser = argparse.ArgumentParser(
        description="Flutter UI Audit — scans for UI, state, navigation, and animation issues",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python flutter_ui_audit.py .
  python flutter_ui_audit.py /path/to/flutter_project
  python flutter_ui_audit.py . --all          # Show info findings too
  python flutter_ui_audit.py . --only red     # Only critical issues
        """
    )
    parser.add_argument("path", help="Flutter project path (must contain lib/ directory)")
    parser.add_argument("--all", action="store_true", help="Show all findings including INFO")
    parser.add_argument("--only", choices=["red", "yellow", "green"], help="Filter by severity")
    args = parser.parse_args()

    project_path = args.path
    if not os.path.exists(project_path):
        print(f"❌ Path not found: {project_path}")
        sys.exit(1)

    print(f"🔍 Scanning Flutter project: {os.path.abspath(project_path)}")
    dart_files = read_dart_files(project_path)

    if not dart_files:
        print(f"❌ No .dart files found in {project_path}/lib/")
        sys.exit(1)

    print(f"📁 Found {len(dart_files)} Dart files")

    state_manager = detect_state_manager(dart_files)
    print(f"📊 State manager detected: {state_manager.upper()}")
    print()

    result = AuditResult()

    for filepath, lines in dart_files:
        check_animation_issues(result, filepath, lines)
        check_navigation_issues(result, filepath, lines)
        check_performance_issues(result, filepath, lines)

        if state_manager == "riverpod":
            check_state_issues_riverpod(result, filepath, lines)
        elif state_manager == "provider":
            check_state_issues_provider(result, filepath, lines)
        else:
            check_state_issues_riverpod(result, filepath, lines)
            check_state_issues_provider(result, filepath, lines)

    # Filter if requested
    if args.only:
        severity_map = {"red": "RED", "yellow": "YELLOW", "green": "GREEN"}
        result.findings = [f for f in result.findings if f.severity == severity_map[args.only]]

    print_results(result, show_all=args.all)

    # Exit code: 1 if critical issues found
    sys.exit(1 if result.red else 0)


if __name__ == "__main__":
    main()
