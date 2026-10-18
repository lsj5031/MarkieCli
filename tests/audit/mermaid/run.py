#!/usr/bin/env python3
"""Native Mermaid audit. Inventory first; run each public API and CLI independently.

The case expectations are intentional contracts, not snapshots of current output.
No production code or golden assets are rewritten. Failed observations are append-only.
"""
import argparse
from collections import Counter
from datetime import datetime, timezone
import glob
import hashlib
import json
import math
import os
from pathlib import Path
import re
import struct
import subprocess
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[3]
ART = ROOT / '.amp/in/artifacts/feature-audit/mermaid'
INV = ROOT / '.amp/in/audit/mermaid-inventory.json'
PREFIX = ''
FIELDS = 'id area feature user_story expected_behavior source owner status test_command evidence errors fix retest showcase updated_at'.split()
CASES = []


def add(area, feature, source, expected, checks=None, boundary=False, mapping=None):
    ident = f'MER-{len(CASES)+1:03}'
    CASES.append(dict(id=ident, area=area, feature=feature, input=source,
                      expected=expected, checks=checks or {}, boundary=boundary,
                      mapping=mapping or {
                          'flowchart': 'src/mermaid/parser.rs:79-476; src/mermaid/flowchart.rs:10-739; src/mermaid/layout.rs:152-236',
                          'sequence': 'src/mermaid/parser.rs:482-734; src/mermaid/render.rs:180-629; src/mermaid/layout.rs:239-417',
                          'class': 'src/mermaid/parser.rs:740-920; src/mermaid/render.rs:635-1050; src/mermaid/layout.rs:421-547',
                          'state': 'src/mermaid/parser.rs:926-1286; src/mermaid/render.rs:1140-2207; src/mermaid/layout.rs:550-659',
                          'er': 'src/mermaid/parser.rs:1292-1556; src/mermaid/render.rs:2213-2552; src/mermaid/layout.rs:662-727',
                      }.get(area, 'src/mermaid/parser.rs:15-73; src/mermaid/render.rs:127-167; src/bin/mermaid.rs')))


