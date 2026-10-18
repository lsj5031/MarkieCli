"""Source-grounded CLI audit inventory; execution lives in run.py."""
import json
from pathlib import Path
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[3]
CASES = []
THEMES = 'catppuccin_latte catppuccin_mocha dracula everforest github_dark github_light gruvbox_dark gruvbox_light monokai_pro nord solarized_dark solarized_light tokyo_night'.split()


def add(area, feature, expected, kind='render', **kw):
    source = {'main': 'src/main.rs:1-142', 'standalone': 'src/bin/mermaid.rs:1-127',
              'export': 'src/export.rs:1-112', 'themes': 'src/theme.rs:1-358',
              'math': 'src/math.rs:70-1025; src/renderer.rs:1269-1370',
              'fonts': 'src/fonts.rs:1-197; src/export.rs:11-26,45-59',
              'xml': 'src/xml.rs:1-40; src/renderer.rs:436-441',
              'install': 'scripts/install.sh; scripts/download-fonts.sh; .github/workflows/release.yml; Cargo.toml; README.md:137-168'}[area]
    CASES.append(dict(id=f'CLI-{len(CASES)+1:03}', area=area, feature=feature,
                      expected=expected, source=source, kind=kind, **kw))


for binary, area in [('markie', 'main'), ('markie-mermaid', 'standalone')]:
    for flag in ['--help', '-h', '--version', '-V']:
        add(area, f'{binary} {flag}', 'Exit 0; print command help/version to stdout.', 'info', binary=binary, args=[flag], contains='0.5.0' if flag in ['-V', '--version'] else 'Usage:')
    for name, args, message in [
        ('required input', [], 'required'),
        ('unknown option', ['--bogus'], 'unexpected argument'),
        ('extra input', ['input.md', 'extra.md', '-o', 'out.svg'], 'unexpected argument'),
    ]:
        add(area, f'{binary} {name}', 'Exit 2 with usage and actionable argument error.', 'error', binary=binary, args=args, code=2, error=message)
    for name, path in [('missing file', 'missing.md'), ('invalid UTF8 file', 'invalid.md'), ('directory input', 'directory')]:
        add(area, f'{binary} {name}', 'Exit 1 with input read error.', 'error', binary=binary, args=[path, '-o', 'out.svg'], code=1, error='Failed to read input file')
    add(area, f'{binary} stdin', 'Read UTF8 stdin and save valid SVG; no image bytes on stdout.', binary=binary, args=['-', '-o', 'out.svg'])
    add(area, f'{binary} invalid UTF8 stdin', 'Exit 1 with stdin read error.', 'error', binary=binary, args=['-', '-o', 'out.svg'], stdin_hex='ff', code=1, error='Failed to read from stdin')

add('main', 'default file output', 'input.md produces input.png at 2400px width (1200 × 2).', 'default-file')
add('main', 'default stdin output', 'stdin produces output.png at 2400px width.', 'default-stdin')
add('main', 'long output option', '--output chooses the SVG path.', args=['input.md', '--output', 'out.svg'])
for flag, value in [('-w', '397'), ('--width', '641'), ('--width', '397.25')]:
    add('main', f'width {flag} {value}', 'SVG canvas width equals requested width.', args=['input.md', '-o', 'out.svg', flag, value], width=float(value))
for value in ['0', '-1', 'NaN', 'inf', '-inf', '1e39']:
    add('main', f'invalid width {value}', 'Reject nonpositive/nonfinite dimensions cleanly without output.', 'reject', args=['input.md', '-o', 'out.svg', f'--width={value}'])
