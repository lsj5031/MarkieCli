#!/usr/bin/env python3
"""Source-grounded baseline audit. Run --inventory first, then optional MD-NNN IDs.

Only public CLI output is exercised. Each case retains its source, command,
stdout, stderr, SVG, structural assertions and raster preview. No network beyond
the temporary loopback HTTP server is used. The inventory is the report input.
"""
import argparse
import base64
import collections
import datetime
import functools
import html
import http.server
import json
import os
import re
from pathlib import Path
import subprocess
import threading
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
INPUTS = ROOT / '.amp/in/audit/markdown-inputs'
REPORT = ROOT / '.amp/in/audit/markdown-inventory.json'
OUT = ROOT / '.amp/in/artifacts/feature-audit/markdown'
BIN = ROOT / 'target/debug/markie'
ENV = dict(os.environ)
NS = '{http://www.w3.org/2000/svg}'
CASES = []


def case(area, feature, markdown, expected, source, checks=(), unsupported=False, **extra):
    CASES.append(dict(id=f'MD-{len(CASES)+1:03}', area=area, feature=feature,
                      markdown=markdown, expected=expected, source=source,
                      checks=list(checks), unsupported=unsupported, **extra))


# Expectations express user-visible contracts; deliberately asymmetric strings
# and nested scopes distinguish implementations that merely emit some text.
case('document', 'Empty document', '', 'Valid empty SVG with positive canvas height.', 'renderer.rs:455-602')
case('document', 'Paragraph separation', 'Alpha paragraph\n\nBeta paragraph', 'Both paragraphs appear on separate baselines.', 'renderer.rs:617-630,721-733', [('contains', 'Alpha'), ('below', 'Beta', 'Alpha')])
for level in range(1, 7):
    case('headings', f'ATX H{level}', '#' * level + ' UniqueHeading\n\nBody', f'H{level} is bold at its documented scale; body resets size and weight.', 'renderer.rs:605-616,712-721,2611-2631', [('heading', level)])