def cases():
    for label, src in [('empty', ''), ('whitespace', ' \n\t'), ('comments', '%% comment\n%% second')]:
        add('general', label, src, 'Succeed with a finite, empty diagram; no phantom nodes.', {'texts': [], 'exact_text_count': 0})
    for family in ['flowchart TD', 'sequenceDiagram', 'classDiagram', 'stateDiagram', 'erDiagram']:
        add('general', f'empty {family}', family, 'Header-only input succeeds without fabricated user nodes. State start sentinel may remain.', {})
    for family in ['pie', 'gantt', 'journey', 'mindmap', 'unknownDiagram']:
        add('general', f'unsupported {family}', family+'\nA --> B', 'Reject with unsupported-family diagnostic naming the type and supported families.', {'error': 'Unsupported Mermaid diagram type'}, True)
    add('general', 'malformed recognized source', 'flowchart LR\n??? invalid syntax ???', 'Remain panic-free; permissive parsing may return an empty diagram, with warning for nonempty body.', {'exact_text_count': 0, 'stderr': 'no visual content'})
    for header in ['flowchart', 'graph']:
        for direction in ['TD', 'TB', 'BT', 'LR', 'RL']:
            add('flowchart', f'{header} {direction} after comments', f"%%{{init: ignored}}\n%% TB\n{header} {direction}\nA[Origin] --> B[Destination]", 'Preserve requested direction, endpoint identities and target arrow; comments do not become nodes.', {'texts': ['Origin', 'Destination'], 'direction': direction, 'edge_count': 1, 'arrows': [1, 0, 0]})
    shapes = [('rectangle','[',' ]'.strip(),'Rect'),('rounded','(',')','RoundedRect'),('stadium','([','])','Stadium'),('subroutine','[[',']]','Subroutine'),('cylinder','[(',')]','Cylinder'),('circle','((','))','Circle'),('double circle','(((',')))','DoubleCircle'),('diamond','{','}','Rhombus'),('hexagon','{{','}}','Hexagon'),('parallelogram','[/','/]','Parallelogram'),('reverse parallelogram','[\\','\\]','ParallelogramAlt'),('trapezoid','[/','\\]','Trapezoid'),('reverse trapezoid','[\\','/]','TrapezoidAlt')]
    for name, opening, closing, ast in shapes:
        for mode in ['standalone', 'inline']:
            src = f'A{opening}Shape{closing}'
            if mode == 'inline':
                src += ' --> B[Target]'
            add('flowchart', f'{name} {mode}', 'flowchart LR\n'+src, f'Render the {name} outline with Shape inside it; preserve the correct shape in the parsed node.', {'texts': ['Shape'], 'shape': ast})
    edges = [('<==>','Thick','Arrow','Arrow'),('<-->','Solid','Arrow','Arrow'),('<.->','Dotted','Arrow','Arrow'),('x==x','Thick','Cross','Cross'),('o==o','Thick','Circle','Circle'),('x--x','Solid','Cross','Cross'),('o--o','Solid','Circle','Circle'),('x-.x','Dotted','Cross','Cross'),('o-.o','Dotted','Circle','Circle'),('<==','Thick','None','Arrow'),('<--','Solid','None','Arrow'),('<-.','Dotted','None','Arrow'),('o-->','Solid','Arrow','Circle'),('--o>','Solid','Arrow','Circle'),('o==>','Thick','Arrow','Circle'),('o-.->','Dotted','Arrow','Circle'),('==o','Thick','Circle','None'),('==x','Thick','Cross','None'),('o==','Thick','None','Circle'),('x==','Thick','None','Cross'),('==>','Thick','Arrow','None'),('--x','Solid','Cross','None'),('x--','Solid','None','Cross'),('--o','Solid','Circle','None'),('o--','Solid','None','Circle'),('-.->','Dotted','Arrow','None'),('-.x','Dotted','Cross','None'),('x-.','Dotted','None','Cross'),('-.o','Dotted','Circle','None'),('o-.','Dotted','None','Circle'),('-.-','Dotted','None','None'),('-->','Solid','Arrow','None'),('---','Solid','None','None'),('->>','Solid','Arrow','Arrow'),('->','Solid','Arrow','None'),('--','Solid','None','None')]
    for op, style, head, tail in edges:
        counts = [int(head=='Arrow')+int(tail=='Arrow'), int(head=='Circle')+int(tail=='Circle'), 2*(int(head=='Cross')+int(tail=='Cross'))]
        add('flowchart', 'edge '+op, f'flowchart LR\nA[Source] {op}|relation| B[Target]', f'Connect Source to Target using {style.lower()} stroke; head={head} at Target, tail={tail} at Source. These operators are the native parser contract (including extensions --o> and ->>), not a promise of full upstream Mermaid compatibility.', {'texts':['Source','Target','relation'], 'edge_count':1, 'arrows':counts, 'style':style, 'head':head, 'tail':tail})
    for tag in ['<br>', '<br/>', '<br />']:
        add('flowchart', 'quoted multiline '+tag, f'flowchart LR\nA["First{tag}Second{tag}Third"] --> B[Finish]', 'Normalize quote delimiters and all break tags before measurement and drawing; three text lines fit inside A.', {'texts':['First','Second','Third','Finish'], 'no_text':['<br','"'], 'multiline':True})
    add('flowchart', 'Unicode and XML text', 'flowchart LR\nA[承認待ち 日本語 中文 & <safe>] --> B[Long asymmetric destination label]', 'Preserve Unicode and XML special characters in valid SVG and keep text within node boundaries.', {'texts':['承認待ち 日本語 中文 & <safe>','Long asymmetric destination label']})
    add('flowchart', 'flat group membership', 'flowchart LR\nsubgraph g["First group"]\nA[Inside A] --> B[Inside B]\nend\nC[Outside]\nB --> C', 'Enclose A and B only; C lies outside. Quoted display title is First group.', {'texts':['First group','Inside A','Inside B','Outside'], 'group':['Inside A','Inside B','Outside']})
    add('flowchart', 'two flat groups', 'flowchart LR\nsubgraph First\nA[One]\nend\nsubgraph Second\nB[Two]\nend\nA --> B', 'Display two separately bounded titled groups, with correct membership and one intergroup edge.', {'texts':['First','Second','One','Two'], 'group_count':2, 'edge_count':1})
    add('flowchart', 'nested groups rejection', 'flowchart LR\nsubgraph Outer\nA[Before]\nsubgraph Inner\nB[Inside]\nend\nend', 'Reject unsupported nesting explicitly, naming Inner and Outer; never silently flatten ownership.', {'error':'Nested subgraphs are not supported'}, True)
    add('flowchart', 'group border edge label', 'flowchart TD\nsubgraph Review\nA[An intentionally long review label that exercises measured node width] --> B[承認待ち 日本語 中文]\nend\nB -->|approved| C[Release]', 'Keep the approved label clear of the Review border so the outgoing edge and group membership remain legible.', {'texts':['approved','Review','Release'], 'group_label_clear':True})
    add('flowchart', 'reverse cycle routing', 'flowchart LR\nA[Queued] --> B[Running]\nB -->|retry| A\nB --> C[Finished]', 'Draw independently distinguishable outgoing and return routes with arrowheads at their correct targets.', {'texts':['retry'], 'edge_count':3, 'distinct_routes':True})
    for direction in ['TD','BT','LR','RL']:
        add('flowchart', 'self loop '+direction, f'flowchart {direction}\nA[Running] -->|heartbeat| A', 'Draw a visible external loop with target arrow aligned to the final curve tangent, no clipping and readable label clearance. Marker tips meet the actual supported node outline.', {'texts':['Running','heartbeat'], 'self_loop':True, 'shaped_loops':direction=='TD'})
    add('flowchart', 'self loop endpoint markers', 'flowchart LR\nA[Running] o--o A', 'Preserve both circle endpoints on a self-loop, as on an ordinary edge.', {'self_markers':2})
    add('flowchart', 'parallel edges', 'flowchart LR\nA[Source] -->|first| B[Target]\nA -->|second| B', 'Keep both relations separately readable rather than superimposing their routes and labels.', {'texts':['first','second'], 'edge_count':2, 'distinct_routes':True})
    add('flowchart', 'later explicit node label', 'flowchart LR\nA --> B\nB[Final label]', 'Use one B node with its explicit Final label, retaining the A→B relation.', {'texts':['Final label'], 'no_text':['B'], 'edge_count':1})
    for feat, src, expected in [
        ('chain edges','A --> B --> C','Preserve both relations and all three endpoints or explicitly diagnose this unsupported chain.'),
        ('fanout shorthand','A & B --> C','Preserve both A→C and B→C or explicitly diagnose the unsupported shorthand.'),
        ('semicolon statements','A[One]; B[Two]; A --> B','Preserve both declarations and relation or explicitly diagnose unsupported separators.'),
        ('style directive','A[One]\nstyle A fill:#ff0000','Styling directives are outside native syntax; never advertise configurable Mermaid init/style support.'),
        ('subgraph direction','subgraph G\ndirection LR\nA[One] --> B[Two]\nend','Local direction is outside the flat-group contract; it must not be mistaken for implemented direction control.'),
        ('asymmetric shape','A>Flag] --> B[Target]','Asymmetric shape is present in complex fixture but absent from native NodeShape; flag as unsupported rather than certify the fallback.')]:
        add('flowchart', feat, 'flowchart TD\n'+src, expected, {}, True)
    pre = 'sequenceDiagram\nparticipant A as Caller\nparticipant B as Server\n'
    add('sequence','participant aliases',pre+'A->>B: request\nB-->>A: reply','Preserve declared participant order and aliases; messages point to the actual receiver.', {'texts':['Caller','Server','request','reply'], 'sequence_arrows':True})
    for op in ['->>','-->>','-->','->','-x','-)','>>+','>>-']:
        marker='no endpoint marker' if op in ['->','-->'] else ('cross endpoint' if op=='-x' else ('closed triangle' if op in ['->>','-->>'] else 'open async arrow'))
        add('sequence','message '+op,pre+f'A{op}B: operation', f'Preserve sender A and receiver B with {marker} and '+('dashed' if op.startswith('--') else 'solid')+' stroke. Upstream Mermaid ->/--> are bare lines; -)/--) are open async arrows. Native >>+/>>- extensions use open asynchronous arrows.', {'texts':['operation'], 'sequence_op':op})
    add('sequence','self message',pre+'A->>A: recurse','Show a visible loop returning left into Caller, with label inside the canvas.', {'texts':['recurse'],'sequence_self':True})
    for pos in ['right of','left of','over']:
        add('sequence','note '+pos,pre+f'Note {pos} A: note content\nA->>B: request','Anchor note at the requested side of Caller, preserve note text, and contain it in the canvas.', {'texts':['note content'],'sequence_note':pos})
    for block in ['alt','opt','loop','par','critical']:
        add('sequence','control '+block,pre+f'{block} condition\nA->>B: primary\nelse alternative\nB-->>A: fallback\nend', 'Enclose primary and else branch in a correctly titled control frame, with separator and correct message directions.', {'texts':[block+' condition','primary','alternative','fallback'],'sequence_arrows':True,'control':True})
    add('sequence','nested control',pre+'alt outer\nloop inner\nA->>B: primary\nend\nelse denied\nB-->>A: fallback\nend','Retain nested frames and branch membership; messages connect the correct lifelines.', {'texts':['alt outer','loop inner','primary','denied','fallback'],'control':True,'sequence_arrows':True})
    add('sequence','activation span',pre+'activate B\nA->>B: work\ndeactivate B','Draw the activation along Server from activate to deactivate; retain message direction.', {'texts':['work'],'activation':True})
    add('sequence','nested activation',pre+'activate B\nactivate B\nA->>B: nested\ndeactivate B\ndeactivate B','Nested activations remain distinguishable and span their matching deactivate events.', {'texts':['nested'],'nested_activation':True})
    for feat, src, expected in [
        ('implicit participants','A->>B: request','Implicit lifeline declarations are not implemented; record missing message rather than passing empty output.'),
        ('actor declaration','actor A\nparticipant B\nA->>B: request','Actor glyph/declaration is outside current participant model; record the unsupported construct.'),
        ('multi participant note',pre+'Note over A,B: shared note','Multi-participant note span is absent from native note lookup; preserve as unsupported contract.'),
        ('parallel and branch',pre+'par first\nA->>B: first work\nand second\nB->>A: second work\nend','Standard par/and branches appear in complex fixture but native parser supports else only; do not certify flattened branches.'),
        ('inline activation',pre+'A->>+B: work\nB-->>-A: done','Inline activation signs are outside the declared activate/deactivate subset; do not certify missing messages.')]:
        add('sequence',feat,src if src.startswith('sequence') else 'sequenceDiagram\n'+src,expected,{},True)
    add('sequence','long note bounds',pre+'Note right of B: This is a deliberately very long note that must stay readable inside the exported diagram canvas', 'Preserve note text within the note and exported canvas; wrap or expand rather than clipping.', {'texts':['This is a deliberately very long note that must stay readable inside the exported diagram canvas'],'text_bounds':True})
    add('sequence','unclosed control',pre+'alt condition\nA->>B: work','Unclosed controls must remain panic-free; native parser is permissive and may close at EOF.', {'texts':['work']})
    add('class','class declarations','classDiagram\nclass Alpha\nclass Beta','Show distinct class boxes with their names, without inventing relations.', {'texts':['Alpha','Beta']})
    for vis in ['+','-','#','~']:
        add('class','visibility '+vis, f'classDiagram\nclass Account {{\n{vis}id: String\n{vis}update(value: String, count: int): bool\n}}', 'Keep visibility, attribute type, parameter names/types and return type in separate class compartments.', {'texts':[vis+' id: String',vis+' update(value: String, count: int): bool']})
    for stereo in ['abstract','interface','service']:
        add('class','body stereotype '+stereo,f'classDiagram\nclass Account {{\n<<{stereo}>>\n+id: String\n}}','Render stereotype text; abstract/interface class title is italic, ordinary stereotypes remain upright.', {'texts':['<<'+stereo+'>> Account'],'italic':stereo in ['abstract','interface']})
    add('class','prefix stereotype','classDiagram\nclass <<abstract>> Account','Display native prefix stereotype and honor abstract title styling.', {'texts':['<<abstract>> Account'],'italic':True})
    for op, marker, endpoint, dashed in [('<|--','triangle','from',False),('*--','diamond','from',False),('o--','hollow diamond','from',False),('-->','arrow','to',False),('--','none','none',False),('..>','arrow','to',True),('..|>','triangle','to',True),('..','none','none',True)]:
        add('class','relation '+op,f'classDiagram\nclass Source\nclass Target\nSource {op} Target : relation',f'Connect declared classes with {"dashed" if dashed else "solid"} relation and {marker} at {endpoint}; marker direction must preserve UML meaning.', {'texts':['relation'],'class_relation':[marker,endpoint,dashed]})
    add('class','malformed stereotype','classDiagram\nclass <<abstract Account','Return a class parse error identifying missing >> and source line.', {'error':"missing '>>'"})
    for feat, src, expected in [
        ('implicit classes','Source --> Target','Relationship-only classes are not auto-created by current parser; treat as unsupported subset.'),
        ('multiplicity','class Source\nclass Target\nSource "1" --> "*" Target : contains','Multiplicities exist in AST/render but are never parsed; standard quoted endpoint cardinality must not be reported as supported.'),
        ('standard return type','class Account {\n+update(value: String) bool\n}','Trailing return types occur in complex fixture; bool must not silently disappear.'),
        ('static and abstract members','class Account {\n+count$\n+run()*\n}','Static/abstract member flags have drawing paths but parser never sets them; do not certify literal punctuation as styling.'),
        ('reverse realization','class Source\nclass Target\nSource <|.. Target : implements','Reverse realization in complex fixture is absent from parser operator table; record unsupported contract.')]:
        add('class',feat,'classDiagram\n'+src,expected,{},True)
    add('class','long member fit','classDiagram\nclass Account {\n+extraordinarilyLongAttributeName: Map~String,Value~\n+performLengthyOperation(argument: String): Result~String~\n}','All member text fits within the measured class box.', {'texts':['+ extraordinarilyLongAttributeName: Map~String,Value~'],'text_bounds':True})
    for header in ['stateDiagram','stateDiagram-v2','state']:
        add('state','header '+header,header+'\n[*] --> Idle\nIdle --> Active: begin\nActive --> [*]','Show start, two states and terminal ring; transition arrows point to target.', {'texts':['Idle','Active','begin'],'state_arrows':True})
    for definition in ['state "Friendly label" as Ready','state Ready "Friendly label"']:
        add('state','label '+definition,'stateDiagram\n'+definition,'Show Friendly label on Ready without quote delimiters.', {'texts':['Friendly label']})
    add('state','alias survives transition','stateDiagram\nstate "Friendly label" as Ready\nReady --> Done: finish','Preserve explicit display label when a transition references its ID.', {'texts':['Friendly label','Done','finish']})
    for arrow in ['-->','->']:
        add('state','transition '+arrow, f'stateDiagram\nA {arrow} B: move','Connect A→B with label move and arrow pointing into B.', {'texts':['A','B','move'],'state_arrows':True})
    add('state','reverse pair','stateDiagram\nA --> B: forward\nB --> A: reverse','Keep reverse transitions distinct, with correct target arrows and readable labels.', {'texts':['forward','reverse'],'distinct_routes':True,'state_arrows':True})
    add('state','self transition','stateDiagram\nA --> A: retry','Show an external self-loop; a line through the state body does not represent a visible self transition.', {'texts':['retry'],'state_self':True})
    add('state','composite children','stateDiagram\nstate Parent {\nstate First\nstate Second\nFirst --> Second: advance\n}','Draw First and Second inside Parent and retain internal transition targeting Second.', {'texts':['Parent','First','Second','advance'],'state_group':True,'state_arrows':True})
    add('state','composite local sentinels','stateDiagram\nstate Parent {\n[*] --> First\nFirst --> [*]\n}','Keep start/end inside Parent with both internal transitions; do not route local sentinels to global nodes.', {'texts':['Parent','First'],'local_sentinels':True})
    add('state','nested composite','stateDiagram\nstate Parent {\nstate Child {\nstate Grandchild\nGrandchild --> Done: next\n}\n}','Display Parent→Child→Grandchild containment and nested transition, without stale shallow child copies.', {'texts':['Parent','Child','Grandchild','Done','next'],'nested_state':True})
    add('state','cross boundary transition','stateDiagram\nstate Parent {\nstate Child\n}\nChild --> Outside: exit','Preserve the child-to-external transition or reject it explicitly; silently dropping the relation is a defect.', {'error':'Cross-boundary state transitions are not supported'},True)
    for pos in ['right of','left of','over']:
        add('state','note '+pos,'stateDiagram\nstate Ready\nnote '+pos+' Ready: details','Display details at the requested location relative to Ready.', {'texts':['Ready','details'],'state_note':pos})
    for feat, src, error in [('missing state','state {','Missing state definition'),('missing closing quote','state "Label','Missing closing quote'),('missing ID','state "Label"','Missing state identifier'),('missing alias','state "Label" as ','State parse error:')]:
        add('state',feat,'stateDiagram\n'+src,'Return a contextual state parse error without panic.', {'error':error})
    add('state','long note fit','stateDiagram\nstate Ready\nnote right of Ready: This is a very long state note that should wrap or expand without overflowing the canvas','Keep complete note inside the note box and canvas.', {'text_bounds':True})
    add('er','entities and attributes','erDiagram\nACCOUNT {\n    *string id\n    string email\n}\nLOG','Show ACCOUNT with starred key and email attribute, plus separate LOG entity.', {'texts':['ACCOUNT','*string id','string email','LOG']})
    add('er','auto declared entities','erDiagram\nACCOUNT ||--o{ LOG : writes','Create both endpoint entities and correctly labeled relationship without explicit blocks.', {'texts':['ACCOUNT','LOG','writes'],'er_cards':['||','o{']})
    for left in ['||','|o','}o','}|']:
        for right in ['||','o|','o{','|{']:
            for line in ['--','..']:
                add('er',f'cardinality {left}{line}{right}',f'erDiagram\nACCOUNT {left}{line}{right} LOG : relation', 'Preserve endpoint cardinalities (exactly one / zero or one / zero or more / one or more), place crowfoot prongs nearest the entity and preserve identifying solid versus nonidentifying dashed line. Missing parser combinations are defects in cardinality support.', {'texts':['ACCOUNT','LOG','relation'],'er_cards':[left,right],'er_dashed':line=='..'})
    add('er','quoted relationship label','erDiagram\nACCOUNT ||--o{ LOG : "has many"','Render has many without source quote delimiters; retain endpoints/cardinality.', {'texts':['has many'],'er_cards':['||','o{']})
    add('er','multiple relations','erDiagram\nACCOUNT ||--o{ LOG : writes\nACCOUNT ||--o{ EVENT : emits\nLOG }o--|| EVENT : references','Keep all three correctly connected relationships and readable labels.', {'texts':['writes','emits','references']})
    add('er','long attribute fit','erDiagram\nACCOUNT {\n    string extraordinarily_long_attribute_name_with_no_spaces\n}','Keep the full attribute inside its entity box.', {'text_bounds':True})
    add('er','standard key annotation','erDiagram\nACCOUNT {\n    string id PK\n    string parent_id FK\n}','Standard PK/FK are preserved as attribute text; key semantics beyond native leading * are outside parsed contract.', {'texts':['string id PK','string parent_id FK']},True)
    add('general','Markdown fence diagnostics','pie\nA --> B','Real Markdown CLI must reject unsupported family and identify the fence line.', {'error':'Unsupported Mermaid diagram type','markdown':True})
    add('general','recognized family header prefix','errorDiagram\nACCOUNT','Unknown family names must not accidentally match the er prefix and produce a plausible diagram.', {'error':'Unsupported Mermaid diagram type'})
    add('general','all family exports', 'flowchart LR\nA[Export source] --> B[Export target]', 'Representative standalone SVG, scaled PNG and PDF exports remain parseable and preserve diagram content.', {'texts':['Export source','Export target'],'exports':True})