add('main', 'invalid width syntax', 'Exit 2 identifying invalid numeric value.', 'error', args=['input.md', '-o', 'out.svg', '--width=abc'], code=2, error='invalid value')
add('main', 'narrow width boundary', 'Reject widths 1 and 127.99; accept 128 (2 × padding + 4 × body font size at defaults) with finite SVG geometry.', 'narrow', args=['input.md', '-o', 'out.svg', '--width=1'])
add('main', 'empty markdown', 'Empty Markdown produces a valid finite positive canvas.', stdin='', args=['-', '-o', 'out.svg'])
add('main', 'list themes', 'List exactly 13 source-declared themes, without input.', 'list')
add('main', 'list themes early exit', 'Do not read missing input during theme listing.', 'info', args=['missing.md', '--list-themes'], contains='solarized_light')
for shell in ['bash', 'zsh', 'fish', 'powershell', 'elvish']:
    add('main', f'{shell} completions', 'Generate nonempty completion definitions without input.', 'info', args=['--completions', shell], contains='markie')
add('main', 'invalid completion shell', 'Exit 2 and identify supported shell values.', 'error', args=['--completions', 'tcsh'], code=2, error='invalid value')
add('main', 'completion precedence', 'Completions take precedence over theme listing and input reads.', 'info', args=['missing.md', '--list-themes', '--completions', 'bash'], contains='_markie')

for binary in ['markie', 'markie-mermaid']:
    inp = 'input.mmd' if binary.endswith('mermaid') else 'input.md'
    for ext in ['svg', 'png', 'pdf', 'SVG', 'PNG', 'PDF']:
        add('export', f'{binary} {ext}', 'Export structurally valid artifact with positive dimensions and visible content; success notice on stderr only.', binary=binary, args=[inp, '-o', f'out.{ext}'])
    for ext, error in [('', 'Output file has no extension'), ('.jpg', 'Unsupported output format: .jpg')]:
        add('export', f'{binary} invalid extension {ext or "none"}', 'Exit 1 with actionable output-format error.', 'error', binary=binary, args=[inp, '-o', 'out'+ext], code=1, error=error)
    for ext in ['svg', 'png', 'pdf']:
        add('export', f'{binary} {ext} write failure', 'Exit 1 when output parent is missing.', 'error', binary=binary, args=[inp, '-o', 'missing/out.'+ext], code=1, error='Failed to write '+ext.upper())
    for value in ['0', '-1', 'NaN', 'inf', '-inf', '1e39']:
        add('export', f'{binary} invalid PNG scale {value}', 'Exit 1 identifying invalid --png-scale without writing output.', 'error', binary=binary, args=[inp, '-o', 'out.png', '--png-scale='+value], code=1, error='Invalid --png-scale')
    add('export', f'{binary} fractional scaling', 'PNG dimensions equal ceil(SVG dimensions × 0.75).', 'scale', binary=binary, args=[inp, '-o', 'out.png', '--png-scale=.75'])
    add('export', f'{binary} vector ignores PNG scale', 'SVG export ignores PNG-only scale.', binary=binary, args=[inp, '-o', 'out.svg', '--png-scale=NaN'])
    add('export', f'{binary} dash output boundary', 'Output - is an extensionless filename, not stdout image output.', 'error', binary=binary, args=[inp, '-o', '-'], code=1, error='Output file has no extension')
    add('export', f'{binary} enormous PNG scale', 'Oversized raster requests fail cleanly, without panic or abort.', 'reject', binary=binary, args=[inp, '-o', 'out.png', '--png-scale=1e20'])

add('standalone', 'required output', 'Exit 2 identifying required output.', 'error', binary='markie-mermaid', args=['input.mmd'], code=2, error='required')
for pad in [20, 0, 7.5]:
    add('standalone', f'padding {pad}', 'Padding adds twice its value to both dimensions; default scale is 1.', 'padding', binary='markie-mermaid', pad=pad)
for value in ['-1', 'NaN', 'inf', '-inf', '1e39']:
    add('standalone', f'invalid padding {value}', 'Reject negative/nonfinite padding cleanly without output.', 'reject', binary='markie-mermaid', args=['input.mmd', '-o', 'out.svg', '--padding='+value])
for text, name in [('', 'empty'), ('not_a_diagram\nA --> B', 'unsupported')]:
    add('standalone', name+' diagram', 'Blank diagrams intentionally render an empty canvas (parser.rs:45-48).' if not text else 'Exit 1 with a diagram parse error.', 'render' if not text else 'error', binary='markie-mermaid', args=['-', '-o', 'out.svg'], stdin=text, code=1, error='')