case('headings', 'Setext heading syntax', 'First\n=====\n\nSecond\n-----', 'Setext H1/H2 use the same descending bold sizes as ATX.', 'renderer.rs:605-616', [('size_gt', 'First', 'Second')])
case('headings', 'Heading inline emphasis', '# Heading *italic* **strong**\n\nPlain', 'Heading is bold; italic text remains italic; body resets.', 'renderer.rs:667-669,2625-2631', [('attr', 'italic', 'font-style', 'italic'), ('noattr', 'Plain', 'font-weight')])
case('headings', 'Heading attributes unsupported', '# Title {#anchor .wide}', 'Heading attribute extension is disabled; attribute syntax remains literal, no anchor.', 'renderer.rs:461-472,605', [('contains', '{#anchor'), ('no_element', 'a')], True)
case('text', 'Word wrapping at narrow width', 'Alpha ' + 'bravo charlie delta echo foxtrot ' * 5 + 'Zulu', 'Words wrap without losing the final token.', 'renderer.rs:821-938', [('contains', 'Zulu'), ('wrap',)])
case('text', 'Long unbroken word', 'Super' + 'califragilistic' * 30, 'Long body tokens remain readable within the output width through wrapping or scaling.', 'renderer.rs:879-887', [('bounds',)])
case('text', 'CJK and Unicode', '漢字日本語中文測試' * 15 + '\n\nKia ora Māori 😀 café e\u0301\n\n' + 'e\u0301' * 40 + '\n\n' + '🧑‍💻' * 30, 'Unicode is preserved and long CJK text remains inside the canvas.', 'renderer.rs:835-866,2715-2759', [('contains', 'Māori'), ('contains', '😀'), ('bounds',), ('cluster_runs', 'e\u0301', '\u0301'), ('cluster_runs', '🧑‍💻', '\u200d')])
case('text', 'Whitespace normalization', 'Alpha     Beta\t Gamma', 'Runs of normal whitespace collapse to one visible advance.', 'renderer.rs:835-866,940-1004', [('spacing',)])
case('text', 'Nonbreaking space semantics', 'Prefix A&nbsp;B', 'Nonbreaking space preserves the separation and prevents splitting the pair at a valid width boundary.', 'renderer.rs:835-866', [('nonbreak',)], width=128, variants=['light-default'])
case('text', 'Soft line break', 'Alpha\nBeta', 'Source soft break is rendered as a new line (current renderer policy).', 'renderer.rs:569-576,1655-1675', [('below', 'Beta', 'Alpha')])
case('text', 'Hard break by spaces', 'Alpha  \nBeta', 'Two-space hard break creates a new baseline.', 'renderer.rs:578-584', [('below', 'Beta', 'Alpha')])
case('text', 'Hard break by backslash', 'Alpha\\\nBeta', 'Backslash hard break creates a new baseline without showing the slash.', 'renderer.rs:578-584', [('below', 'Beta', 'Alpha'), ('absent', '\\')])
case('text', 'Escaped punctuation and entities', r'\*literal\* &amp; &lt;tag&gt; &#169; "quoted"', 'Escaped delimiters and XML-sensitive entities display as literal text without markup injection.', 'renderer.rs:455-459,2715-2759', [('contains', '*literal*'), ('contains', '<tag>'), ('contains', '©'), ('no_element', 'tag')])
case('text', 'Smart punctuation', '"Alpha" -- beta --- gamma...', 'Quotes, en/em dash and ellipsis are smartened.', 'renderer.rs:466', [('contains', '“Alpha”'), ('contains', '–'), ('contains', '—'), ('contains', '…')])
case('text', 'XML-illegal control filtering', '\x07# Heading\n\nA\x01B\x0cC', 'Illegal XML controls are stripped before parsing, preserving heading syntax and surrounding text.', 'renderer.rs:455-459,2729', [('contains', 'Heading'), ('contains', 'ABC'), ('absent', '\x07')])
case('inline', 'Bold nesting and reset', '**Alpha *Beta* Gamma** Delta', 'Strong and emphasis combine; Gamma remains bold, Delta resets.', 'renderer.rs:667-669,767-768', [('attr', 'Gamma', 'font-weight', '700'), ('attr', 'Beta', 'font-style', 'italic'), ('noattr', 'Delta', 'font-weight')])
case('inline', 'Emphasis nesting and reset', '*Alpha **Beta** Gamma* Delta', 'Gamma stays italic after strong ends; Delta resets.', 'renderer.rs:667-669,767-768', [('attr', 'Gamma', 'font-style', 'italic'), ('noattr', 'Delta', 'font-style')])
case('inline', 'Strikethrough decoration', 'Alpha ~~deleted words~~ Omega', 'Deleted words have strike lines; surrounding words remain undecorated.', 'renderer.rs:669,769,924-926', [('line_count', 2)])
case('inline', 'Unmatched delimiters', '**Alpha and `Beta', 'Unclosed Markdown delimiters remain readable literal content.', 'renderer.rs:474-602', [('contains', '**Alpha'), ('contains', '`Beta')])
case('code', 'Inline code escaping and spacing', 'A `<x> & y` B `next` C', 'Inline code preserves spaces and literal XML-sensitive text, with separate backgrounds.', 'renderer.rs:1056-1117', [('contains', '<x> & y'), ('code_count', 2)])
case('code', 'Long inline code span', '`' + 'long_token_' * 45 + '`', 'Long inline code is kept within the canvas or explicitly diagnosed.', 'renderer.rs:1056-1068', [('bounds',)])
case('code', 'Fenced untyped code', '```\nAlpha  Beta\n  Gamma\n```\n\nOmega', 'Code preserves indentation and line breaks; prose after it appears below.', 'renderer.rs:631-642,1372-1546', [('contains', 'Alpha  Beta'), ('contains', '  Gamma'), ('contains', 'Omega')])
case('code', 'Indented code', '    Alpha  Beta\n    Gamma\n\nOmega', 'Indented code renders in monospace with background.', 'renderer.rs:631-642,1372-1546', [('code_count', 2)])
case('code', 'Known language highlighting', '```rust\nfn main() { let x = "value"; }\n```', 'Known Rust keywords and strings use multiple syntax colors.', 'renderer.rs:1394-1445', [('highlight',)])
case('code', 'Unknown language fallback', '```madeuplanguage\nAlpha <Beta>\n```', 'Unknown language falls back to readable plain code without failing.', 'renderer.rs:1397-1404', [('contains', 'Alpha <Beta>')])
case('code', 'Fence info string', '```rust title=sample\nfn main() {}\n```', 'Code with an extended info string remains readable (highlighting fallback permitted).', 'renderer.rs:638-641,1397-1404', [('contains', 'fn main()')])
case('code', 'Long highlighted code wrapping', '```rust\nlet variable = "' + 'abcdef' * 70 + '";\n```', 'Long code lines wrap inside their background and preserve all characters.', 'renderer.rs:1577-1647', [('bounds',), ('code_payload', 'let variable = "' + 'abcdef' * 70 + '";')])
case('code', 'Empty fenced block', '```\n```\n\nOmega', 'Empty block retains a visible background and following prose.', 'renderer.rs:1456-1485', [('contains', 'Omega'), ('rect_min', 2)])
case('code', 'Blank lines inside code', '```\nAlpha\n\nBeta\n```', 'Interior blank code lines consume their own baseline.', 'renderer.rs:1438-1460,1577-1647', [('code_blank_gap',)])
case('code', 'Tabs inside code', '```\nA\tB\nC    D\n```', 'Tab-separated code retains readable separation and literal payload.', 'renderer.rs:1438-1460,1577-1647', [('contains', 'A\tB')])
case('lists', 'Unordered markers', '- Alpha\n- Beta\n- Gamma', 'Each item, including the first, has one visible bullet.', 'renderer.rs:643-658,2390-2444,2652-2667', [('marker_count', '•', 3)])
case('lists', 'Ordered non-one start', '7. Alpha\n8. Beta\n9. Gamma', 'Visible numbering starts at seven and increments for all three items.', 'renderer.rs:652-656,2652-2662', [('contains', '7.'), ('contains', '8.'), ('contains', '9.')])
case('lists', 'Nested mixed lists', '- Outer\n  3. InnerOne\n  4. InnerTwo\n- Sibling', 'Nested ordered markers and outer bullets are retained; sibling returns to outer indent.', 'renderer.rs:643-658,2390-2444,2669-2676', [('contains', '3.'), ('contains', '4.'), ('marker_count', '•', 2), ('same_x', 'Outer', 'Sibling')])
case('lists', 'Long continuation wrapping', '- Alpha ' + 'continuation words ' * 25 + '\n- Beta', 'Wrapped lines align with item text and the next bullet remains visible.', 'renderer.rs:879-887,2390-2444,2692-2695', [('contains', 'Beta'), ('marker_count', '•', 2), ('bounds',)])
case('lists', 'Loose list paragraphs', '- Alpha\n\n  Continuation\n\n- Beta', 'Loose paragraphs remain associated with the item without losing bullets.', 'renderer.rs:617-630,722-733,2390-2444', [('marker_count', '•', 2), ('below', 'Continuation', 'Alpha')])
case('lists', 'List item begins with code', '- `Alpha` followed\n- **Beta** followed', 'Code-led and formatted items each retain their bullet.', 'renderer.rs:905-920,1056-1117,2390-2444', [('marker_count', '•', 2)])
case('tasks', 'Unchecked task', '- [ ] Alpha', 'Unchecked task displays one empty checkbox with text.', 'renderer.rs:2110-2184', [('checkbox', False)])
case('tasks', 'Checked task lowercase', '- [x] Alpha', 'Checked task displays a checkmark inside its checkbox.', 'renderer.rs:2110-2184', [('checkbox', True)])
case('tasks', 'Checked task uppercase', '- [X] Alpha', 'Uppercase X is recognized as checked.', 'renderer.rs:468,587,2110-2184', [('checkbox', True)])
case('tasks', 'Nested and ordered tasks', '3. [ ] Alpha\n   - [x] Beta\n4. [ ] Gamma', 'Tasks replace list markers with aligned boxes at the correct nesting depth.', 'renderer.rs:2110-2184', [('check_count', 3, 1)])
case('tasks', 'Task wrapping', '- [x] Alpha ' + 'task continuation words ' * 20, 'Task continuation wraps to the text indent and retains checkbox.', 'renderer.rs:2110-2184,2692-2695', [('checkbox', True), ('bounds',)])
case('quotes', 'Basic quote', '> Alpha\n> Beta\n\nOmega', 'Quote has a vertical border and tint; following prose resets indent.', 'renderer.rs:659-662,739-744,2446-2495', [('contains', 'Alpha'), ('quote_border', 1), ('x_gt', 'Alpha', 'Omega'), ('quote_clearance',)])
case('quotes', 'Nested quotes', '> Alpha\n>\n> > Beta\n>\n> Gamma\n\nOmega', 'Nested quotes draw two borders and indent Beta farther than Alpha.', 'renderer.rs:2446-2495', [('quote_border', 2), ('x_gt', 'Beta', 'Alpha'), ('quote_clearance',)])
case('quotes', 'Quote list and code containers', '> - Alpha\n> - Beta\n>\n> ```\n> Gamma\n> ```', 'Quote preserves list markers, code background and container indentation.', 'renderer.rs:643-662,1372-1546,2446-2495', [('marker_count', '•', 2), ('contains', 'Gamma')])
case('quotes', 'GFM alert kind', '> [!WARNING]\n> Alpha', 'GFM alert text remains visible with generic quote styling; no special alert styling is implemented.', 'renderer.rs:472,659-662', [('contains', 'Alpha'), ('quote_border', 1)], True)
case('rules', 'Thematic rule and spacing', 'Alpha\n\n---\n\nBeta', 'Rule spans content width and separates surrounding paragraphs.', 'renderer.rs:1677-1709', [('rule',)])
case('links', 'Inline link styling', '[Alpha Beta](https://example.invalid/a "Title") Omega', 'Link label is colored and underlined; destination/title are not exported as clickable anchors.', 'renderer.rs:663,745,928-932,2633-2650', [('line_count', 3), ('no_element', 'a'), ('contains', 'Alpha')])
case('links', 'Reference links', '[Alpha][ref]\n\n[ref]: https://example.invalid', 'Resolved reference uses link styling without printing definition.', 'renderer.rs:663,745', [('line_count', 1), ('absent', 'example.invalid')])
case('links', 'Angle URL and email autolinks', '<https://example.invalid> <audit@example.invalid>', 'Angle autolinks have underlines and retain label text.', 'renderer.rs:663,745', [('line_count', 2), ('contains', 'audit@example.invalid')])
case('links', 'Bare URL GFM limitation', 'Visit https://example.invalid and www.example.invalid', 'Bare URL text is preserved; only parser-emitted links receive decoration.', 'renderer.rs:461-474,663', [('contains', 'https://example.invalid')], True)
case('links', 'Broken reference link', '[Alpha][missing]', 'Unresolved reference stays literal and readable.', 'renderer.rs:474,821-866', [('contains', '[Alpha][missing]')])
case('links', 'Long link wrapping', '[Alpha ' + 'link words ' * 25 + 'Omega](https://example.invalid)', 'Link label wraps with color and underline across lines.', 'renderer.rs:879-932,940-1004', [('contains', 'Omega'), ('wrap',), ('bounds',)])
case('tables', 'Header and body', '| HeaderA | HeaderB |\n|---|---|\n| Alpha | Beta |', 'Header text is bold and tinted; body retains both cells and borders.', 'renderer.rs:1711-1999', [('attr', 'HeaderA', 'font-weight', '700'), ('noattr', 'Alpha', 'font-weight'), ('contains', 'Beta')])
case('tables', 'Left center right alignment', '| LongLeft | LongCenter | LongRight |\n|:---|:---:|---:|\n| a | b | c |', 'Short cells align left, center and right within their respective columns.', 'renderer.rs:1933-1959', [('table_align',)])
case('tables', 'Long prose cell wrapping', '| A | B |\n|---|---|\n| short | ' + 'long words ' * 45 + '|', 'Rows grow to fit wrapped prose within the canvas.', 'renderer.rs:1859-1878,2001-2080', [('bounds',), ('wrap',)])
case('tables', 'Unbroken cell splitting', '| A | B |\n|---|---|\n| short | ' + 'x' * 240 + '|', 'Unbroken table words split without dropping characters.', 'renderer.rs:2001-2080', [('table_x_count', 240), ('bounds',)])
case('tables', 'Many narrow columns', '|' + '|'.join('Header' + str(n) for n in range(20)) + '|\n|' + '|'.join('---' for _ in range(20)) + '|\n|' + '|'.join('Value' for _ in range(20)) + '|', 'Many columns fit the canvas without text escaping cells.', 'renderer.rs:1834-1875,2001-2080', [('cell_bounds',), ('contains', ''.join('Header' + str(n) for n in range(20))), ('payload_count', 'Value', 20)])
case('tables', 'Inline emphasis flattened', '| A | B |\n|---|---|\n| **Alpha** | *Beta* ~~Gamma~~ |', 'Table body flattens emphasis and strike styling while preserving text; no warning is emitted.', 'renderer.rs:533-541,667-669,1969-1982', [('contains', 'Alpha'), ('noattr', 'Alpha', 'font-weight'), ('noattr', 'Beta Gamma', 'font-style')], True)
case('tables', 'Inline code flattened', '| A |\n|---|\n| `Alpha <Beta>` |', 'Inline code is retained as plain table text without a code box or monospace.', 'renderer.rs:542-549', [('contains', 'Alpha <Beta>'), ('table_plain',)], True)
case('tables', 'Inline HTML ignored', '| A |\n|---|\n| <span style="color:red">Alpha</span><br>Beta |', 'HTML tags and styling are ignored in tables, inner text retained without a forced break.', 'renderer.rs:552-568', [('contains', 'AlphaBeta'), ('fill_not', 'AlphaBeta', 'red')], True)
case('tables', 'Table link destination and style flattened', '| A |\n|---|\n| [Alpha](https://example.invalid) |', 'Link text remains plain in table cells without anchor or underline.', 'renderer.rs:533-541,663,1969-1982', [('contains', 'Alpha'), ('line_count', 0), ('no_element', 'a')], True)
case('tables', 'Image in table semantic preservation', '| A |\n|---|\n| ![AltText](pixel.png) |', 'Table image is embedded or reduced to readable alt text inside its cell, without corrupting table position.', 'renderer.rs:664-670,746-748,2186-2248', [('contains', 'AltText'), ('no_element', 'image')])
case('tables', 'Escaped pipe', '| A | B |\n|---|---|\n| Alpha\\|Beta | Gamma |', 'Escaped pipe stays inside one cell.', 'renderer.rs:1780-1788', [('contains', 'Alpha|Beta'), ('contains', 'Gamma')])
case('tables', 'Missing and extra cells', '| A | B |\n|---|---|\n| Alpha |\n| Beta | Gamma | Extra |', 'Missing cell is empty; parser discards surplus cell per table grammar.', 'renderer.rs:1746-1788', [('contains', 'Alpha'), ('contains', 'Gamma'), ('absent', 'Extra')])
case('footnotes', 'Reference and named definition', 'Alpha[^note] Omega\n\n[^note]: Definition', 'Named reference is raised and smaller; definition shows matching bracket marker.', 'renderer.rs:699-710,798-803,1232-1267', [('sup', 'note', 'Alpha'), ('contains', '[note]'), ('contains', 'Definition')])
case('footnotes', 'Repeated reference', 'Alpha[^n] Beta[^n]\n\n[^n]: Definition', 'Both references show the same label and the definition appears once.', 'renderer.rs:1232-1267,699-710', [('text_count', 'n', 2), ('text_count', 'Definition', 1)])
case('footnotes', 'Undefined reference', 'Alpha[^missing] Omega', 'Undefined footnote remains literal with no invented definition.', 'renderer.rs:474,589,1232-1267', [('contains', 'Alpha[^missing]')])
case('footnotes', 'Definition with multiple paragraphs', 'Alpha[^n]\n\n[^n]: First\n\n    Second\n\nOmega', 'Both footnote paragraphs display below the reference and ordinary prose resumes afterward.', 'renderer.rs:699-710,722-733,798-803', [('contains', 'First'), ('below', 'Second', 'First'), ('contains', 'Omega')])
case('definitions', 'Term and definition styles', 'Term\n: Definition\n\nOmega', 'Term is bold; definition is indented; ordinary prose resets.', 'renderer.rs:671-698,771-797', [('attr', 'Term', 'font-weight', '700'), ('x_gt', 'Definition', 'Term'), ('noattr', 'Omega', 'font-weight')])
case('definitions', 'Multiple definitions per term', 'Term\n: First\n: Second\n\nOther\n: Third', 'All definitions are retained at consistent indent on increasing baselines.', 'renderer.rs:684-698,790-797', [('below', 'Second', 'First'), ('same_x', 'First', 'Second'), ('contains', 'Third')])
case('definitions', 'Wrapped definition', 'Term\n: Alpha ' + 'definition words ' * 25 + '\n\nOmega', 'Definition wraps at its indent and prose after the list resets to document indent.', 'renderer.rs:691-698,2692-2695', [('bounds',), ('x_gt', 'Alpha', 'Omega')])
case('metadata', 'YAML front matter hidden', '---\ntitle: SECRET\n---\n\nAlpha', 'Leading YAML metadata is hidden and body displayed.', 'renderer.rs:469,490-496,670', [('contains', 'Alpha'), ('absent', 'SECRET')])
case('metadata', 'Plus front matter hidden', '+++\ntitle = "SECRET"\n+++\n\nAlpha', 'Leading plus metadata is hidden and body displayed.', 'renderer.rs:470,490-496,670', [('contains', 'Alpha'), ('absent', 'SECRET')])
case('metadata', 'Nonleading delimiter', 'Alpha\n\n---\n\nBeta', 'Nonleading dashes create a rule and do not hide later content.', 'renderer.rs:469,589,1677-1709', [('contains', 'Beta'), ('rule',)])
case('html', 'Block HTML displayed as code', '<div class="note">\nAlpha <b>Beta</b>\n</div>\n\nOmega', 'Block HTML source is displayed as code, retaining tags and text without execution.', 'renderer.rs:516-531', [('contains', 'div'), ('contains', 'Alpha'), ('contains', 'Omega'), ('no_element', 'div')])
case('html', 'Inline break forms', 'Alpha<br>Beta<BR/>Gamma<br />Delta', 'All three supported br spellings force new baselines.', 'renderer.rs:1124-1125', [('below', 'Beta', 'Alpha'), ('below', 'Gamma', 'Beta'), ('below', 'Delta', 'Gamma')])
case('html', 'Formatting alias tags', 'A <b>Bold</b> <i>Italic</i> <s>Strike</s> <em>Em</em> <strong>Strong</strong> <del>Deleted</del> Z', 'Plain HTML formatting aliases apply and close correctly.', 'renderer.rs:1126-1153', [('attr', 'Bold', 'font-weight', '700'), ('attr', 'Italic', 'font-style', 'italic'), ('line_count', 2), ('noattr', 'Z', 'font-weight')])
case('html', 'Formatting tag attributes', 'A <b class="x">Bold</b> <em title="x">Italic</em> Z', 'Formatting tags with harmless attributes preserve bold/italic intent.', 'renderer.rs:1126-1189', [('attr', 'Bold', 'font-weight', '700'), ('attr', 'Italic', 'font-style', 'italic')])
case('html', 'Superscript', 'Alpha<sup>Raised</sup> Omega', 'Superscript is smaller and raised, then resets.', 'renderer.rs:1172-1175,890-891', [('sup', 'Raised', 'Alpha'), ('same_size', 'Alpha', 'Omega')])
case('html', 'Subscript', 'Alpha<sub>Lowered</sub> Omega', 'Subscript is smaller and lower, then resets.', 'renderer.rs:1176-1179,890-891', [('sub', 'Lowered', 'Alpha'), ('same_size', 'Alpha', 'Omega')])
case('html', 'Underline continuous spaces', 'A <u>Alpha Beta</u> Z', 'Underline covers both words and intervening space, then ends.', 'renderer.rs:1180,928-932,940-1004', [('line_count', 3)])
case('html', 'Mark highlight theme contrast', 'A <mark>Alpha Beta</mark> Z', 'Highlight covers words and spaces; yellow in light, dark amber in dark theme.', 'renderer.rs:1181-1185,1201-1207,893-907,964-982', [('mark',)])
case('html', 'Span foreground colors', 'A <span style="color:#f00">Red</span> <font color="blue">Blue</font> Z', 'Span style and legacy font attribute set colors and restore surrounding text.', 'renderer.rs:237-272,2633-2638', [('attr', 'Red', 'fill', '#f00'), ('attr', 'Blue', 'fill', 'blue'), ('fill_not', 'Z', 'blue')])
case('html', 'Background aliases', 'A <span style="background-color:rgb(200, 210, 220)">Alpha</span> <font bgcolor="#abc">Beta</font> Z', 'CSS background and bgcolor create background rectangles.', 'renderer.rs:248-251,270,893-907', [('fill_exists', 'rgb(200, 210, 220)'), ('fill_exists', '#abc')])
case('html', 'Inline font-size em', 'A <span style="font-size:1.5em">Alpha</span> Z', '1.5em scales text to 1.5 times the surrounding font.', 'renderer.rs:255-259,277-299', [('scale', 'Alpha', 'A', 1.5)])
case('html', 'Inline font-size percent', 'A <span style="font-size:150%">Alpha</span> Z', '150% scales text to 1.5 times the surrounding font.', 'renderer.rs:277-299', [('scale', 'Alpha', 'A', 1.5)])
case('html', 'Inline font-size px', 'A <span style="font-size:16px">Alpha</span> Z', '16px matches the renderer 16px base; 8px would be half-size.', 'renderer.rs:277-299', [('scale', 'Alpha', 'A', 1.0)])
case('html', 'Inline size invalid values and clamp', 'A <span style="font-size:-1em">Invalid</span> <span style="font-size:100em">Huge</span> <span style="font-size:0.01em">Tiny</span> Z', 'Invalid negative value is ignored, positive scale clamps to 0.4 through 3.', 'renderer.rs:277-299', [('scale', 'Invalid', 'A', 1.0), ('scale', 'Huge', 'A', 3.0), ('scale', 'Tiny', 'A', 0.4)])
case('html', 'Nested color precedence', 'A <span style="color:red">Outer <span style="color:blue">Inner</span> Tail</span> Z', 'Innermost specified color wins; closing restores the outer color then theme.', 'renderer.rs:1210-1228', [('attr', 'Inner', 'fill', 'blue'), ('attr', 'Tail', 'fill', 'red'), ('fill_not', 'Z', 'red')])
case('html', 'Unstyled nested span scope', 'A <span style="color:red">Outer <span>Inner</span> Tail</span> Z', 'Unstyled span does not prematurely close the colored outer span.', 'renderer.rs:1165-1168,1194-1196', [('attr', 'Tail', 'fill', 'red')])
case('html', 'Unknown nested closing tag scope', 'A <span style="color:red">Outer <custom>Inner</custom> Tail</span> Z', 'Unknown nested tags preserve text without disturbing active supported style scopes.', 'renderer.rs:1165-1168,1189-1191', [('attr', 'Tail', 'fill', 'red')])
case('html', 'Self-closing tag scope', 'A <span style="color:red">Outer <custom/> Tail</span> Z', 'Self-closing unsupported tags do not change surrounding style.', 'renderer.rs:1161-1163', [('attr', 'Tail', 'fill', 'red')])
case('html', 'Nested size and baseline', 'A <sup>Outer<sub>Inner</sub>Tail</sup> Z', 'Nested sup/sub combine size and baseline; closing restores outer then normal size.', 'renderer.rs:1172-1179,1210-1228', [('scale', 'Inner', 'A', 0.49), ('scale', 'Tail', 'A', 0.7), ('same_size', 'A', 'Z')])
case('html', 'Style decoration CSS', 'A <span style="text-decoration:underline">Alpha Beta</span> Z', 'CSS underline draws contiguous line segments across spaces.', 'renderer.rs:252-254,928-932,940-1004', [('line_count', 3)])
case('html', 'Attribute quoting and case', "A <SPAN STYLE='color:green'>Alpha</SPAN> <font color=blue>Beta</font> Z", 'Attribute names/tags are case insensitive and single/unquoted values work.', 'renderer.rs:183-232,1230-1276', [('attr', 'Alpha', 'fill', 'green'), ('attr', 'Beta', 'fill', 'blue')])
case('html', 'Unsafe CSS rejected', 'A <span style="color:red;background:url(javascript:alert(1))" onclick="bad()">Alpha</span> Z', 'Unsafe CSS and event attributes never become executable SVG; safe color is retained.', 'renderer.rs:302-336', [('attr', 'Alpha', 'fill', 'red'), ('raw_absent', 'javascript'), ('raw_absent', 'onclick')])
case('html', 'Unknown inline tag content', 'A <custom>Alpha</custom> Z', 'Unknown inline tags are ignored while content remains visible; no warning.', 'renderer.rs:1189-1191', [('contains', 'Alpha'), ('no_element', 'custom')], True)
case('html', 'Unsupported HTML style properties', 'A <span style="font-weight:bold;font-style:italic;border:1px solid red">Alpha</span> Z', 'Unsupported CSS is ignored without diagnostics; content remains plain.', 'renderer.rs:244-262', [('noattr', 'Alpha', 'font-weight'), ('noattr', 'Alpha', 'font-style')], True)
case('images', 'Local PNG', 'Before\n\n![AltText](pixel.png)\n\nAfter', 'Relative PNG is embedded as data URL and surrounding text remains.', 'renderer.rs:2250-2336', [('image_count', 1), ('contains', 'After')])
case('images', 'Local JPEG', '![AltText](pixel.jpg)', 'Relative JPEG dimensions are read and image embedded.', 'renderer.rs:2313-2336,2390-2409', [('image_count', 1)])
case('images', 'Local SVG', '![AltText](shape.svg)', 'Relative SVG dimensions are read and image embedded.', 'renderer.rs:2371-2388', [('image_count', 1)])
case('images', 'Large image proportional fitting', '![AltText](wide.svg)', 'Oversized image scales proportionally inside content width.', 'renderer.rs:2213-2223', [('image_fit', 4.0)])
case('images', 'Small image natural dimensions', '![AltText](shape.svg)', 'Small 80×40 image is not enlarged.', 'renderer.rs:2213-2223', [('image_size', 80.0, 40.0)])
case('images', 'Missing image fallback', 'A ![MeaningfulAlt](missing.png) Z', 'Missing local file warns Failed to read image and shows alt as inline code.', 'renderer.rs:2197-2211,2319-2320', [('contains', 'MeaningfulAlt'), ('stderr', 'Failed to read image')])
case('images', 'Corrupt image fallback', 'A ![MeaningfulAlt](corrupt.png) Z', 'Invalid payload warns Failed to read image size and preserves alt.', 'renderer.rs:2197-2211,2383-2387', [('contains', 'MeaningfulAlt'), ('stderr', 'Failed to read image size')])
case('images', 'Absolute local path rejected', 'A ![MeaningfulAlt](/etc/passwd) Z', 'Absolute image warns absolute image paths are disallowed; preserves an alt fallback.', 'renderer.rs:2425-2436', [('stderr', 'absolute image paths are disallowed'), ('contains', 'MeaningfulAlt')])
case('images', 'Parent traversal rejected', 'A ![MeaningfulAlt](../outside.png) Z', 'Escaping base directory warns blocked potential path traversal; preserves alt fallback.', 'renderer.rs:2438-2506', [('stderr', 'blocked potential path traversal'), ('contains', 'MeaningfulAlt')])
case('images', 'Symlink outside base rejected', 'A ![MeaningfulAlt](escape.svg) Z', 'Symlink to an SVG outside the input directory is blocked consistently with traversal protection.', 'renderer.rs:2438-2506', [('image_count', 0), ('contains', 'MeaningfulAlt')])
case('images', 'Unsupported local GIF', 'A ![MeaningfulAlt](pixel.gif) Z', 'Unsupported extension is reported and preserves alt fallback.', 'renderer.rs:2321-2324,2390-2409', [('contains', 'MeaningfulAlt'), ('warning',)], True)
case('images', 'Stdin local image without base', 'A ![MeaningfulAlt](pixel.png) Z', 'No-base local image is rejected visibly with alt fallback.', 'renderer.rs:2509-2510', [('contains', 'MeaningfulAlt'), ('warning',)], True, stdin=True)
case('images', 'Empty image source', 'A ![MeaningfulAlt]() Z', 'Empty destination does not drop meaningful alt text.', 'renderer.rs:2191-2194', [('contains', 'MeaningfulAlt')])
case('images', 'Base64 PNG data URL', '![AltText](data:image/png;base64,{PNG})', 'Base64 image data URL renders without network.', 'renderer.rs:2251-2263,2339-2368', [('image_count', 1)])
case('images', 'Base64 SVG data URL', '![AltText](data:image/svg+xml;base64,{SVG})', 'Base64 SVG data URL reads dimensions and renders.', 'renderer.rs:2251-2263,2371-2380', [('image_count', 1)])
case('images', 'Malformed base64 fallback', 'A ![MeaningfulAlt](data:image/png;base64,INVALID!) Z', 'Malformed base64 warns Failed to decode data URL and shows alt.', 'renderer.rs:2363-2366', [('contains', 'MeaningfulAlt'), ('stderr', 'Failed to decode data URL')])
case('images', 'Non-base64 data URL unsupported', 'A ![MeaningfulAlt](data:image/svg+xml,%3Csvg%3E) Z', 'Unsupported non-base64 data URL preserves alt with diagnostic.', 'renderer.rs:2355-2360', [('contains', 'MeaningfulAlt'), ('warning',)], True)
case('images', 'Empty data URL payload', 'A ![MeaningfulAlt](data:image/png;base64,) Z', 'Empty data URL preserves alt with diagnostic.', 'renderer.rs:2345-2347', [('contains', 'MeaningfulAlt'), ('warning',)], True)
case('images', 'Formatted image alt fallback', 'A ![*Emphasis* `CodeAlt` text](missing.png) Z', 'Fallback keeps all alt content, including inline code, without leaking it into surrounding flow.', 'renderer.rs:542-549,821-825,2197-2211', [('contains', 'Emphasis CodeAlt text')])
case('images', 'Remote PNG localhost', '![AltText]({SERVER}/image.png)', 'Loopback HTTP PNG is fetched and embedded.', 'renderer.rs:2266-2309', [('image_count', 1)])
case('images', 'Remote SVG query string', '![AltText]({SERVER}/shape.svg?cache=1)', 'HTTP SVG with query has correct MIME and dimensions.', 'renderer.rs:2266-2309,2411-2423', [('image_count', 1)])
case('images', 'Remote missing Content-Type extension fallback', '![AltText]({SERVER}/no-type.png?cache=1)', 'URL extension supplies MIME when the response omits Content-Type.', 'renderer.rs:2278-2284,2411-2423', [('image_count', 1)])
case('images', 'Remote unknown MIME rejected visibly', 'A ![MeaningfulAlt]({SERVER}/unknown) Z', 'Unidentified remote image preserves alt and emits a diagnostic.', 'renderer.rs:2278-2287', [('contains', 'MeaningfulAlt'), ('warning',)], True)
case('images', 'Remote HTTP 404 fallback', 'A ![MeaningfulAlt]({SERVER}/missing) Z', 'HTTP error warns Failed to fetch image and shows alt.', 'renderer.rs:2274-2277,2197-2211', [('contains', 'MeaningfulAlt'), ('stderr', 'Failed to fetch image')])
case('images', 'Remote invalid content fallback', 'A ![MeaningfulAlt]({SERVER}/corrupt.png) Z', 'Corrupt remote image warns dimension failure and shows alt.', 'renderer.rs:2302-2309', [('contains', 'MeaningfulAlt'), ('stderr', 'Failed to read image size')])
case('images', 'Remote redirect', '![AltText]({SERVER}/redirect)', 'HTTP redirect to a valid image is followed and embedded.', 'renderer.rs:2266-2277', [('image_count', 1)])
case('images', 'Remote 10 MiB limit', 'A ![MeaningfulAlt]({SERVER}/oversize.png) Z', 'Image above 10 MiB warns size limit and preserves alt without unbounded read.', 'renderer.rs:2289-2300', [('contains', 'MeaningfulAlt'), ('stderr', 'exceeds the 10 MiB size limit')], variants=['light-default'])
case('images', 'Remote timeout bound', 'A ![MeaningfulAlt]({SERVER}/slow.png) Z', 'Stalled loopback response times out near 10 seconds and preserves alt.', 'renderer.rs:2269-2273', [('contains', 'MeaningfulAlt'), ('stderr', 'Failed to fetch image')], variants=['light-default'])
case('width', 'Width below horizontal padding', 'Alpha\n\n| Header |\n|---|\n| Beta |', 'A width smaller than padding is rejected with an actionable minimum-width diagnostic.', 'renderer.rs:392-453,2698-2700', [('reject',)], width=40, variants=['light-default'])
case('width', 'Zero width public renderer', 'Alpha', 'Zero width produces an actionable rejection, never an unusable zero-width SVG.', 'renderer.rs:392-453,2765-2780', [('reject',)], width=0, variants=['light-default'])
case('width', 'Negative width public renderer', 'Alpha', 'Negative width is rejected with a clear error.', 'renderer.rs:392-453,2765-2780', [('reject',)], width=-1, variants=['light-default'])
case('document', 'Reusable renderer isolation', 'Alpha', 'A second render call returns only the second document with fresh layout and styles.', 'renderer.rs:455-602', [('public_reuse',)], api=True, variants=['light-default'])


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def source_for(c):
    # Resolve current line numbers from owning symbols instead of retaining
    # hand-counted ranges that become inaccurate when neighboring code changes.
    owners = {
        'document': ['render', 'finalize_svg'],
        'headings': ['handle_start_tag', 'handle_end_tag', 'current_font_size'],
        'text': ['render', 'render_text', 'render_text_content_token', 'render_whitespace_token', 'draw_text_at'],
        'inline': ['handle_start_tag', 'handle_end_tag', 'render_text_content_token'],
        'code': ['render_inline_code', 'render_code_block_with_language', 'wrap_styled_line'],
        'lists': ['handle_start_tag', 'start_list_item', 'render_text_content_token', 'end_list_item', 'next_list_marker'],
        'tasks': ['render_task_marker', 'start_list_item'],
        'quotes': ['handle_start_tag', 'start_blockquote', 'end_blockquote'],
        'rules': ['render_horizontal_rule'],
        'links': ['render', 'handle_start_tag', 'current_fill', 'render_text_content_token'],
        'tables': ['render', 'finish_table', 'wrap_table_text', 'finish_image'],
        'footnotes': ['handle_start_tag', 'handle_end_tag', 'render_footnote_reference', 'render_text_content_token'],
        'definitions': ['handle_start_tag', 'handle_end_tag', 'line_start_x'],
        'metadata': ['render', 'handle_start_tag'],
        'html': ['parse_inline_html_attrs', 'parse_font_size_scale', 'sanitize_color', 'render_inline_html', 'effective_inline_style'],
        'images': ['finish_image', 'load_image_payload', 'parse_data_url', 'resolve_image_path', 'image_dimensions'],
        'width': ['new_with_base_path', 'right_edge', 'finalize_svg'],
    }
    lines = (ROOT / 'src/renderer.rs').read_text().splitlines()
    refs = []
    for symbol in owners[c['area']]:
        line = next(i for i, text in enumerate(lines, 1) if re.search(r'\bfn ' + symbol + r'\b', text))
        refs.append(f'src/renderer.rs:{line} ({symbol})')
    return '; '.join(refs)