def inventory():
    records = []
    stamp = datetime.now(timezone.utc).isoformat()
    for case in CASES:
        row = dict.fromkeys(FIELDS, '')
        row.update(id=case['id'], area='Mermaid / '+case['area'], feature=case['feature'],
                   user_story='As a diagram author, I can exercise '+case['feature']+' independently and trust the rendered meaning.',
                   expected_behavior=case['expected'], source=case['mapping']+'; docs/engine-quality.md; '+('Native subset boundary: accepted input is not evidence of supported syntax.' if case['boundary'] else 'Intended native contract; imperfect rendering is a failure, not the expected behavior.'),
                   owner='mermaid', status='inventoried', updated_at=stamp)
        records.append(row)
    INV.write_text(json.dumps(records, indent=2, ensure_ascii=False)+'\n')
    (ART/'cases.json').write_text(json.dumps(CASES, indent=2, ensure_ascii=False)+'\n')
    print(json.dumps(Counter(c['area'] for c in CASES), indent=2), len(CASES))


def run_command(command, log):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    log.write_text('$ '+ ' '.join(map(str,command))+'\n'+result.stdout+result.stderr)
    return result


def tag(element):
    return element.tag.rsplit('}',1)[-1]


def points(element):
    return [tuple(map(float,p.split(','))) for p in element.attrib['points'].split()]


