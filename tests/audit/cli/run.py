#!/usr/bin/env python3
"""Run all CLI baseline stories, or one exact reproduction with --case CLI-NNN."""
import argparse
from collections import Counter
from datetime import datetime, timezone
import json
import hashlib
import math
import os
from pathlib import Path
import re
import resource
import shlex
import struct
import subprocess
import sys
import xml.etree.ElementTree as ET
sys.dont_write_bytecode = True
from inventory import ROOT, CASES, THEMES

ART = ROOT / '.amp/in/artifacts/feature-audit/cli'
INV = ROOT / '.amp/in/audit/cli-inventory.json'
ENV = os.environ.copy()
BIN = Path(os.environ.get('MARKIE_BIN_DIR', ROOT / 'target/debug')).resolve()
NS = '{http://www.w3.org/2000/svg}'


def limits(memory_gib=1):
    resource.setrlimit(resource.RLIMIT_AS, (memory_gib*1024**3, memory_gib*1024**3))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def invoke(argv, cwd, data=None, env=None, memory_gib=1):
    try:
        result = subprocess.run(argv, cwd=cwd, input=data, capture_output=True, timeout=60,
                                env={**ENV, **(env or {})}, preexec_fn=lambda: limits(memory_gib))
    except subprocess.TimeoutExpired as error:
        return dict(command=shlex.join(map(str, argv)), cwd=str(cwd), returncode=None,
                    classification='timeout', timeout_seconds=60,
                    stdout=(error.stdout or b'').decode('utf8', 'replace'),
                    stderr=(error.stderr or b'').decode('utf8', 'replace'))
    return dict(command=shlex.join(map(str, argv)), cwd=str(cwd), returncode=result.returncode,
                classification='crash' if result.returncode < 0 or result.returncode == 101 else 'clean-error' if result.returncode else 'success',
                stdout=result.stdout.decode('utf8', 'replace'), stderr=result.stderr.decode('utf8', 'replace'))


def write(path, obj):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=2)+'\n')


def valid_svg(path):
    root = ET.parse(path).getroot()
    assert root.tag == NS+'svg', 'not an SVG document'
    for key in ['width', 'height']:
        value = float(root.attrib[key])
        assert math.isfinite(value) and value > 0, f'invalid {key}: {value}'
    assert not re.search(r'(?i)(?:="|[ ,(])(?:nan|[+-]?inf)(?:[ ,)\"]|$)', path.read_text()), 'nonfinite SVG geometry'
    return root


def dimensions(path):
    if path.suffix.lower() == '.png':
        raw = path.read_bytes()
        assert raw[:8] == b'\x89PNG\r\n\x1a\n', 'invalid PNG signature'
        return struct.unpack('>II', raw[16:24])
    root = valid_svg(path)
    return float(root.attrib['width']), float(root.attrib['height'])


