"""Checks WCAG contrast of the colour tokens in crates/web/style.css.
Usage: python3 -I tools/contrast.py"""
import re, sys

css = open("crates/web/style.css").read()
light_block = css[css.index(":root {"):css.index("@media (prefers-color-scheme: dark)")]
dark_block = css[css.index("@media (prefers-color-scheme: dark)"):css.index("/* ---", css.index("@media (prefers-color-scheme: dark)"))]
sheet_block = css[css.index(".sheet {"):]

def tokens(block):
    return dict(re.findall(r"--([\w-]+):\s*(#[0-9a-fA-F]{6})", block))

def lum(hexc):
    r, g, b = (int(hexc[i:i + 2], 16) / 255 for i in (1, 3, 5))
    f = lambda c: c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)

def ratio(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)

# (foreground, background, minimum): text 4.5, UI components / large text 3.
pairs = [
    ("ink", "surface", 4.5), ("ink", "bg", 4.5), ("ink", "surface-2", 4.5),
    ("muted", "surface", 4.5), ("muted", "bg", 4.5), ("muted", "surface-2", 4.5),
    ("accent", "surface", 4.5), ("accent", "bg", 4.5), ("accent-ink", "accent", 4.5),
    ("ok", "surface", 4.5), ("ok", "ok-soft", 4.5), ("warn", "surface", 4.5), ("warn", "warn-soft", 4.5),
    ("danger", "surface", 4.5), ("ink", "accent-soft", 4.5), ("accent", "accent-soft", 4.5),
    ("line-strong", "surface", 3.0), ("line-strong", "bg", 3.0), ("focus", "surface", 3.0), ("focus", "bg", 3.0),
]
failed = 0
for name, block in (("light", light_block), ("dark", dark_block)):
    t = tokens(light_block) | tokens(block)
    for fg, bg, need in pairs:
        r = ratio(t[fg], t[bg])
        ok = r >= need
        failed += not ok
        print(f"{name:5} {fg:>12} on {bg:<12} {r:5.2f}:1 {'ok' if ok else 'FAIL (needs %.1f)' % need}")
s = tokens(sheet_block)
for fg in ("ink-s", "label", "accent-s"):
    r = ratio(s[fg], "#ffffff")
    failed += r < 4.5
    print(f"sheet {fg:>12} on white        {r:5.2f}:1 {'ok' if r >= 4.5 else 'FAIL'}")
sys.exit(1 if failed else 0)
