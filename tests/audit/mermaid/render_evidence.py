#!/usr/bin/env python3
"""Export an existing audit run with the real CLI and make review contact sheets.

Requires Pillow and pdftoppm on the caller's environment. The images are native
PNG exports; contact sheets only resize/label them. Never overwrites a prior run.
"""
from pathlib import Path
import argparse
import json
import struct
import subprocess
import xml.etree.ElementTree as ET
from PIL import Image, ImageDraw, ImageFont

ROOT=Path(__file__).resolve().parents[3]
ART=ROOT/'.amp/in/artifacts/feature-audit/mermaid'
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--run',type=Path,required=True)
parser.add_argument('--group',action='append')
parser.add_argument('--complex',action='store_true')
args=parser.parse_args()
BASE=args.run.resolve()
OUT=ART/('visual-'+BASE.name)
OUT.mkdir()
metadata=subprocess.run(['cargo','metadata','--no-deps','--format-version','1'],cwd=ROOT,text=True,capture_output=True,check=True)
BINARY=Path(json.loads(metadata.stdout)['target_directory'])/'debug/markie-mermaid'
cases={c['id']:c for c in json.loads((ART/'cases.json').read_text())}
groups={
    'directions':['015','017','018','019','020'],
    'shapes-a':['025','027','029','031','033','035'],
    'shapes-b':['037','039','041','043','045','047','049'],
    'edge-markers':['051','054','055','060','063','074'],
    'flow-layout':['087','090','091','092','094','095'],
    'flow-loops':['096','097','098','099','100','101','102'],
    'shaped-loops':['096-diamond-LR','096-circle-LR','096-diamond-TD','096-circle-TD'],
    'sequence-messages':['109','111','112','114','115','116','118'],
    'sequence-self':['118-sequence-self-'+name for name in ['bare-solid','bare-dotted','closed-solid','closed-dotted','async-solid','async-dotted','cross-solid','cross-dotted']],
    'sequence-controls':['119','120','121','122','126','127','129','135'],
    'class':['138','142','145','146','147','150','151','152'],
    'compatibility-flow':['103','103-variant-1','103-variant-2','104','104-variant-1','105','105-variant-1'],
    'compatibility-sequence':['130','130-variant-1','130-variant-2','134','134-variant-1'],
    'compatibility-class':['155','155-variant-1','155-variant-2','156','156-variant-1','156-variant-2','157-variant-1','158','158-variant-1','159','159-variant-1'],
    'state':['161','166','167','170','171','172','173','174'],
    'state-notes':['175','176','177','182'],
    'er':['183','184','185','186','187','191','193','200','201','205','209','215'],
}
try: font=ImageFont.truetype('DejaVuSans.ttf',17)
except OSError: font=ImageFont.load_default()
logs=[]


def run(command):
    p=subprocess.run(command,cwd=ROOT,text=True,capture_output=True)
    logs.append({'command':command,'exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr})
    if p.returncode: raise RuntimeError(p.stderr)


for group,ids in groups.items():
    if args.group and group not in args.group: continue
    images=[]
    for short in ids:
        ident='MER-'+short; src=BASE/(ident+'.mmd'); png=OUT/(ident+'.png')
        if not (BASE/(ident+'.svg')).exists(): continue
        run([str(BINARY),str(src),'-o',str(png),'--png-scale','2'])
        svg=ET.parse(BASE/(ident+'.svg')).getroot()
        with png.open('rb') as stream: header=stream.read(24)
        assert header[:8]==b'\x89PNG\r\n\x1a\n'
        width,height=struct.unpack('>II',header[16:24])
        assert abs(width-2*float(svg.attrib['width']))<=1
        assert abs(height-2*float(svg.attrib['height']))<=1
        image=Image.open(png).convert('RGB')
        images.append((ident,cases.get(ident,{'feature':''}),image))
    if not images: continue
    cellw,cellh=640,460
    sheet=Image.new('RGB',(cellw*2,cellh*((len(images)+1)//2)), '#edf0f3')
    draw=ImageDraw.Draw(sheet)
    for index,(ident,case,image) in enumerate(images):
        x=(index%2)*cellw; y=(index//2)*cellh
        draw.text((x+12,y+10),ident+' '+case['feature'],fill='#171c20',font=font)
        image.thumbnail((cellw-24,cellh-45),Image.Resampling.LANCZOS)
        sheet.paste(image,(x+12,y+40))
    sheet.save(OUT/(group+'.png'))
    print(group,flush=True)

for ident in ['MER-094','MER-109','MER-138','MER-171','MER-184','MER-223','MER-104','MER-134-variant-1','MER-156-variant-1','MER-158-variant-1','MER-159-variant-1']:
    if not (BASE/(ident+'.svg')).exists(): continue
    pdf=OUT/(ident+'.pdf')
    run([str(BINARY),str(BASE/(ident+'.mmd')),'-o',str(pdf)])
    assert pdf.read_bytes()[:5]==b'%PDF-'
    run(['pdftoppm','-png','-singlefile','-scale-to','1200',str(pdf),str(OUT/(ident+'-pdf'))])

for family in (['mermaid','sequence','class','state','er'] if args.complex else []):
    source=ROOT/f'tests/fixtures/complex-{family}.mmd'
    for ext in ['svg','png','pdf']:
        run([str(BINARY),str(source),'-o',str(OUT/f'complex-{family}.{ext}')])
    run(['pdftoppm','-png','-singlefile','-scale-to','2000',str(OUT/f'complex-{family}.pdf'),str(OUT/f'complex-{family}-pdf')])
(OUT/'export-log.json').write_text(json.dumps(logs,indent=2)+'\n')
print('export checks passed',len(logs))