def inventory():
    existing = {r['id']: r for r in json.loads(REPORT.read_text())} if REPORT.exists() else {}
    rows = []
    for c in CASES:
        row = {k: '' for k in ['test_command', 'evidence', 'errors', 'fix', 'retest', 'showcase']}
        row.update(existing.get(c['id'], {}))
        row.update(id=c['id'], area=c['area'], feature=c['feature'],
                   user_story=f"As a document author, I want {c['feature'].lower()} to preserve my document's meaning and remain readable.",
                   expected_behavior=c['expected'], source=source_for(c),
                   owner='markdown', status=row.get('status', 'inventoried'), updated_at=now())
        rows.append(row)
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(rows, indent=2) + '\n')
    print(f'Inventoried {len(rows)} stories: {dict(collections.Counter(r["area"] for r in rows))}')


def publish(rows, updates=None):
    REPORT.write_text(json.dumps(rows, indent=2) + '\n')
    importer = ROOT / 'scripts/feature-audit.py'
    if importer.exists():
        update_path = REPORT.with_name('markdown-update.json')
        update_path.write_text(json.dumps(updates if updates is not None else [{k: v for k, v in r.items() if k != 'retest'} for r in rows], indent=2))
        result = subprocess.run(['python3', str(importer), 'import', str(update_path)], cwd=ROOT, capture_output=True, text=True)
        if result.returncode:
            print('CSV import failed:', result.stderr, flush=True)