add('standalone', 'built-in theme name boundary', 'Standalone themes accept file paths only; builtin name produces file diagnostic.', 'error', binary='markie-mermaid', args=['input.mmd', '-o', 'out.svg', '-t', 'dracula'], code=1, error='Theme file not found')

for theme in THEMES:
    add('themes', 'builtin '+theme, 'Apply exact source palette background and foreground.', 'theme', args=['input.md', '-o', 'out.svg', '-t', theme], builtin=theme)
add('themes', 'default solarized_light', 'No theme option equals solarized_light.', 'theme', args=['input.md', '-o', 'out.svg'], builtin='solarized_light')
add('themes', 'normalized builtin alias', 'Trim whitespace, ignore case, normalize hyphens.', 'theme', args=['input.md', '-o', 'out.svg', '--theme', '  SoLaRiZeD-LiGhT  '], builtin='solarized_light')
for binary in ['markie', 'markie-mermaid']:
    inp = 'input.mmd' if binary.endswith('mermaid') else 'input.md'
    for filename in ['custom.toml', 'custom.yaml', 'toml-disguised.yaml', 'yaml-disguised.toml']:
        add('themes', binary+' '+filename, 'Detect TOML/YAML by contents and apply required palette.', 'theme', binary=binary, args=[inp, '-o', 'out.svg', '--theme', filename], custom=True)
    for filename in ['bad-theme.toml', 'incomplete.toml', 'missing-theme', 'directory']:
        add('themes', binary+' invalid '+filename, 'Exit 1 with theme parse/read diagnostic.', 'error', binary=binary, args=[inp, '-o', 'out.svg', '-t', filename], code=1, error='')
    add('themes', binary+' invalid palette color', 'Reject invalid colors rather than silently exporting a different palette.', 'reject', binary=binary, args=[inp, '-o', 'out.svg', '-t', 'invalid-color.toml'])
    add('themes', binary+' XML attribute palette', 'Theme strings cannot break SVG XML attributes.', 'palette-injection', binary=binary, args=[inp, '-o', 'out.svg', '-t', 'xml-color.toml'])

MATH = [
    ('identifiers numbers operators', 'x+17', ['x', '+', '17'], 'tokens'),
    ('Greek commands', r'\alpha+\beta', ['α', 'β'], 'tokens'),
    ('Unicode literal preservation', 'α+β+漢', ['α', 'β', '漢'], 'tokens'),
    ('text whitespace', r'\text{hello world}', ['hello world'], 'tokens'),
    ('superscript', 'x^7', ['x', '7'], 'sup'),
    ('subscript', 'x_k', ['x', 'k'], 'sub'),
    ('combined scripts', 'x_k^7', ['x', 'k', '7'], 'subsup'),
    ('fraction', r'\frac{17}{3}', ['17', '3'], 'frac'),
    ('nested fraction', r'\frac{1}{\frac{2}{7}}', ['1', '2', '7'], 'tokens'),
    ('binomial no rule', r'\binom{7}{3}', ['7', '3'], 'binom'),
    ('square root', r'\sqrt{17}', ['17'], 'root'),
    ('indexed root', r'\sqrt[3]{17}', ['3', '17'], 'root'),
    ('sum limits', r'\sum_{k=1}^{7} k', ['∑', 'k', '7'], 'tokens'),
    ('integral limits', r'\int_0^7 x dx', ['∫', '0', '7', 'x'], 'tokens'),
    ('over accent', r'\hat{x}', ['x'], 'tokens'),
    ('under annotation', r'\underbrace{x}_{7}', ['x', '7'], 'tokens'),
    ('quad spacing', r'x\quad y', ['x', 'y'], 'space'),
    ('matrix cells', r'\begin{matrix}1&2\\3&7\end{matrix}', ['1', '2', '3', '7'], 'matrix'),
    ('bracket matrix', r'\begin{bmatrix}1&2\\3&7\end{bmatrix}', ['[', ']', '1', '7'], 'tokens'),
    ('aligned equations', r'\begin{aligned}a&=17\\longname&=3\end{aligned}', ['a', '17', 'longname', '3'], 'aligned'),
    ('cases brace', r'\begin{cases}x&x>0\\7&x<0\end{cases}', ['{', 'x', '7'], 'tokens'),
    ('array cells', r'\begin{array}{lr}1&2\\3&7\end{array}', ['1', '2', '3', '7'], 'matrix'),
    ('nested matrix', r'\begin{matrix}1&\begin{matrix}2&3\\5&7\end{matrix}\\9&4\end{matrix}', ['1', '2', '3', '5', '7', '9', '4'], 'tokens'),
    ('stretchy tall delimiters', r'\left(\frac{17}{3}\right)', ['(', ')', '17', '3'], 'stretch'),
    ('relation entities', 'x<7', ['x', '<', '7'], 'tokens'),
]
for feature, latex, tokens, check in MATH:
    add('math', feature, 'Preserve mathematical content and relative layout; no math failure warning.', 'math', latex=latex, tokens=tokens, check=check)
