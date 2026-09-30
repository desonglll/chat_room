#!/usr/bin/env python3
"""Fail when the shipping Web client's bundle exceeds its transfer budget.

The budget is on **gzipped total**, not raw entry size, for two reasons.

Transfer size is what costs the user time; raw bytes are not. And a single entry
chunk understates the cost once code splitting is in play, so measuring only
`index-*.js` would let total weight grow while the number being checked stayed
flat.

History: until 2026-09-30 this script measured `web/dist/assets/app.js` — the Vue
client. TG-003 repointed the embedded bundle at `packages/web`, so the budget was
being enforced on a frozen client nobody edits (440 KB, comfortably passing)
while what actually shipped was never measured at all. CI hid it by building both.

The budget is a guardrail against losing control, not a target. It is expected to
need raising — most likely at M3, when lottie-web arrives for animated stickers.
Raise it deliberately, in a commit that says what grew and why. Raising it to
silence a surprise is how a gate stops meaning anything.

2026-10-01 (integration lead, M3): split into two budgets. By M3 the single total
was 464 KB gzip, but 234 KB of it was lazy chunks (lottie workers, plyr, the
emoji database, the sticker panel) that only load when a feature is opened. One
number could no longer tell "startup got heavier" from "a feature got a lazy
chunk", so it would have been raised blindly. Now:

* initial — what `index.html` loads before first paint (entry script, its
  modulepreloads, stylesheets). This is the user-facing cost and the strict one.
* total — every JS/CSS asset. A loose ceiling so lazy chunks cannot grow unbounded.
"""
import re

import gzip
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "packages" / "web" / "dist" / "assets"
BUILD_HINT = "bun run -F '@tg/web' build"

INDEX_HTML = ROOT / "packages" / "web" / "dist" / "index.html"

# Gzipped size of what index.html loads before first paint (230 KB at the split).
MAX_INITIAL_GZIP_BYTES = 300_000
# Gzipped total of every JS and CSS asset, lazy chunks included (464 KB at the split).
MAX_GZIP_BYTES = 1_500_000


def initial_assets() -> set[str]:
    html = INDEX_HTML.read_text(encoding="utf-8")
    return {
        Path(ref).name
        for ref in re.findall(r'(?:src|href)="([^"]+\.(?:js|css))"', html)
    }


def gzip_size(path: Path) -> int:
    return len(gzip.compress(path.read_bytes(), 6))


def main() -> int:
    if not ASSETS.is_dir():
        print(f"error: missing {ASSETS.relative_to(ROOT)}; run `{BUILD_HINT}` first")
        return 1

    files = sorted(p for p in ASSETS.iterdir() if p.suffix in {".js", ".css"})
    if not files:
        print(f"error: no .js or .css in {ASSETS.relative_to(ROOT)}; run `{BUILD_HINT}` first")
        return 1

    sizes = [(p, p.stat().st_size, gzip_size(p)) for p in files]
    total_raw = sum(raw for _, raw, _ in sizes)
    total_gzip = sum(gz for _, _, gz in sizes)

    for path, raw, gz in sorted(sizes, key=lambda entry: -entry[2]):
        print(f"  {path.name}  {raw} raw  {gz} gzip")

    initial = initial_assets()
    missing = initial - {p.name for p, _, _ in sizes}
    if not initial or missing:
        print(f"error: index.html references unknown assets {sorted(missing)}; rebuild")
        return 1
    initial_gzip = sum(gz for p, _, gz in sizes if p.name in initial)
    print(
        f"initial load: {len(initial)} asset(s), {initial_gzip} gzip "
        f"(budget {MAX_INITIAL_GZIP_BYTES} gzip)"
    )
    if initial_gzip >= MAX_INITIAL_GZIP_BYTES:
        print("error: initial load over budget. Lazy-load the feature (`import()`), or raise")
        print("MAX_INITIAL_GZIP_BYTES in a commit that says what grew and why.")
        return 1

    summary = (
        f"{len(sizes)} asset(s), {total_raw} raw, {total_gzip} gzip "
        f"(budget {MAX_GZIP_BYTES} gzip)"
    )

    if total_gzip >= MAX_GZIP_BYTES:
        print(f"error: web bundle over budget: {summary}")
        print("Reduce the bundle, or raise MAX_GZIP_BYTES in a commit that says what grew and why.")
        return 1

    headroom = MAX_GZIP_BYTES - total_gzip
    print(f"web bundle check passed: {summary}, {headroom} gzip headroom")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