def nodes(tree, name):
    return list(tree.iter(NS + name))


@functools.lru_cache(maxsize=None)
def measured_width(text, size, code, bold, italic):
    command = [str(ROOT / '.amp/in/audit/markdown-public-probe'), '--measure', text, size, str(code).lower(), str(bold).lower(), str(italic).lower()]
    return float(subprocess.check_output(command, env=ENV, cwd=ROOT, text=True))


def check(spec, tree, raw, stderr, result, width, variant):
    texts = nodes(tree, 'text') if tree is not None else []
    payload = ''.join(''.join(t.itertext()).replace('\u00a0', ' ') for t in texts)
    def node(s):
        found = [t for t in texts if ''.join(t.itertext()).replace('\u00a0', ' ') == s]
        assert found, f'No exact text node {s!r}; text={payload!r}'
        return found[0]
    def num(s, attr):
        return float(node(s).get(attr, '0'))
    kind, *args = spec
    if kind == 'contains': assert args[0] in payload, f'Missing text {args[0]!r}; text={payload!r}'
    elif kind == 'payload_count': assert payload.count(args[0]) == args[1], f'Lost or duplicated payload: {args!r}; {payload!r}'
    elif kind == 'absent': assert args[0] not in payload, f'Unexpected text {args[0]!r}'
    elif kind == 'raw_absent': assert args[0] not in raw, f'Unsafe raw content {args[0]!r}'
    elif kind == 'below': assert num(args[0], 'y') > num(args[1], 'y'), f'{args[0]} not below {args[1]}'
    elif kind == 'same_y': assert abs(num(args[0], 'y') - num(args[1], 'y')) < 0.1, f'Nonbreaking pair was split across baselines: {num(args[0], "y")} and {num(args[1], "y")}'
    elif kind == 'nonbreak':
        pair = [t for t in texts if t.text and 'A\u00a0B' in t.text]
        assert pair or abs(num('A', 'y') - num('B', 'y')) < 0.1, 'Nonbreaking pair was split across baselines'
    elif kind == 'same_x': assert abs(num(args[0], 'x') - num(args[1], 'x')) < 0.1, f'Indent differs for {args}'
    elif kind == 'x_gt': assert num(args[0], 'x') > num(args[1], 'x'), f'Indent not greater for {args}'
    elif kind == 'same_size': assert num(args[0], 'font-size') == num(args[1], 'font-size'), f'Sizes differ for {args}'
    elif kind == 'size_gt': assert num(args[0], 'font-size') > num(args[1], 'font-size'), f'Sizes not ordered for {args}'
    elif kind == 'scale': assert abs(num(args[0], 'font-size') / num(args[1], 'font-size') - args[2]) < 0.01, f'{args[0]} scale={num(args[0], "font-size") / num(args[1], "font-size")}, expected={args[2]}'
    elif kind == 'attr': assert node(args[0]).get(args[1]) == args[2], f'{args[0]} {args[1]}={node(args[0]).get(args[1])!r}, expected={args[2]!r}'
    elif kind == 'noattr': assert node(args[0]).get(args[1]) is None, f'{args[0]} unexpectedly has {args[1]}'
    elif kind == 'fill_not': assert node(args[0]).get('fill') != args[1], f'{args[0]} retained color {args[1]}'
    elif kind == 'fill_exists': assert any(t.get('fill') == args[0] for t in nodes(tree, 'rect')), f'Missing background {args[0]}'
    elif kind == 'no_element': assert not nodes(tree, args[0]), f'Unexpected <{args[0]}>'
    elif kind == 'text_exact': assert any(''.join(t.itertext()) == args[0] for t in texts), f'No preserved nonbreaking text {args[0]!r}; nodes={[t.text for t in texts]}'
    elif kind in ('text_count', 'marker_count'): assert sum(''.join(t.itertext()) == args[0] for t in texts) == args[1], f'Expected {args[1]} occurrences of {args[0]!r}; actual={sum(t.text == args[0] for t in texts)}'
    elif kind == 'line_count': assert len(nodes(tree, 'line')) == args[0], f'Line count={len(nodes(tree, "line"))}, expected={args[0]}'
    elif kind == 'rect_min': assert len(nodes(tree, 'rect')) >= args[0], 'Missing code background'
    elif kind == 'code_count': assert len([t for t in texts if t.get('font-family') == 'monospace']) >= args[0], 'Missing monospace code'
    elif kind == 'table_plain': assert all(t.get('font-family') != 'monospace' for t in texts), 'Unexpected monospace in flattened table'
    elif kind == 'highlight': assert len({t.get('fill') for t in texts}) >= 3, 'Syntax colors are not distinct'
    elif kind == 'code_payload': assert ''.join((t.text or '').replace('\u00a0', ' ') for t in texts if t.get('font-family') == 'monospace') == args[0], 'Code characters lost during wrapping'
    elif kind == 'cluster_runs':
        cluster, identifying_char = args
        runs = [t.text or '' for t in texts if identifying_char in (t.text or '')]
        assert runs, f'Missing grapheme payload {cluster!r}'
        assert all(run == cluster * (len(run) // len(cluster)) for run in runs), f'Detached visible grapheme: {runs!r}'
    elif kind == 'code_blank_gap': assert num('Beta', 'y') - num('Alpha', 'y') >= 35, f'Blank code line collapsed: gap={num("Beta", "y") - num("Alpha", "y")}'
    elif kind == 'wrap':
        if width <= 360: assert len({t.get('y') for t in texts}) >= 3, 'Expected multiple wrapped baselines'
    elif kind == 'bounds':
        for t in texts:
            advance = measured_width(t.text or '', t.get('font-size'), t.get('font-family') == 'monospace', t.get('font-weight') == '700', t.get('font-style') == 'italic')
            assert float(t.get('x')) + advance <= width - 31, f'Text overflow: x={t.get("x")}, measured advance={advance:.2f}, right padding edge={width-32}, text={(t.text or "")[:90]!r}'
    elif kind == 'spacing': assert 40 < num('Beta', 'x') - num('Alpha', 'x') < 70, 'Whitespace gap collapsed or excessive'
    elif kind in ('sup', 'sub'):
        assert num(args[0], 'font-size') < num(args[1], 'font-size'), 'Script not smaller'
        assert (num(args[0], 'y') < num(args[1], 'y')) == (kind == 'sup'), 'Wrong script baseline direction'
    elif kind == 'heading':
        scale = [2.2, 1.8, 1.5, 1.25, 1.1, 1.0][args[0]-1]
        assert abs(num('UniqueHeading', 'font-size') / num('Body', 'font-size') - scale) < 0.01
        assert node('UniqueHeading').get('font-weight') == '700'
        assert node('Body').get('font-weight') is None
    elif kind == 'checkbox':
        assert len([r for r in nodes(tree, 'rect') if r.get('ry') == '2']) == 1, 'Missing checkbox'
        assert bool(nodes(tree, 'polyline')) == args[0], 'Wrong checkmark state'
    elif kind == 'check_count':
        assert len([r for r in nodes(tree, 'rect') if r.get('ry') == '2']) == args[0]
        assert len(nodes(tree, 'polyline')) == args[1]
    elif kind == 'quote_border': assert len([n for n in nodes(tree, 'line') if n.get('stroke-width') == '3']) == args[0], 'Missing quote border'
    elif kind == 'quote_clearance':
        border_end = max(float(n.get('y2')) for n in nodes(tree, 'line') if n.get('stroke-width') == '3')
        assert num('Omega', 'y') - num('Omega', 'font-size') * 0.8 >= border_end, 'Following prose overlaps quote border'
    elif kind == 'rule': assert any(t.get('stroke-width') == '1.5' for t in nodes(tree, 'line')), 'Missing thematic rule'
    elif kind == 'mark': assert any(r.get('fill') == ('#4a4600' if variant.startswith('dark') else '#ffff00') for r in nodes(tree, 'rect')), 'Wrong highlight color'
    elif kind == 'table_x_count': assert payload.count('x') == args[0], f'Lost hard-split cell characters: {payload.count("x")}'
    elif kind == 'table_align':
        assert num('a', 'x') < num('b', 'x') < num('c', 'x')
        for label, header in [('a', 'LongLeft'), ('b', 'LongCenter'), ('c', 'LongRight')]:
            assert num(label, 'x') >= num(header, 'x'), f'{label} alignment falls left of header'
    elif kind == 'cell_bounds':
        cells = [r for r in nodes(tree, 'rect') if r.get('stroke') and float(r.get('width', 0)) < width / 2]
        for t in texts:
            x, y, size = float(t.get('x')), float(t.get('y')), float(t.get('font-size'))
            advance = measured_width(t.text or '', t.get('font-size'), False, t.get('font-weight') == '700', False)
            assert any(float(r.get('x')) <= x and x + advance <= float(r.get('x')) + float(r.get('width')) + 0.1 and float(r.get('y')) <= y <= float(r.get('y')) + float(r.get('height')) for r in cells), f'Text outside cell: {t.attrib}, measured advance={advance:.2f}, text={t.text}'
    elif kind == 'image_count': assert len(nodes(tree, 'image')) == args[0], f'Image count={len(nodes(tree, "image"))}, expected={args[0]}'
    elif kind == 'image_fit':
        image = nodes(tree, 'image')[0]
        w, h = float(image.get('width')), float(image.get('height'))
        assert w <= width - 64 + 0.1 and abs(w / h - args[0]) < 0.02, f'Incorrect image fit {w}×{h}'
    elif kind == 'image_size':
        image = nodes(tree, 'image')[0]
        assert (float(image.get('width')), float(image.get('height'))) == tuple(args)
    elif kind == 'stderr': assert args[0] in stderr, f'Missing diagnostic {args[0]!r}; stderr={stderr!r}'
    elif kind == 'warning': assert 'Warning:' in stderr, f'Unsupported input silently discarded; stderr={stderr!r}'
    elif kind == 'reject': assert result.returncode != 0 and stderr, f'Invalid width accepted: exit={result.returncode}, stderr={stderr!r}'
    elif kind == 'positive_geometry':
        for n in tree.iter():
            for attr in ['width', 'height']:
                value = n.get(attr)
                if value and '%' not in value: assert float(value) >= 0, f'Negative {attr}: {n.attrib}'
        for t in texts: assert 0 <= float(t.get('x')) < width, f'Text begins outside canvas: {t.attrib}'
    elif kind == 'public_reuse': assert 'Alpha' not in payload and 'Beta' in payload, f'Renderer retains prior document: {payload!r}'
    else: raise ValueError(f'Unknown check {spec}')


PNG = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=')
SVG = b'<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect width="80" height="40" fill="#008080"/><circle cx="20" cy="20" r="12" fill="#ffc000"/></svg>'


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        with (OUT / 'http.log').open('a') as f:
            f.write(fmt % args + '\n')

    def do_GET(self):
        route = self.path.split('?')[0]
        if route == '/slow.png':
            time.sleep(11)
        if route == '/redirect':
            self.send_response(302)
            self.send_header('Location', '/image.png')
            self.end_headers()
            return
        if route == '/missing':
            self.send_error(404)
            return
        data = SVG if route == '/shape.svg' else PNG
        if route == '/corrupt.png': data = b'not an image'
        if route == '/oversize.png': data = b'x' * (10 * 1024 * 1024 + 1)
        self.send_response(200)
        if route not in ['/unknown', '/no-type.png']:
            self.send_header('Content-Type', 'image/svg+xml' if route.endswith('.svg') else 'image/png')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        try: self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError): pass


def fixtures():
    INPUTS.mkdir(parents=True, exist_ok=True)
    (INPUTS / 'pixel.png').write_bytes(PNG)
    # The retained 16×8 JPEG fixture is generated once with Pillow. It is a real
    # JPEG, not PNG bytes mislabeled with a .jpg extension.
    assert (HERE / 'pixel.jpg').exists(), 'Missing retained pixel.jpg fixture'
    (INPUTS / 'pixel.jpg').write_bytes((HERE / 'pixel.jpg').read_bytes())
    (INPUTS / 'shape.svg').write_bytes(SVG)
    (INPUTS / 'wide.svg').write_bytes(SVG.replace(b'width="80" height="40"', b'width="2000" height="500"', 1))
    (INPUTS / 'corrupt.png').write_bytes(b'not an image')
    (INPUTS / 'pixel.gif').write_bytes(b'GIF89a')
    (OUT / 'outside.svg').write_bytes(SVG)
    link = INPUTS / 'escape.svg'
    if link.is_symlink(): link.unlink()
    link.symlink_to(os.path.relpath(OUT / 'outside.svg', INPUTS))


def run(ids, phase):
    global OUT, BIN
    if phase != 'baseline':
        OUT = OUT / phase / datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
    OUT.mkdir(parents=True, exist_ok=True)
    command = ['cargo', 'build', '--lib', '--bin', 'markie', '--message-format=json']
    build = subprocess.run(command, env=ENV, cwd=ROOT, capture_output=True, text=True)
    (OUT / 'cargo-build.log').write_text(build.stdout + build.stderr)
    if build.returncode: raise RuntimeError(build.stderr)
    artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith('{')]
    artifacts = [a for a in artifacts if a.get('reason') == 'compiler-artifact' and a['target']['name'] == 'markie']
    BIN = Path(next(a['executable'] for a in artifacts if a.get('executable')))
    library = Path(next(f for a in artifacts if 'lib' in a['target']['kind'] for f in a['filenames'] if f.endswith('.rlib')))
    command = ['rustc', '--edition=2024', str(HERE / 'public-probe.rs'), '--extern', f'markie={library}', '-L', f'dependency={BIN.parent}/deps', '-o', str(ROOT / '.amp/in/audit/markdown-public-probe')]
    build = subprocess.run(command, env=ENV, cwd=ROOT, capture_output=True, text=True)
    (OUT / 'probe-build.log').write_text(build.stdout + build.stderr)
    if build.returncode: raise RuntimeError(build.stderr)
    fixtures()
    rows = json.loads(REPORT.read_text())
    by_id = {r['id']: r for r in rows}
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f'http://127.0.0.1:{server.server_port}'
    try:
        for c in CASES:
            if ids and c['id'] not in ids: continue
            row = by_id[c['id']]
            row['status'] = 'retesting' if phase == 'retest' else ('fixing' if phase == 'targeted' else 'testing')
            row['updated_at'] = now()
            if phase != 'targeted': publish(rows, [{k: v for k, v in row.items() if k != 'retest'}])
            folder = OUT / c['id']
            folder.mkdir(exist_ok=True)
            source = c['markdown'].replace('{PNG}', base64.b64encode(PNG).decode()).replace('{SVG}', base64.b64encode(SVG).decode()).replace('{SERVER}', url)
            path = INPUTS / (c['id'] + '.md')
            path.write_text(source)
            failures, outcomes = [], []
            variants = c.get('variants', ['light-default', 'light-narrow', 'dark-narrow'])
            for variant in variants:
                width = c.get('width', 360 if variant.endswith('narrow') else 1200)
                target = folder / (variant + '.svg')
                command = [str(BIN), '-' if c.get('stdin') else str(path), '--width=' + str(width), '-o', str(target)]
                if variant.startswith('dark'): command += ['--theme', 'nord']
                if c.get('api'):
                    command = [str(ROOT / '.amp/in/audit/markdown-public-probe'), str(target)]
                start = time.monotonic()
                result = subprocess.run(command, input=source if c.get('stdin') else None, capture_output=True, text=True, env=ENV, cwd=ROOT, timeout=18)
                elapsed = time.monotonic() - start
                (folder / (variant + '.stderr')).write_text(result.stderr)
                (folder / (variant + '.stdout')).write_text(result.stdout)
                (folder / (variant + '.command.json')).write_text(json.dumps(dict(command=command, elapsed=elapsed, exit_code=result.returncode), indent=2))
                raw = target.read_text() if target.exists() else ''
                tree = None
                problems = []
                try:
                    if raw: tree = ET.fromstring(raw)
                    if not any(s[0] == 'reject' for s in c['checks']):
                        assert result.returncode == 0, f'CLI exit {result.returncode}: {result.stderr}'
                        assert tree is not None, 'No SVG output'
                        assert float(tree.get('height')) > 0, 'Nonpositive canvas height'
                    for spec in c['checks']:
                        try: check(spec, tree, raw, result.stderr, result, width, variant)
                        except (AssertionError, IndexError) as e: problems.append(str(e) or f'Failed assertion {spec}')
                except (AssertionError, ET.ParseError, ValueError) as e: problems.append(str(e))
                outcome = dict(variant=variant, width=width, seconds=elapsed, exit=result.returncode, failures=problems, text=[''.join(n.itertext()) for n in nodes(tree, 'text')] if tree is not None else [])
                outcomes.append(outcome)
                failures.extend(variant + ': ' + p for p in problems)
            (folder / 'checks.json').write_text(json.dumps(outcomes, indent=2, ensure_ascii=False))
            row['test_command'] = f"python3 tests/audit/markdown/run.py {c['id']}"
            row['evidence'] = str(folder.relative_to(ROOT)) + '/checks.json; SVG + stdout/stderr + exact commands alongside'
            row['status'] = 'failed' if failures else ('unsupported' if c['unsupported'] else 'passed')
            if failures:
                incident = dict(phase=phase, at=now(), severity='high' if c['feature'] == 'Symlink outside base rejected' else 'medium', reproduction=row['test_command'], expected=c['expected'], actual=failures)
                prior = json.loads(row['errors']) if row['errors'] else []
                if incident['actual'] not in [i['actual'] for i in prior]: prior.append(incident)
                row['errors'] = json.dumps(prior, ensure_ascii=False)
            row['updated_at'] = now()
            if phase == 'retest':
                row['retest'] = ('failed: ' + '; '.join(failures)) if failures else ('passed: all ' + str(len(variants)) + ' public CLI/renderer variants meet the story contract; ' + row['evidence'])
            if phase != 'targeted': publish(rows, [dict(row)])
            print(c['id'], row['status'], '; '.join(failures)[:200], flush=True)
    finally:
        server.shutdown()
    print(dict(collections.Counter(r['status'] for r in rows)))
    (OUT / 'results.json').write_text(json.dumps(rows, indent=2) + '\n')
    print('Evidence:', OUT, flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inventory', action='store_true')
    parser.add_argument('--phase', choices=['baseline', 'targeted', 'retest'], default='targeted')
    parser.add_argument('ids', nargs='*')
    args = parser.parse_args()
    if args.inventory: inventory()
    else: run(args.ids, args.phase)