def rect(element):
    return tuple(float(element.attrib.get(k,0)) for k in ['x','y','width','height'])


def contains(box, xy):
    x,y,w,h=box
    return x-0.5 <= xy[0] <= x+w+0.5 and y-0.5 <= xy[1] <= y+h+0.5


def overlap(a,b):
    return a[0] < b[0]+b[2] and a[0]+a[2] > b[0] and a[1] < b[1]+b[3] and a[1]+a[3] > b[1]


def validate(case, root):
    checks=case['checks']; failures=[]; notes=[]
    def need(value, message):
        if not value: failures.append(message)
    elements=list(root.iter()); texts=[e for e in elements if tag(e)=='text']
    labels=[''.join(e.itertext()) for e in texts]
    polygons=[e for e in elements if tag(e)=='polygon']
    lines=[e for e in elements if tag(e)=='line']
    routes=[e for e in elements if tag(e)=='polyline' and 'stroke-linejoin' in e.attrib]
    boxes=[e for e in elements if tag(e)=='rect' and e.attrib.get('stroke-width')=='1' and e.attrib.get('fill')!='none' and e.attrib.get('stroke-dasharray')!='4,2']
    groups=[e for e in elements if tag(e)=='rect' and e.attrib.get('stroke-dasharray')=='4,2']
    textby={label:e for label,e in zip(labels,texts)}
    def xy(label):
        el=textby[label]; return float(el.attrib['x']),float(el.attrib['y'])
    for label in checks.get('texts',[]): need(label in labels,f'Missing exact visible text {label!r}; actual={labels}')
    for label in checks.get('no_text',[]): need(not any(label in x for x in labels),f'Unexpected visible text {label!r}')
    if 'exact_text_count' in checks: need(len(texts)==checks['exact_text_count'],f'Expected {checks["exact_text_count"]} text nodes, got {len(texts)}')
    if 'direction' in checks and all(l in textby for l in ['Origin','Destination']):
        a,b=xy('Origin'),xy('Destination'); d=checks['direction']
        need({'LR':b[0]>a[0],'RL':b[0]<a[0],'TD':b[1]>a[1],'TB':b[1]>a[1],'BT':b[1]<a[1]}[d],f'Wrong {d} order: {a} -> {b}')
    if 'edge_count' in checks: need(len(routes)==checks['edge_count'],f'Expected {checks["edge_count"]} edge routes, got {len(routes)}')
    if 'arrows' in checks:
        circles=[e for e in elements if tag(e)=='circle']; cross=[e for e in lines if e.attrib.get('stroke-width')=='2.5']
        need([len(polygons),len(circles),len(cross)]==checks['arrows'],f'Wrong arrow/circle/cross counts: {[len(polygons),len(circles),len(cross)]} expected {checks["arrows"]}')
        if routes:
            route_points=points(routes[0])
            for p in polygons:
                ps=points(p); tip=ps[0]; base=((ps[1][0]+ps[2][0])/2,(ps[1][1]+ps[2][1])/2)
                at_head=math.dist(tip,route_points[-1])<math.dist(tip,route_points[0])
                ordered=route_points if at_head else list(reversed(route_points))
                endpoint=ordered[-1]; previous=next((q for q in reversed(ordered[:-1]) if math.dist(q,endpoint)>.1),endpoint)
                need((endpoint[0]-previous[0])*(tip[0]-base[0])+(endpoint[1]-previous[1])*(tip[1]-base[1])>0,'Arrowhead opposes last nonzero route segment')
            if checks.get('tail') in ['Circle','Cross']:
                for e in circles+cross:
                    if tag(e)=='circle': center=(float(e.attrib['cx']),float(e.attrib['cy']))
                    else: center=((float(e.attrib['x1'])+float(e.attrib['x2']))/2,(float(e.attrib['y1'])+float(e.attrib['y2']))/2)
                    if math.dist(center,route_points[0])<math.dist(center,route_points[-1]):
                        need(center[0]>route_points[0][0], 'Tail marker lies inside Source and is overpainted by node')
        if routes and polygons and 'Source' in textby:
            pts=points(routes[0]); source,target=xy('Source'),xy('Target')
            for p in polygons:
                ps=points(p); tip=ps[0]; base=((ps[1][0]+ps[2][0])/2,(ps[1][1]+ps[2][1])/2)
                is_head=abs(tip[0]-target[0])<abs(tip[0]-source[0]); expected=1 if is_head else -1
                need((tip[0]-base[0])*expected>0, 'Flowchart arrowhead points away from endpoint')
        pills=[rect(e) for e in elements if tag(e)=='rect' and e.attrib.get('stroke-width')=='0.5']
        for pill in pills:
            for marker in polygons+circles+cross:
                if tag(marker)=='polygon': ps=points(marker)
                elif tag(marker)=='circle':
                    cx,cy,r=[float(marker.attrib[k]) for k in ['cx','cy','r']]; ps=[(cx-r,cy-r),(cx+r,cy+r)]
                else: ps=[(float(marker.attrib['x1']),float(marker.attrib['y1'])),(float(marker.attrib['x2']),float(marker.attrib['y2']))]
                xs,ys=zip(*ps); marker_bounds=(min(xs),min(ys),max(xs)-min(xs),max(ys)-min(ys))
                need(not overlap(pill,marker_bounds),'Edge label obscures endpoint marker')
    if 'style' in checks and routes:
        need(('stroke-dasharray' in routes[0].attrib)==(checks['style']=='Dotted'),'Incorrect edge dash style')
        need(float(routes[0].attrib['stroke-width'])==(1.5 if checks['style']=='Thick' else .75),'Incorrect stroke weight')
    if 'shape' in checks:
        shape=checks['shape']
        if shape in ['Circle','DoubleCircle']: need(len([e for e in elements if tag(e)=='circle'])==(2 if shape=='DoubleCircle' else 1),'Incorrect circle outline count')
        if shape in ['Rhombus','Hexagon','Parallelogram','ParallelogramAlt','Trapezoid','TrapezoidAlt']:
            outlines=[p for p in polygons if 'stroke' in p.attrib]
            need(bool(outlines),'Missing polygon shape outline')
            if outlines:
                pts=points(outlines[0]); need(len(pts)==(6 if shape=='Hexagon' else 4),'Wrong shape vertex count')
                ys=sorted(set(y for x,y in pts)); top=sorted(x for x,y in pts if y==ys[0]); bottom=sorted(x for x,y in pts if y==ys[-1])
                if shape=='Rhombus': need(len(ys)==3 and len(top)==len(bottom)==1,'Diamond must have top/bottom tips, not horizontal edges')
                if shape in ['Parallelogram','ParallelogramAlt']:
                    need(len(top)==len(bottom)==2,'Missing parallel edges')
                    if len(top)==len(bottom)==2: need((top[0]>bottom[0])==(shape=='Parallelogram'),'Wrong parallelogram slant direction')
                if shape in ['Trapezoid','TrapezoidAlt']:
                    need(len(top)==len(bottom)==2,'Missing trapezoid horizontal edges')
                    if len(top)==len(bottom)==2: need(((top[1]-top[0])<(bottom[1]-bottom[0]))==(shape=='Trapezoid'),'Wrong trapezoid width direction')
        if shape=='Cylinder': need(any(tag(e)=='ellipse' for e in elements),'Missing cylinder top ellipse')
        if shape in ['Rect','Subroutine','RoundedRect','Stadium']:
            need(bool(boxes),'Missing rectangular shape')
            if boxes:
                rx=float(boxes[0].attrib.get('rx',0)); height=float(boxes[0].attrib['height'])
                if shape in ['Rect','Subroutine']: need(rx==0,'Square-corner rectangle has rounded outline')
                if shape=='RoundedRect': need(0<rx<height/2,'Rounded rectangle must differ from stadium')
                if shape=='Stadium': need(abs(rx-height/2)<.1,'Stadium caps must span half node height')
                if shape=='Subroutine': need(len([e for e in lines if e.attrib.get('stroke-width')=='1' and e.attrib.get('x1')==e.attrib.get('x2')])==2,'Missing subroutine compartment lines')
    if checks.get('distinct_routes'):
        paths=[points(e) for e in routes]
        if not paths:
            paths=[[(float(e.attrib['x1']),float(e.attrib['y1'])),(float(e.attrib['x2']),float(e.attrib['y2']))] for e in lines if e.attrib.get('stroke-width')=='.75' or e.attrib.get('stroke-width')=='0.75']
            paths += [points(e) for e in elements if tag(e)=='polyline']
        canon=[min(tuple(p),tuple(reversed(p))) for p in paths]
        need(len(canon)==len(set(canon)), 'Coincident same/reverse routes: '+str(paths))
    if 'group_count' in checks: need(len(groups)==checks['group_count'],'Wrong flat group count')
    if 'group' in checks and groups and all(t in textby for t in checks['group']):
        a,b,out=checks['group']; need(contains(rect(groups[0]),xy(a)) and contains(rect(groups[0]),xy(b)) and not contains(rect(groups[0]),xy(out)), 'Incorrect spatial group membership')
    if checks.get('group_label_clear') and groups and 'approved' in textby:
        label=next((e for e in elements if tag(e)=='rect' and e.attrib.get('stroke-width')=='0.5'),None)
        if label is not None:
            g=rect(groups[0]); r=rect(label)
            need(not (r[1]<g[1]+g[3]<r[1]+r[3] and r[0]<g[0]+g[2] and r[0]+r[2]>g[0]),f'Label pill {r} crosses group bottom border {g}')
    if checks.get('self_loop'):
        curves=[e for e in elements if tag(e)=='path' and ' C ' in e.attrib.get('d','')]
        need(bool(curves),'Missing self-loop curve')
        if curves and polygons:
            coords=list(map(float,re.findall(r'-?\d+(?:\.\d+)?',curves[0].attrib['d']))); cp2=coords[4:6]; end=coords[6:8]
            ps=points(polygons[0]); base=((ps[1][0]+ps[2][0])/2,(ps[1][1]+ps[2][1])/2)
            dot=(end[0]-cp2[0])*(ps[0][0]-base[0])+(end[1]-cp2[1])*(ps[0][1]-base[1])
            need(dot>0,f'Self-loop arrow points against final curve tangent: dot={dot}, final={cp2}->{end}, triangle={ps}')
        label_boxes=[e for e in elements if tag(e)=='rect' and e.attrib.get('stroke-width')=='0.5']
        if curves and label_boxes:
            vals=list(map(float,re.findall(r'-?\d+(?:\.\d+)?',curves[0].attrib['d']))); r=rect(label_boxes[0]); hits=0
            for i in range(1,100):
                t=i/100; u=1-t; x=u**3*vals[0]+3*u*u*t*vals[2]+3*u*t*t*vals[4]+t**3*vals[6]; y=u**3*vals[1]+3*u*u*t*vals[3]+3*u*t*t*vals[5]+t**3*vals[7]
                hits+=contains(r,(x,y))
            need(not hits, f'Self-loop label intersects curve at {hits} sampled positions')
    if 'self_markers' in checks: need(len([e for e in elements if tag(e)=='circle'])==checks['self_markers'],'Self-loop drops tail marker')
    if checks.get('multiline') and boxes:
        for label in ['First','Second','Third']:
            if label in textby: need(contains(rect(boxes[0]),xy(label)),f'Multiline {label} outside its node')
    if 'sequence_op' in checks:
        op=checks['sequence_op']
        if checks.get('sequence_self'):
            messages=[e for e in elements if tag(e)=='polyline' and len(points(e))==4]
            route=points(messages[0]) if messages else []
        else:
            messages=[e for e in lines if e.attrib.get('y1')==e.attrib.get('y2') and e.attrib.get('stroke-width')=='0.75']
            route=[(float(messages[0].attrib['x1']),float(messages[0].attrib['y1'])),(float(messages[0].attrib['x2']),float(messages[0].attrib['y2']))] if messages else []
            if route and 'Caller' in textby and 'Server' in textby:
                need(abs(route[0][0]-xy('Caller')[0])<1 and abs(route[-1][0]-xy('Server')[0])<1,'Message endpoints do not connect Caller to Server')
        need(bool(messages),'Missing sequence message route')
        opened=[e for e in elements if tag(e)=='polyline' and len(points(e))==3]
        crosses=[e for e in elements if tag(e)=='path' and e.attrib.get('stroke-width')=='1.5']
        expected=(1,0,0) if op in ['->>','-->>'] else ((0,1,0) if op in ['-)', '--)', '>>+', '>>-'] else ((0,0,1) if op in ['-x','--x'] else (0,0,0)))
        need((len(polygons),len(opened),len(crosses))==expected,f'{op}: wrong filled/open/cross markers; expected={expected}, actual={(len(polygons),len(opened),len(crosses))}')
        if route:
            need(('stroke-dasharray' in messages[0].attrib)==op.startswith('--'),'Sequence message dash mismatch')
            for marker in polygons+opened:
                ps=points(marker); tip=ps[0] if tag(marker)=='polygon' else ps[1]; shoulder=[q for q in ps if q!=tip]; base=tuple(sum(q[i] for q in shoulder)/2 for i in [0,1])
                need(math.dist(tip,route[-1])<.1,'Sequence marker misses receiver endpoint')
                need(sum((route[-1][i]-route[-2][i])*(tip[i]-base[i]) for i in [0,1])>0,'Sequence marker opposes final route segment')
            for cross in crosses:
                coords=list(map(float,re.findall(r'-?\d+(?:\.\d+)?',cross.attrib['d'])))
                need(math.dist((coords[0]+4,coords[1]+4),route[-1])<.1,'Cross misses receiver endpoint')
    if checks.get('sequence_arrows'):
        for p in polygons:
            ps=points(p)
            if len(ps)==3:
                message=next((e for e in lines if abs(float(e.attrib['y2'])-ps[0][1])<.1 and abs(float(e.attrib['x2'])-ps[0][0])<.1),None)
                need(message is not None,'Arrowhead does not terminate a message at its receiver')
                if message is not None:
                    dx=float(message.attrib['x2'])-float(message.attrib['x1'])
                    need(dx*(ps[0][0]-(ps[1][0]+ps[2][0])/2)>0,'Sequence arrowhead opposes sender-to-receiver direction')
    if checks.get('sequence_self'):
        loops=[e for e in elements if tag(e)=='polyline' and len(points(e))==4]
        need(bool(loops),'Missing sequence self-message loop')
    if 'sequence_note' in checks and 'note content' in textby and 'Caller' in textby:
        x,y=xy('note content'); cx,cy=xy('Caller'); pos=checks['sequence_note']
        need((x>cx if pos=='right of' else x<cx),f'Incorrect note side {pos}')
    if checks.get('control'):
        frames=[e for e in elements if tag(e)=='rect' and e.attrib.get('stroke-dasharray')=='5,3']
        need(bool(frames),'Missing control frame')
        for label in ['primary','fallback']:
            if label in textby: need(any(contains(rect(frame),xy(label)) for frame in frames),f'Control message {label} lies outside every frame')
        if 'loop inner' in textby and 'denied' in textby:
            inner=min((rect(frame) for frame in frames if contains(rect(frame),xy('loop inner'))),key=lambda r:r[2])
            need(xy('denied')[1]>inner[1]+inner[3],'Parent alternative label is inside nested loop frame')
    if checks.get('activation'): need(any(float(e.attrib.get('height',0))>16 for e in boxes if e.attrib.get('width')=='8'),'Missing activation span')
    if checks.get('nested_activation'):
        active=[e for e in boxes if e.attrib.get('width')=='8']; need(len(set(e.attrib['x'] for e in active))>1,'Nested activations occupy identical x and hide one another')
    if 'italic' in checks:
        title=next((e for e in texts if 'Account' in ''.join(e.itertext())),None)
        need(title is not None and (title.attrib.get('font-style')=='italic')==checks['italic'],'Incorrect stereotype title italics')
    if 'class_relation' in checks:
        marker,endpoint,dashed=checks['class_relation']; relations=[e for e in lines if e.attrib.get('stroke-width')=='0.75' and e.attrib.get('y1')!=e.attrib.get('y2')]
        need(bool(relations),'Missing class relation line')
        if marker=='none': need(not polygons,'Undirected relation unexpectedly has arrowhead')
        else:
            need(len(polygons)==1,'Missing or extra relation marker')
            if polygons and relations:
                p=points(polygons[0]); line=relations[0]; endxy=(float(line.attrib['x1' if endpoint=='from' else 'x2']),float(line.attrib['y1' if endpoint=='from' else 'y2']))
                need(math.dist(p[0],endxy)<1,f'{marker} on wrong UML endpoint; tip={p[0]}, expected={endxy}')
                need(len(p)==(4 if 'diamond' in marker else 3),'Wrong UML marker shape')
                hollow=marker in ['triangle','hollow diamond']
                need(('stroke' in polygons[0].attrib)==hollow,'Wrong filled versus hollow UML marker')
                need((polygons[0].attrib.get('fill')==line.attrib['stroke'])!=hollow,'UML marker fill reverses inheritance/aggregation/composition meaning')
        if relations: need(('stroke-dasharray' in relations[0].attrib)==dashed,'Wrong UML dash style')
    if checks.get('state_arrows'):
        # Each transition primitive is immediately followed by its arrow polygon.
        for i,e in enumerate(elements):
            if tag(e) not in ['line','polyline'] or i+1>=len(elements) or tag(elements[i+1])!='polygon': continue
            if tag(e)=='line': a=(float(e.attrib['x1']),float(e.attrib['y1'])); b=(float(e.attrib['x2']),float(e.attrib['y2']))
            else: a,b=points(e)[-2:]
            ps=points(elements[i+1]); base=((ps[1][0]+ps[2][0])/2,(ps[1][1]+ps[2][1])/2)
            need((b[0]-a[0])*(ps[0][0]-base[0])+(b[1]-a[1])*(ps[0][1]-base[1])>0, f'State arrow opposes final segment {a}->{b}; triangle={ps}')
        leaf_boxes=[rect(box) for box in boxes if not any(other is not box and contains(rect(box),(rect(other)[0]+rect(other)[2]/2,rect(other)[1]+rect(other)[3]/2)) for other in boxes)]
        for pill in [rect(e) for e in elements if tag(e)=='rect' and e.attrib.get('stroke-width')=='0.5']:
            need(not any(overlap(pill,node) for node in leaf_boxes),'State transition label overlaps leaf node border/body')
            for marker in polygons:
                xs,ys=zip(*points(marker)); need(not overlap(pill,(min(xs),min(ys),max(xs)-min(xs),max(ys)-min(ys))),'State transition label obscures target arrow')
    if checks.get('state_self'):
        need(any(tag(e)=='path' or tag(e)=='polyline' for e in elements),'Self transition is a straight line through state, not external loop')
    if checks.get('state_group') and all(t in textby for t in ['Parent','First','Second']):
        parent=next((rect(e) for e in boxes if contains(rect(e),xy('Parent'))),None)
        need(parent and all(contains(parent,xy(t)) for t in ['First','Second']),'Composite child outside parent')
    if checks.get('local_sentinels'):
        need(len(polygons)==2,'Composite loses local start/end transitions')
        parent=next((rect(e) for e in boxes if 'Parent' in textby and contains(rect(e),xy('Parent'))),None)
        circles=[e for e in elements if tag(e)=='circle']
        need(parent and circles and all(contains(parent,(float(e.attrib['cx']),float(e.attrib['cy']))) for e in circles),'Composite start/end sentinels rendered outside Parent')
    if checks.get('nested_state'): need('Grandchild' in labels,'Nested composite loses grandchildren')
    if 'state_note' in checks and 'details' in textby and 'Ready' in textby:
        a,b=xy('details'),xy('Ready'); pos=checks['state_note']; need((a[0]>b[0] if pos=='right of' else a[0]<b[0]),f'State {pos} note always drawn to the right')
    if 'er_cards' in checks:
        left,right=checks['er_cards']
        relationship=next((e for e in lines if math.hypot(float(e.attrib['x2'])-float(e.attrib['x1']),float(e.attrib['y2'])-float(e.attrib['y1']))>30),None)
        need(relationship is not None,'Missing ER relationship line')
        if relationship is not None:
            ends=[(float(relationship.attrib['x1']),float(relationship.attrib['y1'])),(float(relationship.attrib['x2']),float(relationship.attrib['y2']))]
            for endpoint,other,card in [(ends[0],ends[1],left),(ends[1],ends[0],right)]:
                length=math.dist(endpoint,other); ux=(other[0]-endpoint[0])/length; uy=(other[1]-endpoint[1])/length
                circles=[e for e in elements if tag(e)=='circle' and math.dist(endpoint,(float(e.attrib['cx']),float(e.attrib['cy'])))<length/2]
                bars=[]; forks=[]
                for line in lines:
                    if line is relationship: continue
                    a=(float(line.attrib['x1']),float(line.attrib['y1'])); b=(float(line.attrib['x2']),float(line.attrib['y2']))
                    center=((a[0]+b[0])/2,(a[1]+b[1])/2)
                    if math.dist(center,endpoint)>=length/2: continue
                    along=(b[0]-a[0])*ux+(b[1]-a[1])*uy
                    if abs(along)<.1: bars.append(line)
                    else:
                        forks.append(line)
                        need(along<0,'Crowfoot prongs face away from entity endpoint')
                expected={'||':(2,0,0),'|o':(1,1,0),'o|':(1,1,0),'}o':(0,1,3),'o{':(0,1,3),'}|':(1,0,3),'|{':(1,0,3)}[card]
                need((len(bars),len(circles),len(forks))==expected,f'Wrong {card} endpoint glyph: bars/circles/forks={(len(bars),len(circles),len(forks))}; expected={expected}')
            circles=[e for e in elements if tag(e)=='circle']
            if len(circles)==2:
                a,b=circles; need(math.dist((float(a.attrib['cx']),float(a.attrib['cy'])),(float(b.attrib['cx']),float(b.attrib['cy'])))>float(a.attrib['r'])+float(b.attrib['r'])+1,'Opposite ER cardinality circles overlap')
        if checks.get('er_dashed') is not None:
            if relationship is not None: need(('stroke-dasharray' in relationship.attrib)==checks['er_dashed'],'Nonidentifying ER line lost its dotted style')
    if checks.get('text_bounds'):
        width=float(root.attrib['width'])
        for t in texts:
            x=float(t.attrib['x']); text=''.join(t.itertext()); font=float(t.attrib.get('font-size',13)); estimated=len(text)*font*.5
            end=x+estimated*(.5 if t.attrib.get('text-anchor')=='middle' else 1)
            need(end<=width+1,f'Text extends beyond canvas: {text!r}, approximate right={end:.1f}, width={width}')
    return failures, notes