def visible(path, expected_advance=None):
    # PNG visibility check without third-party Python packages. Decode the
    # 8-bit RGB/RGBA scanlines produced by resvg and Poppler, including filters.
    import zlib
    raw = path.read_bytes()
    compressed = bytearray()
    offset = 8
    while offset < len(raw):
        size = struct.unpack('>I', raw[offset:offset+4])[0]
        tag, payload = raw[offset+4:offset+8], raw[offset+8:offset+8+size]
        if tag == b'IHDR':
            width, height, depth, color, _, _, interlace = struct.unpack('>IIBBBBB', payload)
        if tag == b'IDAT':
            compressed.extend(payload)
        offset += size + 12
    assert depth == 8 and color in [2, 6] and interlace == 0, 'unsupported audit PNG encoding'
    channels = 3 if color == 2 else 4
    stride = width * channels
    decoded = zlib.decompress(compressed)
    previous = bytearray(stride)
    colors = set()
    ink_columns = set()
    for y in range(height):
        start = y * (stride+1)
        filter_type = decoded[start]
        row = bytearray(decoded[start+1:start+1+stride])
        for i in range(stride):
            left = row[i-channels] if i >= channels else 0
            up = previous[i]
            upper_left = previous[i-channels] if i >= channels else 0
            predictor = 0
            if filter_type == 1:
                predictor = left
            elif filter_type == 2:
                predictor = up
            elif filter_type == 3:
                predictor = (left+up)//2
            elif filter_type == 4:
                p = left+up-upper_left
                distances = [abs(p-left), abs(p-up), abs(p-upper_left)]
                predictor = [left, up, upper_left][distances.index(min(distances))]
            else:
                assert filter_type == 0
            row[i] = (row[i]+predictor) % 256
        colors.update(bytes(row[i:i+channels]) for i in range(0, stride, channels))
        if expected_advance is not None:
            assert channels == 4, 'font advance probe requires transparent RGBA'
            ink_columns.update(i//channels for i in range(0, stride, channels) if row[i+3])
        elif len(colors) > 4:
            return
        previous = row
    if expected_advance is not None:
        assert ink_columns, 'missing glyph ink'
        actual = max(ink_columns)-min(ink_columns)+1
        # Ten capital Ms have little side-bearing; allow half one em for
        # rasterization/bearings, not the much narrower condensed face.
        assert abs(actual-expected_advance) < 17.6, (path.name, actual, expected_advance)
        return
    raise AssertionError(f'{path.name}: only {len(colors)} colors; no visible content')


def fixtures(d):
    (d / 'input.md').write_text('# Audit\n\nCLI content **bold** and `code`.\n')
    (d / 'input.mmd').write_text('flowchart LR\n A[Start] --> B[Finish]\n')
    (d / 'invalid.md').write_bytes(b'\xff')
    (d / 'directory').mkdir(exist_ok=True)
    palette = '[colors.primary]\nbackground = "#112233"\nforeground = "#ddeeff"\n[colors.normal]\nblack = "#223344"\nblue = "#445566"\ncyan = "#667788"\nwhite = "#99aabb"\n'
    yaml = 'colors:\n  primary:\n    background: "#112233"\n    foreground: "#ddeeff"\n  normal:\n    black: "#223344"\n    blue: "#445566"\n    cyan: "#667788"\n    white: "#99aabb"\n'
    for name, data in [('custom.toml', palette), ('custom.yaml', yaml), ('toml-disguised.yaml', palette), ('yaml-disguised.toml', yaml), ('bad-theme.toml', '{{['), ('incomplete.toml', '[colors.primary]\nbackground="red"'), ('invalid-color.toml', palette.replace('#112233', 'bogus-color')), ('xml-color.toml', palette.replace('#112233', '#112233\\" injected=\\"yes'))]:
        (d / name).write_text(data)


def public_probe(logs):
    probe = ART / 'public-probe'
    fingerprint = hashlib.sha256((BIN / 'libmarkie.rlib').read_bytes() + (ROOT / 'tests/audit/cli/public_probe.rs').read_bytes()).hexdigest()
    fingerprint_path = ART / 'public-probe-fingerprint.json'
    if not probe.exists() or not fingerprint_path.exists() or json.loads(fingerprint_path.read_text()) != fingerprint:
        built = invoke(['rustc', '--edition=2024', str(ROOT / 'tests/audit/cli/public_probe.rs'),
                        '--extern', 'markie='+str(BIN / 'libmarkie.rlib'),
                        '-L', 'dependency='+str(BIN / 'deps'), '-o', str(probe)], ROOT, memory_gib=4)
        logs.append(built)
        assert built['returncode'] == 0, built
        write(fingerprint_path, fingerprint)
    return probe


def run(c, d, logs):
    fixtures(d)
    binary = c.get('binary', 'markie')
    args = c.get('args', ['-', '-o', 'out.svg'])
    data = c.get('stdin', 'flowchart LR\n A[Start] --> B[Finish]' if binary.endswith('mermaid') else 'Audit content')
    data = bytes.fromhex(c['stdin_hex']) if 'stdin_hex' in c else data.encode()

    def call(arguments, input_data=data, env=None):
        # Store exact stdin alongside command and cwd so every probe is reproducible.
        (d / f'stdin-{len(logs)}.bin').write_bytes(input_data or b'')
        for flag in ['-o', '--output']:
            if flag in arguments:
                output = d / arguments[arguments.index(flag)+1]
                if output.is_file():
                    output.unlink()
        r = invoke([str(BIN / binary), *arguments], d, input_data, env)
        r['stdin_file'] = str(d / f'stdin-{len(logs)}.bin')
        if env:
            r['environment_overrides'] = env
        logs.append(r)
        return r

    kind = c['kind']
    if kind == 'install':
        from sandbox import installation
        return installation(c, d, logs, invoke)
    if kind == 'library':
        result = invoke([str(public_probe(logs)), c['mode']], d)
        logs.append(result)
        assert result['returncode'] == 0, result
        if c['mode'] == 'font-styles':
            probes = [line.split(':') for line in result['stdout'].splitlines() if line.startswith('family-advance:')]
            assert len(probes) == 4, 'missing host-family advance probes'
            for _, name, advance in probes:
                visible(d / (name+'.png'), expected_advance=float(advance))
        if c['mode'] == 'math-escaped-script':
            expressions = {}
            for line in result['stdout'].splitlines():
                expression, fragment = line.split(': ', 1)
                root = ET.fromstring('<svg>'+fragment+'</svg>')
                expressions[expression] = {t.text: float(t.attrib['y']) for t in root.iter('text')}
            ordinary = expressions['a_b^c']
            assert ordinary['b'] > ordinary['a'] > ordinary['c'], 'paired scripts lost their base'
            escaped = expressions[r'\_a^b']
            assert set(escaped) == {'_', 'a', 'b'}, 'escaped input lost or invented text'
            assert escaped['_'] == escaped['a'] > escaped['b'], 'escaped underscore changed script grouping'
        return
    if kind in ['math', 'math-error', 'math-literal']:
        expression = '$'+c['latex']+'$' if c.get('inline') else '$$\n'+c['latex']+'\n$$'
        data = ('before '+expression+' after' if c.get('inline') else expression).encode()
    if kind == 'list':
        r = call(['--list-themes'])
        assert r['returncode'] == 0 and r['stdout'].splitlines() == THEMES, r
        return
    if kind in ['default-file', 'default-stdin']:
        r = call(['input.md'] if kind == 'default-file' else ['-'])
        assert r['returncode'] == 0, r
        assert dimensions(d / ('input.png' if kind == 'default-file' else 'output.png'))[0] == 2400
        assert not r['stdout'] and 'PNG saved to:' in r['stderr'], r
        visible(d / ('input.png' if kind == 'default-file' else 'output.png'))
        return
    if kind == 'padding':
        r = call(['input.mmd', '-o', 'zero.svg', '--padding=0'])
        assert r['returncode'] == 0, r
        base = dimensions(d / 'zero.svg')
        padding_args = [] if c['pad'] == 20 else [f'--padding={c["pad"]}']
        r = call(['input.mmd', '-o', 'out.svg', *padding_args])
        assert r['returncode'] == 0, r
        actual = dimensions(d / 'out.svg')
        assert all(abs(a-b-2*c['pad']) < .01 for a,b in zip(actual, base)), (base, actual)
        r = call(['input.mmd', '-o', 'out.png', *padding_args])
        assert r['returncode'] == 0 and dimensions(d / 'out.png') == tuple(math.ceil(v) for v in actual), r
        return
    if kind == 'scale':
        r = call([*args[:args.index('-o')], '-o', 'reference.svg'])
        assert r['returncode'] == 0, r
        ref = dimensions(d / 'reference.svg')
        r = call(args)
        assert r['returncode'] == 0 and dimensions(d / 'out.png') == tuple(math.ceil(v*.75) for v in ref), r
        return
    if kind == 'repeat':
        assert call(['input.md', '-o', 'one.svg'])['returncode'] == 0
        assert call(['input.md', '-o', 'two.svg'])['returncode'] == 0
        assert (d / 'one.svg').read_bytes() == (d / 'two.svg').read_bytes(), 'nondeterministic SVG'
        return
    if kind == 'narrow':
        for width in ['1', '127.99']:
            result = call(['input.md', '-o', 'out.svg', '--width='+width])
            assert result['returncode'] == 1 and 'width' in result['stderr'].lower() and not (d / 'out.svg').exists(), result
        result = call(['input.md', '-o', 'out.svg', '--width=128'])
        assert result['returncode'] == 0 and dimensions(d / 'out.svg')[0] == 128, result
        return
    if kind in ['font-cwd', 'font-empty']:
        import shutil
        (d / 'fonts').mkdir(exist_ok=True)
        env = None
        if kind in ['font-empty', 'font-cwd']:
            config = d / 'fontconfig.xml'
            (d / 'empty-system-fonts').mkdir(exist_ok=True)
            config.write_text(f'<fontconfig><dir>{d}/empty-system-fonts</dir><cachedir>{d}/font-cache</cachedir></fontconfig>')
            env = {'FONTCONFIG_FILE': str(config), 'FONTCONFIG_PATH': str(d), 'HOME': str(d), 'XDG_DATA_HOME': str(d / 'data')}
        if kind == 'font-cwd':
            # Use real host faces as local fixtures, then hide system inventory.
            # Explicit-family reference exports detect generic-family mismatch.
            for family, key in [('sans-serif', 'AUDIT_SANS'), ('monospace', 'AUDIT_MONO')]:
                match = invoke(['fc-match', '-f', '%{file}\n%{family[0]}\n', family], ROOT)
                logs.append(match)
                assert match['returncode'] == 0 and len(match['stdout'].splitlines()) == 2, match
                filename, name = match['stdout'].splitlines()
                shutil.copy(filename, d / 'fonts' / Path(filename).name)
                env[key] = name
            probe = invoke([str(public_probe(logs)), 'font-export-families'], d, env=env)
            probe['environment_overrides'] = env
            logs.append(probe)
            assert probe['returncode'] == 0, probe
        for ext in ['svg', 'png', 'pdf']:
            r = call(['input.md', '-o', 'out.'+ext], env=env)
            if kind == 'font-empty' and r['returncode'] == 1:
                assert 'No usable fonts' in r['stderr'] and './fonts' in r['stderr'], r
                return
            assert r['returncode'] == 0, r
        logs.append(invoke(['pdftoppm', '-scale-to', '1000', '-singlefile', '-png', str(d / 'out.pdf'), str(d / 'pdf-preview')], d))
        assert logs[-1]['returncode'] == 0
        visible(d / 'out.png')
        visible(d / 'pdf-preview.png')
        return
    r = call(args, data)
    if kind == 'info':
        assert r['returncode'] == 0 and c['contains'] in r['stdout'] and not r['stderr'], r
        return
    if kind == 'error':
        assert r['returncode'] == c['code'] and c['error'].lower() in r['stderr'].lower() and r['stderr'], r
        assert not r['stdout'], r
        return
    if kind == 'reject':
        assert r['returncode'] in [1, 2] and r['classification'] == 'clean-error' and r['stderr'], 'invalid value accepted or crashed: '+str(r)
        assert not (d / 'out.svg').exists() and not (d / 'out.png').exists(), 'invalid value wrote output'
        return
    if kind == 'palette-injection' and r['returncode'] == 1:
        assert 'Invalid Alacritty color' in r['stderr'] and not (d / 'out.svg').exists(), r
        return
    assert r['returncode'] == 0 and not r['stdout'], r
    out = d / 'out.svg'
    for flag in ['-o', '--output']:
        if flag in args:
            out = d / args[args.index(flag)+1]
    ext = out.suffix.lower()
    assert ext[1:].upper()+' saved to:' in r['stderr'], r
    if ext == '.png':
        assert all(v > 0 for v in dimensions(out))
        visible(out)
        return
    if ext == '.pdf':
        assert out.read_bytes().startswith(b'%PDF-'), 'invalid PDF signature'
        info = invoke(['pdfinfo', str(out)], d)
        logs.append(info)
        assert info['returncode'] == 0 and re.search(r'Pages:\s+1', info['stdout']), info
        preview = invoke(['pdftoppm', '-scale-to', '1000', '-singlefile', '-png', str(out), str(d / 'pdf-preview')], d)
        logs.append(preview)
        assert preview['returncode'] == 0, preview
        visible(d / 'pdf-preview.png')
        return
    root = valid_svg(out)
    ts = list(root.iter(NS+'text'))
    content = ''.join(t.text or '' for t in ts)
    if kind == 'palette-injection':
        assert all('injected' not in el.attrib for el in root.iter()), 'palette string introduced injected SVG attribute'
    if 'width' in c:
        assert float(root.attrib['width']) == c['width'], root.attrib
    if kind == 'narrow':
        assert all(0 <= float(t.attrib['x']) < 1 for t in ts), 'text starts outside 1px canvas'
    if kind == 'theme':
        if c.get('custom'):
            # DiagramStyle intentionally mixes 3% foreground into the code background.
            background = '#28394a' if binary.endswith('mermaid') else '#112233'
            assert background in out.read_text() and '#ddeeff' in out.read_text(), 'custom palette not applied'
        else:
            import tomllib
            palette = tomllib.loads((ROOT / 'themes' / (c['builtin']+'.toml')).read_text())
            for key in ['background', 'foreground']:
                assert palette['colors']['primary'][key] in out.read_text(), key+' palette absent'
    if kind == 'xml':
        for token in c['tokens']:
            assert token in content, (token, content)
    if kind == 'math-error':
        assert re.search(r'Warning: math render failed \(line \d+\): LaTeX parse error', r['stderr']), r
        assert c['latex'] in content, 'math error fallback lost source'
    if kind == 'math-literal':
        assert c['latex'] in content and 'math render failed' not in r['stderr'], 'unbalanced delimiter input not preserved as literal Markdown'
    if kind == 'math':
        assert 'math render failed' not in r['stderr'], r
        for token in c['tokens']:
            assert token in content, f'missing {token!r}: {content!r}'
        def find(token):
            return next(t for t in ts if t.text == token)
        check = c['check']
        if check in ['sup', 'sub', 'subsup']:
            base = find('x')
            if check in ['sup', 'subsup']:
                assert float(find('7').attrib['y']) < float(base.attrib['y']), 'superscript not raised'
            if check in ['sub', 'subsup']:
                assert float(find('k').attrib['y']) > float(base.attrib['y']), 'subscript not lowered'
            if check == 'subsup':
                for expression, nested in [('x^7_k', False), ('x_{k^7}', True)]:
                    variant = call(['-', '-o', 'scripts.svg'], ('$$\n'+expression+'\n$$').encode())
                    assert variant['returncode'] == 0 and 'math render failed' not in variant['stderr'], variant
                    nodes = {t.text:t for t in valid_svg(d / 'scripts.svg').iter(NS+'text')}
                    assert (float(nodes['7'].attrib['y']) > float(nodes['x'].attrib['y'])) == nested, 'paired and intentionally nested script semantics conflated'
        if check in ['frac', 'binom']:
            assert float(find('17' if check == 'frac' else '7').attrib['y']) < float(find('3').attrib['y'])
            assert bool(list(root.iter(NS+'line'))) == (check == 'frac'), 'fraction/binomial rule wrong'
        if check == 'root':
            assert list(root.iter(NS+'path')), 'missing radical path'
        if check == 'matrix':
            assert float(find('1').attrib['x']) < float(find('2').attrib['x'])
            assert float(find('1').attrib['y']) < float(find('3').attrib['y'])
        if check == 'aligned':
            equals = [t for t in ts if t.text == '=']
            assert len(equals) == 2 and abs(float(equals[0].attrib['x'])-float(equals[1].attrib['x'])) < .1, 'aligned equals have different x positions'
        if check == 'space':
            assert float(find('y').attrib['x'])-float(find('x').attrib['x']) >= 16, 'quad space missing'
        if check == 'stretch':
            span = float(find('3').attrib['y'])-float(find('17').attrib['y'])
            assert float(find('(').attrib['font-size']) >= span or 'scale' in find('(').attrib.get('transform', ''), 'stretchy delimiter retains ordinary font size beside tall fraction'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--case', action='append')
    parser.add_argument('--new', action='store_true', help='Only run newly inventoried stories')
    parser.add_argument('--retest', action='store_true', help='Record an actual post-fix retest event for selected stories')
    parser.add_argument('--no-import', action='store_true')
    args = parser.parse_args()
    ART.mkdir(parents=True, exist_ok=True)
    rows = json.loads(INV.read_text())
    index = {r['id']:r for r in rows}
    def publish(update):
        write(INV, rows)
        if not args.no_import:
            # Import only this actual event; replaying old retest fields would
            # fabricate fresh verification timestamps for unchanged stories.
            path = ROOT / '.amp/in/audit/cli-update.json'
            write(path, [update])
            result = subprocess.run([sys.executable, 'scripts/feature-audit.py', 'import', str(path)], cwd=ROOT, capture_output=True)
            assert result.returncode == 0, result.stderr.decode()
    for c in CASES:
        if args.case and c['id'] not in args.case:
            continue
        row = index[c['id']]
        if args.new and row['status'] != 'inventoried':
            continue
        row.update(status='retesting' if args.retest else 'testing', updated_at=datetime.now(timezone.utc).isoformat())
        publish(dict(id=c['id'], status=row['status']))
        story = ART / c['id']
        story.mkdir(exist_ok=True)
        d = story / datetime.now(timezone.utc).strftime('run-%Y%m%dT%H%M%S%f') if (story / 'initial-result.json').exists() else story
        d.mkdir(exist_ok=True)
        logs = []
        failure = ''
        try:
            run(c, d, logs)
        except Exception as e:
            failure = type(e).__name__+': '+str(e)
        result = dict(id=c['id'], feature=c['feature'], expected=c['expected'],
                      status='failed' if failure else 'passed', actual=failure or 'All public-interface assertions passed.', commands=logs,
                      limits='60s subprocess timeout; RLIMIT_AS 1 GiB; core dumps disabled',
                      updated_at=datetime.now(timezone.utc).isoformat())
        if not (story / 'initial-result.json').exists():
            write(story / 'initial-result.json', result)
        write(d / 'result.json', result)
        write(story / 'latest-result.json', result)
        evidence = str((story / 'initial-result.json').relative_to(ROOT))+';'+str((d / 'result.json').relative_to(ROOT))
        row.update(status=result['status'], evidence=evidence,
                   updated_at=result['updated_at'])
        update = dict(id=c['id'], status=row['status'], evidence=evidence, expected_behavior=c['expected'])
        if failure:
            row['errors'] = failure
            update['errors'] = failure
        if args.retest:
            row['retest'] = ('failed: '+failure) if failure else 'passed: '+c['feature']+' — all public-interface and semantic assertions passed; '+str((d / 'result.json').relative_to(ROOT))
            update['retest'] = row['retest']
        publish(update)
        print(c['id'], result['status'], c['feature'], failure, flush=True)
    counts = Counter(r['status'] for r in rows)
    write(ART / 'summary.json', dict(counts=counts, failures=[r for r in rows if r['status'] == 'failed']))
    print(dict(counts))


if __name__ == '__main__':
    main()
