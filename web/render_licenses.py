#!/usr/bin/env python3
"""Renders web/fonts/LICENSES.md to web/fonts/LICENSES.html for the demo's footer link.

Standard library only, so the build needs no Markdown package. It handles the subset the file uses:
`#` headings, paragraphs, pipe tables, `-` lists (with indented continuation lines), `code`,
[text](url) and <url> links. Anything else stops the build rather than rendering wrongly.

Usage: render_licenses.py [input.md] [output.html]
"""
import html
import re
import sys
from pathlib import Path

WEB = Path(__file__).resolve().parent
SRC = Path(sys.argv[1]) if len(sys.argv) > 1 else WEB / "fonts" / "LICENSES.md"
DST = Path(sys.argv[2]) if len(sys.argv) > 2 else SRC.with_suffix(".html")

INLINE = re.compile(r"`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)|<(https?://[^>\s]+)>")


def inline(text: str) -> str:
    out, pos = [], 0
    for m in INLINE.finditer(text):
        out.append(html.escape(text[pos:m.start()], quote=False))
        code, label, href, auto = m.groups()
        if code is not None:
            out.append(f"<code>{html.escape(code, quote=False)}</code>")
        elif label is not None:
            # A link to the Markdown file itself would point at the unrendered source.
            href = re.sub(r"\.md$", ".html", href)
            out.append(f'<a href="{html.escape(href)}">{inline(label)}</a>')
        else:
            out.append(f'<a href="{html.escape(auto)}">{html.escape(auto, quote=False)}</a>')
        pos = m.end()
    out.append(html.escape(text[pos:], quote=False))
    return "".join(out)


def cells(row: str) -> list[str]:
    return [c.strip() for c in row.strip().strip("|").split("|")]


def convert(md: str) -> tuple[str, str]:
    lines = md.splitlines()
    body, title, i = [], None, 0
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
        elif m := re.match(r"(#{1,6})\s+(.*)", line):
            level, text = len(m.group(1)), m.group(2).strip()
            title = title or text
            body.append(f"<h{level}>{inline(text)}</h{level}>")
            i += 1
        elif line.lstrip().startswith("|"):
            rows = []
            while i < len(lines) and lines[i].lstrip().startswith("|"):
                rows.append(cells(lines[i]))
                i += 1
            if len(rows) < 2 or not all(re.fullmatch(r":?-+:?", c) for c in rows[1]):
                sys.exit(f"{SRC}: table without a header separator row")
            head = "".join(f"<th>{inline(c)}</th>" for c in rows[0])
            trs = "".join(
                "<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>\n" for r in rows[2:]
            )
            body.append(f"<div class=\"scroll\"><table>\n<thead><tr>{head}</tr></thead>\n"
                        f"<tbody>\n{trs}</tbody>\n</table></div>")
        elif re.match(r"[-*]\s", line):
            items = []
            while i < len(lines) and (re.match(r"[-*]\s", lines[i])
                                      or (items and lines[i].startswith(" ") and lines[i].strip())):
                if re.match(r"[-*]\s", lines[i]):
                    items.append(lines[i][2:].strip())
                else:
                    items[-1] += " " + lines[i].strip()
                i += 1
            body.append("<ul>\n" + "".join(f"<li>{inline(t)}</li>\n" for t in items) + "</ul>")
        elif re.match(r"\s*(\d+\.|>|```|    )", line):
            sys.exit(f"{SRC}:{i + 1}: unsupported Markdown, extend {Path(__file__).name}: {line!r}")
        else:
            para = []
            while i < len(lines) and lines[i].strip() and not re.match(r"#|\||[-*]\s", lines[i]):
                para.append(lines[i].strip())
                i += 1
            body.append(f"<p>{inline(' '.join(para))}</p>")
    return title or "Licenses", "\n".join(body)


PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · formel</title>
<style>
  :root {{ color-scheme: light dark; --bg: #f6f6f3; --surface: #ffffff; --text: #1c1c1e;
    --muted: #6b6b70; --line: #dedcd6; --accent: #2f4fb3;
    --mono: ui-monospace, "SF Mono", "Cascadia Code", Menlo, Consolas, monospace; }}
  @media (prefers-color-scheme: dark) {{
    :root {{ --bg: #17181b; --surface: #202226; --text: #ececec; --muted: #9a9ca3;
      --line: #34363c; --accent: #8aa4ff; }}
  }}
  body {{ margin: 0; background: var(--bg); color: var(--text);
    font: 15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; }}
  main {{ max-width: 64rem; margin: 0 auto; padding: 1.25rem 1rem 3rem; }}
  h1 {{ font: 600 1.6rem/1.2 "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif; }}
  a {{ color: var(--accent); }}
  code {{ font-family: var(--mono); font-size: .9em; }}
  .scroll {{ overflow-x: auto; }}
  table {{ border-collapse: collapse; background: var(--surface); font-size: .9rem; }}
  th, td {{ border: 1px solid var(--line); padding: .4rem .6rem; text-align: left; vertical-align: top; }}
  th {{ color: var(--muted); font-weight: 600; }}
  .back {{ font-size: .9rem; }}
</style>
</head>
<body>
<main>
<p class="back"><a href="../">← Back to formel</a></p>
{body}
</main>
</body>
</html>
"""

title, body = convert(SRC.read_text(encoding="utf-8"))
DST.write_text(PAGE.format(title=html.escape(title, quote=False), body=body), encoding="utf-8")
print(f"wrote {DST}")