add('math', 'inline placement', 'Preserve formula, scripts and surrounding text.', 'math', latex='x^7', tokens=['before', 'x', '7', 'after'], check='tokens', inline=True)
for latex in [r'\frac{a}{', r'\definitelyunknown{x}']:
    for inline in [True, False]:
        unbalanced = latex == r'\frac{a}{'
        add('math', ('inline' if inline else 'display')+' invalid '+latex, 'Unbalanced brace input is preserved literally by Markdown math scanning.' if unbalanced else 'Warn with source line, preserve source as code, and finish document.', 'math-literal' if unbalanced else 'math-error', latex=latex, inline=inline)

for feature, text, tokens in [
    ('special XML text', 'A & B < C > D "quoted" \'single\'', ['A', '&', 'quoted']),
    ('illegal XML controls', 'A\x01\x07\x0b\x0c\x1fB\ufffe\uffffC', ['ABC']),
    ('valid Unicode whitespace', 'A\tB\n\nC\r\n\n漢字 😀 café', ['漢字', '😀', 'café']),
    ('entity roundtrip', 'A &amp; B &lt; C &#x3b1;', ['&', '<', 'α']),
    ('controls before heading parse', '\x07# Heading', ['Heading']),
]:
    add('xml', feature, 'Valid XML 1.0; preserve valid text and remove illegal characters.', 'xml', stdin=text, tokens=tokens)
add('fonts', 'system font multilingual styling', 'Render sans, monospace, bold, italic, Greek, CJK and emoji; glyph availability depends on environment.', 'render', args=['-', '-o', 'out.png'], stdin='Normal **Bold** *Italic* `Code` café Ελληνικά 漢字 😀')
add('fonts', 'local fonts and external cwd', 'Exports work outside repository and load fonts/ relative to cwd.', 'font-cwd')
add('fonts', 'missing fonts boundary', 'Font-free environment must report missing fonts or produce visible text, not silently blank output.', 'font-empty')
add('fonts', 'repeatable measurement', 'Same font environment yields byte-identical SVG on repeated CLI runs.', 'repeat')
for mode, feature in [
    ('matrix', 'installer OS architecture matrix'), ('versions', 'version normalization'),
    ('latest', 'latest version lookup'), ('latest-fail', 'latest lookup failure'),
    ('unsupported', 'unsupported platform rejection'), ('missing-bin', 'missing archive binary'),
    ('download-fail', 'installer download failure'), ('nested', 'nested binary archive'),
    ('permissions', 'permissions PATH notice overrides'), ('release', 'standalone release availability'),
    ('cargo', 'Cargo two binary distribution'), ('font-download', 'font download success'),
    ('font-fail', 'font download failure cleanup'), ('font-empty', 'empty font cleanup'),
    ('doc-env', 'documented installer environment override'),
]:
    add('install', feature, 'Verify public distribution contract in network-free sandbox or source metadata; no production installation/release.', 'install', mode=mode)

