"""
HARAN v23 Part T — structural checks for the single-file front-end (haran.html).
================================================================================
These are STRUCTURAL checks (presence, well-formedness, wiring) — NOT visual/feel checks. The look,
colors, transitions, tone, and streaming *feel* of the screen are a USER-CONFIRMATION subject and
cannot be asserted by an automated test (★ Part T discipline ★). We never claim "the UI looks right".
"""
from __future__ import annotations

import re
from html.parser import HTMLParser
from pathlib import Path

HARAN_HTML = Path(__file__).with_name("haran.html")


def html() -> str:
    return HARAN_HTML.read_text(encoding="utf-8")


def has(needle: str, text: str | None = None) -> bool:
    return needle in (text if text is not None else html())


def count(needle: str, text: str | None = None) -> int:
    return (text if text is not None else html()).count(needle)


class _Balance(HTMLParser):
    """Tolerant well-formedness check: every non-void start tag has a matching end tag (nesting may be
    flat for our purposes; we just confirm the parser doesn't choke and key containers balance)."""
    VOID = {"meta", "br", "img", "input", "hr", "link", "area", "base", "col", "embed",
            "source", "track", "wbr", "!doctype"}

    def __init__(self):
        super().__init__()
        self.stack = []
        self.ok = True

    def handle_starttag(self, tag, attrs):
        if tag not in self.VOID:
            self.stack.append(tag)

    def handle_endtag(self, tag):
        if tag in self.VOID:
            return
        if self.stack and self.stack[-1] == tag:
            self.stack.pop()
        elif tag in self.stack:           # close to the nearest matching ancestor
            while self.stack and self.stack.pop() != tag:
                pass
        else:
            self.ok = False               # stray end tag


def well_formed(text: str | None = None) -> bool:
    t = text if text is not None else html()
    p = _Balance()
    try:
        p.feed(t)
    except Exception:                     # noqa: BLE001
        return False
    return p.ok and not p.stack           # everything opened was closed


def _obj_block(name: str, text: str) -> str:
    """Return the brace-balanced body of a JS object literal `name: { ... }` (best-effort)."""
    m = re.search(rf"\b{name}\s*:\s*\{{", text)
    if not m:
        return ""
    i = m.end() - 1
    depth, start = 0, i
    for j in range(i, len(text)):
        if text[j] == "{":
            depth += 1
        elif text[j] == "}":
            depth -= 1
            if depth == 0:
                return text[start + 1:j]
    return ""


def i18n_keys(lang: str, text: str | None = None) -> set:
    """Keys defined for a language inside the I18N object (e.g. i18n_keys('ko'))."""
    t = text if text is not None else html()
    block = _obj_block(lang, t)
    return set(re.findall(r"(\w+)\s*:", block))


def data_i18n_keys(text: str | None = None) -> set:
    t = text if text is not None else html()
    return set(re.findall(r'data-i18n\s*=\s*"([^"]+)"', t))


def js_balanced(text: str | None = None) -> bool:
    """Cheap JS sanity: braces/parens/brackets balance across the file (catches truncation)."""
    t = text if text is not None else html()
    pairs = {")": "(", "]": "[", "}": "{"}
    stack = []
    in_str = None
    esc = False
    for ch in t:
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == in_str:
                in_str = None
            continue
        if ch in "\"'`":
            in_str = ch
        elif ch in "([{":
            stack.append(ch)
        elif ch in ")]}":
            if not stack or stack.pop() != pairs[ch]:
                return False
    return not stack
