const $ = id => document.getElementById(id);
let data = [], deliverables = {}, previous = '';
const escape = value => String(value || '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const latest = value => (value || '').split(/\n\[[^\]\n]+\] /).pop().replace(/^\[[^\]\n]+\] /, '');
const verified = row => row.status === 'passed' && /^passed(?:\b|:)/i.test(latest(row.retest));
const chosen = row => row.showcase && !/^(no|false|none|0)$/i.test(row.showcase.trim());
const time = value => { const d = new Date(value); return isNaN(d) ? value : d.toLocaleString('en-NZ', {timeZone:'Pacific/Auckland',dateStyle:'medium',timeStyle:'short'}); };
function evidence(value) {
  return value.split(/[\n;]+/).map(line => {
    let path = line.trim().replace(/^.*?\.amp\/in\/audit\//, '').replace(/^.*?\.amp\/in\/artifacts\/(?=feature-audit\/(?:markdown|mermaid|cli|showcase)\/)/, '');
    if (line.includes('.amp/in/audit/') || /\.amp\/in\/artifacts\/feature-audit\/(?:markdown|mermaid|cli|showcase)\//.test(line) || line.startsWith('/artifacts/')) {
      path = path.replace(/^\/artifacts\//, '');
      return `<a href="/artifacts/${path.split('/').map(encodeURIComponent).join('/')}" target="_blank" rel="noopener">↗ ${escape(line)}</a>`;
    }
    return escape(line);
  }).join('<br>');
}
function render() {
  const n = data.length, done = data.filter(verified).length;
  // Floor so 99.9% never rounds up to a misleading 100%.
  const percent = n ? Math.floor(done / n * 100) : 0;
  $('percent').innerHTML = `${percent}<span>%</span>`;
  $('progress').style.width = `${percent}%`;
  $('verified').textContent = n ? `${done} of ${n} current stories passed a recorded retest · code-derived scope` : 'Inventory pending · no stories tested';
  const fail = data.filter(r => ['failed','fixing'].includes(r.status)).length;
  const blocked = data.filter(r => ['blocked','unsupported'].includes(r.status)).length;
  const untested = data.filter(r => r.status === 'inventoried').length;
  $('stats').innerHTML = [[n,'Stories inventoried','ALL AREAS'],[untested,'Untested','QUEUED'],[done,'Verified passed','RETESTED'],[fail,'Failures / fixing','ATTENTION'],[blocked,'Blocked / unsupported','UNRESOLVED']].map(([v,label,tag]) => `<div class="stat"><small>${tag}</small><strong>${v}</strong><span>${label}</span></div>`).join('');
  const areas = [...new Set(data.map(r => r.area || 'Unassigned'))].sort();
  const selected = $('area').value;
  $('area').innerHTML = '<option value="">All areas</option>' + areas.map(a => `<option>${escape(a)}</option>`).join('');
  $('area').value = selected;
  $('areas').innerHTML = areas.length ? areas.map(a => {const group = data.filter(r => (r.area || 'Unassigned') === a), pass = group.filter(verified).length; return `<div class="area-row"><div><span>${escape(a)}</span><span>${pass} / ${group.length}</span></div><div class="track"><div></div></div></div>`;}).join('') : '<p class="hint">Markdown, Mermaid and CLI coverage will appear as inventories arrive.</p>';
  // Widths are set as DOM properties to respect the dashboard CSP.
  $('areas').querySelectorAll('.track>div').forEach((bar,i) => {const group = data.filter(r => (r.area || 'Unassigned') === areas[i]); bar.style.width = `${group.filter(verified).length/group.length*100}%`;});
  $('stages').innerHTML = '<div class="stage-list">' + ['inventoried','testing','failed','fixing','retesting','passed','unsupported','blocked'].map(s => `<div class="stage"><strong>${data.filter(r=>r.status===s).length}</strong><span>${s === 'inventoried' ? 'Untested' : s[0].toUpperCase()+s.slice(1)}</span></div>`).join('') + '</div>';
  const baseline = data.filter(r => r.test_command && r.evidence && ['failed','fixing','retesting','passed','unsupported','blocked'].includes(r.status)).length;
  const fixes = data.filter(r => r.fix).length;
  const videos = deliverables.videos || [];
  const phases = [
    ['01','Inventory',n?`${n} stories · inventory complete`:'Awaiting inventory','Three owner handoffs · code-derived coverage'],
    ['02','Baseline testing',`${baseline} / ${n} results evidenced`,'Command + evidence + recorded result'],
    ['03','Fixes',`${fixes} stories with fix history`,`${fail} currently failed or fixing`],
    ['04','Full retest',`${done} / ${n} verified passed`,'Separate latest passed retest required'],
    ['05','Showcase',videos.length?'Product tour available':'Pending · product tour + verified selection required',`${deliverables.showcase_selected_verified || 0} verified selections · ${deliverables.showcase_tour_exists?'tour file exists':'tour not generated'}`],
    ['06','Report',deliverables.report_final?'Final report available':deliverables.report_exists?'Draft available · final pending':'Pending · no report','File existence does not complete the audit']
  ];
  $('lifecycle').innerHTML = phases.map(([number,title,value,note],i) => `<div class="phase"><div class="eyebrow">${number} / ${title}</div><strong>${escape(value)}</strong><span>${escape(note)}</span>${i===5&&deliverables.report_exists?'<a href="/download/feature-audit-report.md">↓ Read report</a>':''}${i===4&&videos.length?`<a href="${escape(videos[0].url)}">↗ Open video</a>`:''}</div>`).join('');
  ledger();
}
function ledger() {
  const q = $('search').value.toLowerCase(), area = $('area').value, status = $('status').value;
  const filtered = data.filter(r => (!q || Object.values(r).join(' ').toLowerCase().includes(q)) && (!area || (r.area || 'Unassigned') === area) && (!status || r.status === status) && (!$('showcase').checked || chosen(r)));
  const open = new Set([...document.querySelectorAll('details[open]')].map(el=>el.dataset.id));
  $('count').textContent = `${filtered.length} / ${data.length}`;
  $('ledger').innerHTML = filtered.length ? filtered.map(r => {
    const field = (label,value,code=false,wide=false) => `<div class="${wide?'wide':''}"><h3>${label}</h3>${code?`<pre>${escape(value || 'Not recorded')}</pre>`:`<p>${escape(value || 'Not recorded')}</p>`}</div>`;
    const baselineOnly = r.status === 'passed' && !verified(r);
    const label = baselineOnly ? 'Baseline passed · retest due' : r.status === 'inventoried' ? 'Untested' : r.status;
    return `<details class="story" data-id="${escape(r.id)}" ${open.has(r.id)?'open':''}><summary><span class="id">${escape(r.id)}</span><span class="title">${escape(r.feature)}${chosen(r)?' ✦':''}<span class="sub">${escape(r.area)} · ${escape(r.user_story)}</span></span><span class="badge ${baselineOnly?'baseline':escape(r.status)}">${escape(label)}</span><span class="sub owner">${escape(r.owner || 'Unassigned')}</span></summary><div class="detail">${field('User story',r.user_story)}${field('Expected behavior',r.expected_behavior)}${field('Test command',r.test_command,true,true)}${field('Errors · retained history',r.errors,true)}${field('Fix · retained history',r.fix,true)}${field('Retest · recorded separately',r.retest,true)}<div><h3>Evidence</h3><p>${r.evidence?evidence(r.evidence):'No evidence recorded'}</p></div>${field('Source',r.source)}${field('Showcase choice',r.showcase)}${field('Last updated · Auckland',time(r.updated_at))}</div></details>`;
  }).join('') : `<div class="empty"><strong>${data.length?'No matching stories':'Ready for the first inventory'}</strong>${data.length?'Try another search or filter.':'Audit workers are collecting user stories. Nothing is marked passed until it has evidence and a separate recorded retest.'}</div>`;
}
async function refresh() {
  try {
    const response = await fetch('/api/audit', {cache:'no-store'});
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const payload = await response.json();
    const serialized = JSON.stringify(payload);
    if (serialized !== previous) { data = payload.rows; deliverables = payload.deliverables || {}; previous = serialized; render(); }
    $('connection').textContent = 'Live · synced ' + new Date().toLocaleTimeString('en-NZ',{timeZone:'Pacific/Auckland'});
    $('connection').classList.remove('error');
  } catch (error) { $('connection').textContent = 'Connection lost · showing last snapshot'; $('connection').classList.add('error'); }
}
['search','area','status','showcase'].forEach(id => $(id).addEventListener('input', ledger));
$('status').innerHTML += ['inventoried','testing','failed','fixing','retesting','passed','unsupported','blocked'].map(s=>`<option value="${s}">${s==='inventoried'?'Untested':s}</option>`).join('');
render(); refresh(); setInterval(refresh, 2500);