def test(selected, phase):
    supported=set('activation arrows class_relation control direction distinct_routes edge_count er_cards er_dashed error exact_text_count exports group group_count group_label_clear head italic local_sentinels markdown multiline nested_activation nested_state no_text self_loop self_markers sequence_arrows sequence_note sequence_op sequence_self shape shaped_loops state_arrows state_group state_note state_self stderr style tail text_bounds texts'.split())
    for case in selected:
        unknown=set(case['checks'])-supported
        if unknown: raise ValueError(f'{case["id"]}: unimplemented audit checks {sorted(unknown)}')
    stamp=datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ'); run_dir=ART/stamp; run_dir.mkdir()
    metadata=subprocess.run(['cargo','metadata','--no-deps','--format-version','1'],cwd=ROOT,text=True,capture_output=True,check=True)
    target=Path(json.loads(metadata.stdout)['target_directory'])/'debug'
    built=run_command(['cargo','build','--bins','--lib'],run_dir/'build.log')
    if built.returncode: raise RuntimeError('Build failed: '+built.stderr)
    libraries=sorted(glob.glob(str(target/'deps/libmarkie-*.rlib')),key=os.path.getmtime)
    probe=ART/'probe'
    compiled=run_command(['rustc','--edition=2024','tests/audit/mermaid/probe.rs','--extern','markie='+libraries[-1],'-L','dependency='+str(target/'deps'),'-o',str(probe)],run_dir/'probe-build.log')
    if compiled.returncode: raise RuntimeError(compiled.stderr)
    inv={r['id']:r for r in json.loads(INV.read_text())}; updates=[]; results=[]
    testing=[{'id':c['id'],'status':'testing','test_command':PREFIX+f'python3 tests/audit/mermaid/run.py test --case {c["id"]}'} for c in selected]
    (run_dir/'testing.json').write_text(json.dumps(testing,indent=2)); run_command(['python3','scripts/feature-audit.py','import',str(run_dir/'testing.json')],run_dir/'testing-import.log')
    for case in selected:
        ident=case['id']; stem=run_dir/ident; source=stem.with_suffix('.mmd'); source.write_text(case['input'])
        api=run_command([str(probe),str(source),str(stem)],Path(str(stem)+'.api.log'))
        cli=run_command([str(target/'markie-mermaid'),str(source),'-o',str(stem)+'.svg'],Path(str(stem)+'.cli.log'))
        failures=[]; notes=[]; checks=case['checks']
        if 'error' in checks:
            for name,result in [('API',api),('CLI',cli)]:
                if result.returncode==0 or checks['error'] not in result.stderr: failures.append(f'{name} expected rejection {checks["error"]!r}; exit={result.returncode}; stderr={result.stderr}')
        elif case['boundary'] and api.returncode and cli.returncode and 'not supported' in api.stderr:
            notes.append('Explicit unsupported-construct diagnostic: '+api.stderr.strip())
        elif api.returncode or cli.returncode:
            failures.append(f'Unexpected API/CLI error: {api.returncode}/{cli.returncode}: '+api.stderr+cli.stderr)
        else:
            try:
                root=ET.parse(str(stem)+'.fragment.svg').getroot(); width,height=float(root.attrib['width']),float(root.attrib['height'])
                assert math.isfinite(width) and math.isfinite(height) and width>0 and height>0
                failures,notes=validate(case,root)
                ET.parse(str(stem)+'.svg')
            except Exception as error: failures.append('SVG validation error: '+repr(error))
            if checks.get('stderr') and checks['stderr'] not in api.stderr: failures.append('Missing expected warning '+checks['stderr'])
            if case['boundary'] and not checks: failures.append('Outside-subset syntax was silently accepted instead of diagnosed')
        if checks.get('shaped_loops'):
            for direction in ['LR','TD']:
                for shape,node in [('diamond','A{Running}'),('circle','A((Running))')]:
                    variant=Path(str(stem)+f'-{shape}-{direction}')
                    variant_source=variant.with_suffix('.mmd'); variant_source.write_text(f'flowchart {direction}\n{node} -->|heartbeat| A\n')
                    result=run_command([str(probe),str(variant_source),str(variant)],Path(str(variant)+'.api.log'))
                    rendered=run_command([str(target/'markie-mermaid'),str(variant_source),'-o',str(variant)+'.svg'],Path(str(variant)+'.cli.log'))
                    if result.returncode or rendered.returncode:
                        failures.append(f'{shape} {direction} self-loop API/CLI failed'); continue
                    variant_root=ET.parse(str(variant)+'.fragment.svg').getroot()
                    extra,_=validate({'checks':{'self_loop':True}},variant_root); failures.extend(extra)
                    elements=list(variant_root.iter()); curve=next(e for e in elements if tag(e)=='path' and ' C ' in e.attrib.get('d',''))
                    coords=list(map(float,re.findall(r'-?\d+(?:\.\d+)?',curve.attrib['d']))); endpoints=[coords[:2],coords[-2:]]
                    if shape=='diamond':
                        outline=next(e for e in elements if tag(e)=='polygon' and 'stroke' in e.attrib); ps=points(outline)
                        cx=sum(q[0] for q in ps)/4; cy=sum(q[1] for q in ps)/4; hw=max(q[0] for q in ps)-cx; hh=max(q[1] for q in ps)-cy
                        scores=[abs(x-cx)/hw+abs(y-cy)/hh for x,y in endpoints]
                    else:
                        outline=next(e for e in elements if tag(e)=='circle'); cx,cy,r=[float(outline.attrib[k]) for k in ['cx','cy','r']]
                        scores=[math.dist((cx,cy),q)/r for q in endpoints]
                    if any(abs(score-1)>.005 for score in scores): failures.append(f'{shape} {direction} self-loop endpoints miss outline: {scores}')
        if checks.get('sequence_self'):
            for name,op in [('bare-solid','->'),('bare-dotted','-->'),('closed-solid','->>'),('closed-dotted','-->>'),('async-solid','-)'),('async-dotted','--)'),('cross-solid','-x'),('cross-dotted','--x')]:
                for mode,target_id in [('forward','B'),('self','A')]:
                    variant=Path(str(stem)+f'-sequence-{mode}-{name}'); variant_source=variant.with_suffix('.mmd')
                    variant_source.write_text(f'sequenceDiagram\nparticipant A as Caller\nparticipant B as Server\nA{op}{target_id}: operation\n')
                    result=run_command([str(probe),str(variant_source),str(variant)],Path(str(variant)+'.api.log'))
                    rendered=run_command([str(target/'markie-mermaid'),str(variant_source),'-o',str(variant)+'.svg'],Path(str(variant)+'.cli.log'))
                    if result.returncode or rendered.returncode: failures.append(f'{mode} {op} API/CLI failed'); continue
                    variant_root=ET.parse(str(variant)+'.fragment.svg').getroot()
                    extra,_=validate({'checks':{'texts':['operation'],'sequence_op':op,'sequence_self':mode=='self'}},variant_root)
                    failures.extend(f'{mode} {op}: {error}' for error in extra)
        if checks.get('exports') and not failures:
            exports=[]
            for export_case in [case]+[c for c in CASES if c['id'] in ['MER-109','MER-138','MER-171','MER-184']]:
                export_stem=Path(str(stem)+'-'+export_case['area'])
                export_source=export_stem.with_suffix('.mmd'); export_source.write_text(export_case['input'])
                dimensions=None
                for extension in ['svg','png','pdf']:
                    output=Path(str(export_stem)+'.'+extension)
                    command=[str(target/'markie-mermaid'),str(export_source),'-o',str(output)]
                    if extension=='png': command+=['--png-scale','2']
                    exported=run_command(command,Path(str(output)+'.log'))
                    if exported.returncode: failures.append(f'{extension} export failed: {exported.stderr}'); continue
                    try:
                        data=output.read_bytes()
                        if extension=='svg':
                            exported_svg=ET.fromstring(data); dimensions=(float(exported_svg.attrib['width']),float(exported_svg.attrib['height']))
                            assert any(tag(e)=='text' for e in exported_svg.iter()),'SVG export lost diagram text'
                        elif extension=='png':
                            assert data[:8]==b'\x89PNG\r\n\x1a\n','Invalid PNG signature'
                            actual=struct.unpack('>II',data[16:24])
                            assert dimensions and all(abs(a-2*b)<=1 for a,b in zip(actual,dimensions)),f'Wrong scaled PNG dimensions {actual}'
                            assert b'IDAT' in data and len(data)>1000,'PNG export has no image content'
                        else:
                            assert data.startswith(b'%PDF-') and data.rstrip().endswith(b'%%EOF'),'Invalid PDF structure'
                            assert b'/Page' in data and len(data)>1000,'PDF export has no page content'
                        exports.append(str(output.relative_to(ROOT)))
                    except Exception as error: failures.append(f'{export_case["area"]} {extension} export: {error}')
            Path(str(stem)+'.exports.json').write_text(json.dumps(exports,indent=2)+'\n')
            notes.append(f'Executed {len(exports)}/15 SVG/PNG/PDF exports across five families')
        if checks.get('markdown'):
            md=Path(str(stem)+'.md'); md.write_text('# Audit\n\n```mermaid\n'+case['input']+'\n```\n')
            result=run_command([str(target/'markie'),str(md),'-o',str(stem)+'.markdown.svg'],Path(str(stem)+'.markdown.log'))
            if result.returncode==0 or 'line' not in result.stderr: failures.append('Markdown failed to give source-location rejection: '+result.stderr)
        status='unsupported' if case['boundary'] and not failures else ('failed' if failures else 'passed')
        if status=='passed' and phase=='targeted': status='retesting'
        verdict='failed: ' if failures else ('unsupported: ' if case['boundary'] else ('passed: ' if phase=='full' else phase+': passed '))
        row={'id':ident,'status':status,'test_command':PREFIX+f'python3 tests/audit/mermaid/run.py test --phase {phase} --case {ident}', 'evidence':str(stem.relative_to(ROOT))+'.{mmd,ast.txt,fragment.svg,svg,api.log,cli.log}', 'retest':verdict+'public API + standalone CLI '+stamp}
        if failures: row['errors']=phase+' '+stamp+'; severity='+('high' if case['area'] in ['class','state','er'] else 'medium')+'; expected='+case['expected']+'; actual='+' | '.join(failures)+'; repro='+row['test_command']
        results.append(dict(case=ident,feature=case['feature'],area=case['area'],status=status,failures=failures,notes=notes,stem=str(stem.relative_to(ROOT))))
        Path(str(stem)+'.result.json').write_text(json.dumps(results[-1],indent=2,ensure_ascii=False)+'\n')
        updates.append(row)
        print(ident,status,case['feature'],flush=True)
        if len(updates) % 10 == 0:
            batch=run_dir/f'batch-{len(updates):03}.json'
            batch.write_text(json.dumps(updates[-10:],indent=2,ensure_ascii=False)+'\n')
            run_command(['python3','scripts/feature-audit.py','import',str(batch)],batch.with_suffix('.log'))
    (run_dir/'results.json').write_text(json.dumps(results,indent=2,ensure_ascii=False)+'\n'); (run_dir/'updates.json').write_text(json.dumps(updates,indent=2,ensure_ascii=False)+'\n')
    if len(updates)%10:
        pending=run_dir/'pending.json'; pending.write_text(json.dumps(updates[-(len(updates)%10):],indent=2))
        run_command(['python3','scripts/feature-audit.py','import',str(pending)],run_dir/'import.log')
    (ART/'latest-run.txt').write_text(str(run_dir.relative_to(ROOT))+'\n')
    (run_dir/'environment.json').write_text(json.dumps({'stamp':stamp,'phase':phase,'binary_sha256':hashlib.sha256((target/'markie-mermaid').read_bytes()).hexdigest(),'counts':Counter(r['status'] for r in results),'source_hashes':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (ROOT/'src/mermaid').glob('*.rs')}},indent=2)+'\n')
    print(run_dir,Counter(r['status'] for r in results))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('command',choices=['inventory','test']); parser.add_argument('--case',action='append'); parser.add_argument('--phase',choices=['baseline','targeted','full'],default='targeted'); args=parser.parse_args(); cases(); ART.mkdir(parents=True,exist_ok=True); INV.parent.mkdir(parents=True,exist_ok=True)
    if args.command=='inventory': inventory()
    else: test([c for c in CASES if not args.case or c['id'] in args.case],args.phase)
