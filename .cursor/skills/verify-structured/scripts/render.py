#!/usr/bin/env python3
"""Turn a `tmux capture-pane -e` dump into a standalone HTML page that looks like the terminal."""
import html
import re
import sys

BASIC = [
    "#1d1f21", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2", "#c5c8c6",
    "#5c6370", "#ef8189", "#a9d18e", "#f0d08c", "#7cbcf5", "#d49ae6", "#7fcfd9", "#ffffff",
]
BG, FG = "#16171d", "#d7d9e0"
SGR = re.compile(r"\x1b\[([0-9;:]*)m")


def xterm(n):
    if n < 16:
        return BASIC[n]
    if n < 232:
        n -= 16
        steps = [0, 95, 135, 175, 215, 255]
        return "#%02x%02x%02x" % (steps[n // 36], steps[n // 6 % 6], steps[n % 6])
    level = 8 + (n - 232) * 10
    return "#%02x%02x%02x" % (level, level, level)


def color(codes, i):
    if codes[i + 1] == 2:
        return "#%02x%02x%02x" % tuple(codes[i + 2 : i + 5]), i + 5
    return xterm(codes[i + 2]), i + 3


def apply(state, params):
    codes = [int(c) if c else 0 for c in re.split("[;:]", params)] if params else [0]
    i = 0
    while i < len(codes):
        c = codes[i]
        if c == 0:
            state.clear()
        elif c in (1, 2, 3, 4, 7, 9):
            state[c] = True
        elif c == 22:
            state.pop(1, None)
            state.pop(2, None)
        elif c in (23, 24, 27, 29):
            state.pop(c - 20, None)
        elif 30 <= c <= 37 or 90 <= c <= 97:
            state["fg"] = BASIC[c - 30 if c < 90 else c - 82]
        elif 40 <= c <= 47 or 100 <= c <= 107:
            state["bg"] = BASIC[c - 40 if c < 100 else c - 92]
        elif c == 39:
            state.pop("fg", None)
        elif c == 49:
            state.pop("bg", None)
        elif c in (38, 48) and i + 1 < len(codes):
            value, i = color(codes, i)
            state["fg" if c == 38 else "bg"] = value
            continue
        i += 1


def style(state):
    fg, bg = state.get("fg", FG), state.get("bg")
    if state.get(7):
        fg, bg = bg or BG, fg
    rules = [f"color:{fg}"]
    if bg:
        rules.append(f"background:{bg}")
    if state.get(1):
        rules.append("font-weight:700")
    if state.get(2):
        rules.append("opacity:.6")
    if state.get(3):
        rules.append("font-style:italic")
    deco = [name for flag, name in ((4, "underline"), (9, "line-through")) if state.get(flag)]
    if deco:
        rules.append("text-decoration:" + " ".join(deco))
    return ";".join(rules)


def convert(text):
    state, out = {}, []
    for line in text.split("\n"):
        pos, parts = 0, []
        for match in SGR.finditer(line):
            if match.start() > pos:
                parts.append(f'<span style="{style(state)}">{html.escape(line[pos:match.start()])}</span>')
            apply(state, match.group(1))
            pos = match.end()
        parts.append(f'<span style="{style(state)}">{html.escape(line[pos:])}</span>')
        out.append("".join(parts))
    return "\n".join(out)


if __name__ == "__main__":
    source, target = sys.argv[1], sys.argv[2]
    body = convert(open(source, encoding="utf-8").read().rstrip("\n"))
    page = f"""<!doctype html><meta charset="utf-8"><style>
body{{margin:0;background:{BG}}}
pre{{margin:0;padding:14px 16px;font:14px/1.25 "JetBrains Mono","SF Mono",Menlo,monospace;color:{FG};background:{BG};display:inline-block}}
</style><pre>{body}</pre>"""
    open(target, "w", encoding="utf-8").write(page)
