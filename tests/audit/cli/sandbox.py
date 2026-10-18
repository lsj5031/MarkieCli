"""Network-free install/download interface probes; all writes stay in story sandbox."""
import os
import re
import subprocess
from inventory import ROOT


def installation(c, d, logs, invoke):
    mode = c['mode']
    mock = d / 'mock-bin'
    mock.mkdir(exist_ok=True)
    scripts = {
        'uname': '#!/bin/sh\ncase "$1" in -s) echo "${AUDIT_OS:-Linux}";; -m) echo "${AUDIT_ARCH:-x86_64}";; esac\n',
        'curl': '''#!/bin/sh
echo "$*" >> "$AUDIT_DIR/curl.log"
case "$*" in
  *raw.githubusercontent.com*scripts/install.sh*)
    printf '%s\n' 'printf "%s|%s" "${MARKIE_VERSION-unset}" "${INSTALL_DIR-unset}"'
    exit 0;;
  *api.github.com*)
    if [ "${AUDIT_LATEST_FAIL:-0}" = 1 ]; then echo '{}'; else echo '{"tag_name":"v0.5.0"}'; fi
    exit 0;;
esac
out=''
while [ "$#" -gt 0 ]; do
  if [ "$1" = -o ]; then shift; out="$1"; fi
  shift
done
if [ "${AUDIT_FAIL:-0}" = 1 ]; then echo partial > "$out"; exit 22; fi
if [ "${AUDIT_FONT:-0}" = 1 ]; then
  if [ "${AUDIT_EMPTY:-0}" = 1 ]; then : > "$out"; else echo fixture-font > "$out"; fi
else cp "$AUDIT_DIR/archive.tar.gz" "$out"; fi
''',
    }
    for name, body in scripts.items():
        path = mock / name
        path.write_text(body)
        path.chmod(0o755)
    archive = d / 'archive'
    archive.mkdir(exist_ok=True)
    executable = archive / ('nested' if mode == 'nested' else '') / 'markie'
    executable.parent.mkdir(exist_ok=True)
    if mode != 'missing-bin':
        executable.write_text('#!/bin/sh\necho audit-fixture\n')
        executable.chmod(0o755)
    if mode == 'bundle':
        standalone = executable.with_name('markie-mermaid')
        standalone.write_text('#!/bin/sh\necho standalone-fixture\n')
        standalone.chmod(0o755)
    subprocess.run(['tar', '-czf', str(d / 'archive.tar.gz'), '-C', str(archive), '.'], check=True)
    env = {'PATH': str(mock)+':'+os.environ['PATH'], 'AUDIT_DIR': str(d),
           'INSTALL_DIR': str(d / 'installed'), 'MARKIE_VERSION': '0.5.0', 'HOME': str(d)}

    def script(extra=None, font=False):
        r = invoke(['bash' if font else 'sh', str(ROOT / 'scripts' / ('download-fonts.sh' if font else 'install.sh'))], d, env={**env, **(extra or {})})
        r['environment_overrides'] = {**env, **(extra or {})}
        logs.append(r)
        return r

    if mode == 'matrix':
        for osname in ['Linux', 'Darwin']:
            for arch in ['x86_64', 'amd64', 'aarch64', 'arm64']:
                r = script({'AUDIT_OS': osname, 'AUDIT_ARCH': arch})
                target = ('x86_64' if arch in ['x86_64', 'amd64'] else 'aarch64')+('-unknown-linux-gnu' if osname == 'Linux' else '-apple-darwin')
                assert r['returncode'] == 0 and 'markie-'+target+'.tar.gz' in r['stdout'], r
    elif mode == 'versions':
        for version in ['0.4.0', 'v0.4.0']:
            r = script({'MARKIE_VERSION': version})
            assert r['returncode'] == 0 and '/v0.4.0/' in r['stdout'], r
    elif mode in ['latest', 'latest-fail']:
        r = script({'MARKIE_VERSION': 'latest', 'AUDIT_LATEST_FAIL': '1' if mode.endswith('fail') else '0'})
        if mode.endswith('fail'):
            assert r['returncode'] == 1 and 'Failed to resolve' in r['stderr'], r
        else:
            assert r['returncode'] == 0 and '/v0.5.0/' in r['stdout'], r
    elif mode == 'unsupported':
        for extra, error in [({'AUDIT_OS': 'Windows_NT'}, 'Unsupported OS'), ({'AUDIT_ARCH': 'riscv64'}, 'Unsupported architecture')]:
            r = script(extra)
            assert r['returncode'] == 1 and error in r['stderr'], r
        assert not (d / 'curl.log').exists(), 'unsupported platform attempted download'
    elif mode in ['missing-bin', 'download-fail']:
        r = script({'AUDIT_FAIL': '1' if mode == 'download-fail' else '0'})
        assert r['returncode'] != 0 and not (d / 'installed/markie').exists(), r
        if mode == 'missing-bin':
            assert 'Could not find' in r['stderr'], r
    elif mode in ['nested', 'permissions', 'bundle']:
        r = script()
        assert r['returncode'] == 0, r
        assert (d / 'installed/markie').stat().st_mode & 0o777 == 0o755
        assert 'not in PATH' in r['stdout'], r
        logs.append(invoke([str(d / 'installed/markie')], d))
        assert logs[-1]['stdout'].strip() == 'audit-fixture'
        if mode == 'bundle':
            assert (d / 'installed/markie-mermaid').stat().st_mode & 0o777 == 0o755
            logs.append(invoke([str(d / 'installed/markie-mermaid')], d))
            assert logs[-1]['stdout'].strip() == 'standalone-fixture'
    elif mode == 'release':
        release = (ROOT / '.github/workflows/release.yml').read_text()
        assert all(t in release for t in ['x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu', 'x86_64-apple-darwin', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'])
        assert re.search(r'bin:.*markie-mermaid', release), 'release workflow packages only markie; standalone absent from prebuilt assets'
        assert 'archive: markie-$target' in release, 'multi-bin archive must preserve installer and binstall filename'
        bundle = d / 'bundle'
        bundle.mkdir()
        installation({**c, 'mode': 'bundle'}, bundle, logs, invoke)
    elif mode == 'cargo':
        import tomllib
        manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text())
        assert {b['name'] for b in manifest['bin']} == {'markie', 'markie-mermaid'}
        assert manifest['package']['metadata']['binstall']['bin-dir'] == '{ bin }{ binary-ext }'
        r = invoke(['cargo', 'metadata', '--no-deps', '--offline', '--format-version=1'], ROOT)
        logs.append(r)
        assert r['returncode'] == 0, r
        assert {t['name'] for t in __import__('json').loads(r['stdout'])['packages'][0]['targets'] if t['kind'] == ['bin']} == {'markie', 'markie-mermaid'}
    elif mode in ['font-download', 'font-fail', 'font-empty']:
        r = script({'AUDIT_FONT': '1', 'AUDIT_FAIL': '1' if mode == 'font-fail' else '0', 'AUDIT_EMPTY': '1' if mode == 'font-empty' else '0'}, font=True)
        fonts = list((d / 'fonts').glob('*'))
        if mode == 'font-download':
            assert r['returncode'] == 0 and len(fonts) == 3 and all(p.stat().st_size for p in fonts), r
        else:
            assert r['returncode'] != 0 and not fonts and ('Failed to download' if mode == 'font-fail' else 'Downloaded empty file') in r['stderr'], r
    elif mode == 'doc-env':
        # Execute the actual documentation example with a local curl interceptor.
        example = next(line for line in (ROOT / 'README.md').read_text().splitlines() if 'MARKIE_VERSION=' in line and 'curl ' in line)
        r = invoke(['sh', '-c', example], d, env={**env, 'MARKIE_VERSION': '', 'INSTALL_DIR': ''})
        logs.append(r)
        assert r['stdout'] == '0.4.0|'+str(d / '.local/bin'), 'README override example did not reach installer: '+repr(r['stdout'])