for area, mode, expected in [
    ('math', 'math-error', 'Malformed LaTeX returns Err with LaTeX parse context through the library.'),
    ('math', 'math-unknown', 'Unknown LaTeX command returns Err rather than successful parser-error text.'),
    ('math', 'math-position', 'Explicit x/baseline, color and finite positive metrics apply through render_math_at.'),
    ('export', 'png-invalid-svg', 'Library PNG conversion returns SVG parse context for malformed SVG.'),
    ('export', 'pdf-invalid-svg', 'Library PDF conversion returns SVG parse context for malformed SVG.'),
    ('export', 'png-invalid-scale', 'Scale validation precedes SVG parsing and rejects invalid scales with context.'),
    ('fonts', 'font-size', 'Doubling font size substantially increases measured width and height.'),
    ('fonts', 'font-wrap', 'Public measurement respects max_width and increases wrapped height.'),
    ('fonts', 'font-cache', 'Two measurement instances return equal results for identical queries.'),
    ('fonts', 'font-styles', 'Code, bold and italic metrics are finite; normal/bold sans and mono PNG glyph widths agree with measured advances in the full host-font environment.'),
    ('xml', 'xml', 'Escape all five XML entities and strip invalid characters while preserving valid Unicode/whitespace.'),
    ('themes', 'theme-errors', 'Library builtin, TOML and YAML failures retain specific parse context.'),
]:
    add(area, 'library '+mode, expected, 'library', mode=mode)
add('standalone', 'invalid padding syntax', 'Exit 2 for nonnumeric padding.', 'error', binary='markie-mermaid', args=['input.mmd', '-o', 'out.svg', '--padding=abc'], code=2, error='invalid value')
add('export', 'invalid PNG scale syntax', 'Exit 2 for nonnumeric scale.', 'error', args=['input.md', '-o', 'out.png', '--png-scale=abc'], code=2, error='invalid value')
add('export', 'huge finite width PNG', 'Reject oversized raster dimensions without crash.', 'reject', args=['input.md', '-o', 'out.png', '--width=1e20'])
add('themes', 'palette injection protection', 'A quoted palette value cannot introduce new SVG attributes.', 'palette-injection', args=['input.md', '-o', 'out.svg', '-t', 'xml-color.toml'])
add('math', 'public math color attribute protection', 'Quoted direct math and constructed Theme colors cannot inject SVG attributes; named and rgb CSS colors remain accepted.', 'library', mode='math-color')
add('math', 'escaped underscore versus paired scripts', 'Escaped underscore stays literal beside a with superscript b; ordinary a_b^c keeps b below a and c above a, without internal marker leakage.', 'library', mode='math-escaped-script')


def inventory():
    stamp = datetime.now(timezone.utc).isoformat()
    return [dict(id=c['id'], area=c['area'], feature=c['feature'],
                 user_story='As a user, I can rely on '+c['feature']+'.',
                 expected_behavior=c['expected'], source=c['source'], owner='cli',
                 status='inventoried', test_command='python3 tests/audit/cli/run.py --case '+c['id'],
                 evidence='', errors='', fix='', retest='', showcase='', updated_at=stamp) for c in CASES]


if __name__ == '__main__':
    path = ROOT / '.amp/in/audit/cli-inventory.json'
    path.parent.mkdir(parents=True, exist_ok=True)
    previous = {row['id']: row for row in json.loads(path.read_text())} if path.exists() else {}
    rows = inventory()
    for row in rows:
        saved = previous.get(row['id'], {})
        for key in ['status', 'evidence', 'errors', 'fix', 'retest', 'showcase', 'updated_at']:
            if key in saved:
                row[key] = saved[key]
    path.write_text(json.dumps(rows, indent=2, ensure_ascii=False)+'\n')
    from collections import Counter
    print(len(CASES), dict(Counter(c['area'] for c in CASES)))
