"""Generate representative baseline/final render evidence with real public CLIs."""
import json
import sys
from pathlib import Path
from datetime import datetime, timezone
sys.dont_write_bytecode = True
from inventory import ROOT, MATH
from run import ART, BIN, invoke, write


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('phase', choices=['baseline', 'fixed'])
    phase = parser.parse_args().phase
    d = ART / ('visual-'+phase) / datetime.now(timezone.utc).strftime('run-%Y%m%dT%H%M%S%f')
    d.mkdir(parents=True, exist_ok=True)
    doc = '# Math and text audit\n\n'
    for title, latex, _, _ in MATH:
        doc += '## '+title+'\n\n$$\n'+latex+'\n$$\n\n'
    (d / 'math.md').write_text(doc)
    logs = []
    for ext in ['svg', 'png', 'pdf']:
        logs.append(invoke([str(BIN / 'markie'), str(d / 'math.md'), '-o', str(d / ('math.'+ext)), '--width=720', '--png-scale=1'], ROOT))
    logs.append(invoke(['pdftoppm', '-scale-to', '2800', '-singlefile', '-png', str(d / 'math.pdf'), str(d / 'math-pdf')], ROOT))
    write(d / 'commands.json', logs)
    assert all(r['returncode'] == 0 for r in logs), logs
    print(d)


if __name__ == '__main__':
    main()
