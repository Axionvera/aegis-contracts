/**
 * Single-file dashboard UI. Embedded as a string so the monitoring service has
 * zero build step and zero static-asset deployment concerns.
 */

export const DASHBOARD_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Aegis Event Monitor</title>
<style>
  :root {
    --bg: #0b0f17; --panel: #131a26; --panel-2: #1a2333; --line: #24304a;
    --text: #e6edf7; --muted: #8b9bb4; --accent: #4da3ff; --ok: #3ddc97;
    --warn: #ffb84d; --crit: #ff5c73; --mint: #a78bfa;
  }
  * { box-sizing: border-box; }
  body { margin:0; background:var(--bg); color:var(--text);
    font:14px/1.5 ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif; }
  header { display:flex; align-items:center; gap:16px; padding:14px 20px;
    background:var(--panel); border-bottom:1px solid var(--line); position:sticky; top:0; z-index:5; }
  h1 { font-size:17px; margin:0; letter-spacing:.3px; }
  .dot { width:9px; height:9px; border-radius:50%; background:var(--muted); display:inline-block; }
  .dot.live { background:var(--ok); box-shadow:0 0 0 3px rgba(61,220,151,.18); }
  .dot.down { background:var(--crit); }
  .pill { background:var(--panel-2); border:1px solid var(--line); border-radius:999px;
    padding:3px 10px; font-size:12px; color:var(--muted); }
  main { padding:18px; display:grid; gap:16px; max-width:1400px; margin:0 auto; }
  .grid { display:grid; gap:14px; }
  .kpis { grid-template-columns:repeat(auto-fit,minmax(165px,1fr)); }
  .cols { grid-template-columns:1.35fr .85fr; }
  @media (max-width:1000px){ .cols{grid-template-columns:1fr;} }
  .card { background:var(--panel); border:1px solid var(--line); border-radius:12px; padding:14px 16px; }
  .card h2 { font-size:12px; text-transform:uppercase; letter-spacing:.09em;
    color:var(--muted); margin:0 0 10px; font-weight:600; }
  .kpi .v { font-size:26px; font-weight:650; letter-spacing:-.5px; }
  .kpi .s { font-size:11px; color:var(--muted); }
  table { width:100%; border-collapse:collapse; font-size:12.5px; }
  th { text-align:left; color:var(--muted); font-weight:600; padding:6px 8px;
    border-bottom:1px solid var(--line); font-size:11px; text-transform:uppercase; letter-spacing:.05em; }
  td { padding:6px 8px; border-bottom:1px solid rgba(36,48,74,.5); vertical-align:top; }
  tbody tr:hover { background:rgba(77,163,255,.05); }
  .mono { font-family:ui-monospace,SFMono-Regular,Menlo,monospace; font-size:11.5px; }
  .tag { display:inline-block; padding:1px 7px; border-radius:5px; font-size:11px; font-weight:600; }
  .tag.mint{background:rgba(167,139,250,.16); color:var(--mint);}
  .tag.transfer{background:rgba(77,163,255,.16); color:var(--accent);}
  .tag.wl_add{background:rgba(61,220,151,.16); color:var(--ok);}
  .tag.yield{background:rgba(255,184,77,.16); color:var(--warn);}
  .tag.init{background:rgba(139,155,180,.16); color:var(--muted);}
  .sev-info{color:var(--accent);} .sev-warning{color:var(--warn);} .sev-critical{color:var(--crit);}
  .chart { display:flex; align-items:flex-end; gap:3px; height:110px; padding-top:6px; }
  .bar { flex:1; min-width:3px; background:linear-gradient(180deg,var(--accent),rgba(77,163,255,.25));
    border-radius:3px 3px 0 0; transition:height .3s; }
  .muted{color:var(--muted);} .right{text-align:right;}
  .empty{color:var(--muted); font-size:12.5px; padding:14px 4px; text-align:center;}
  .toolbar{display:flex; gap:8px; flex-wrap:wrap; align-items:center; margin-bottom:10px;}
  input,select,button{background:var(--panel-2); color:var(--text); border:1px solid var(--line);
    border-radius:7px; padding:5px 9px; font-size:12.5px; font-family:inherit;}
  button{cursor:pointer;} button:hover{border-color:var(--accent);}
  .flash{animation:flash .9s ease-out;}
  @keyframes flash{from{background:rgba(77,163,255,.18);}to{background:transparent;}}
</style>
</head>
<body>
<header>
  <h1>Aegis Event Monitor</h1>
  <span class="pill"><span id="dot" class="dot"></span> <span id="conn">connecting…</span></span>
  <span class="pill" id="transport">transport: —</span>
  <span class="pill" id="network">network: —</span>
  <span class="pill" id="ledger">ledger: —</span>
</header>

<main>
  <section class="grid kpis">
    <div class="card kpi"><h2>Events</h2><div class="v" id="k-events">0</div><div class="s" id="k-eps">0/s</div></div>
    <div class="card kpi"><h2>Minted</h2><div class="v" id="k-minted">0</div><div class="s">cumulative</div></div>
    <div class="card kpi"><h2>Transferred</h2><div class="v" id="k-transferred">0</div><div class="s">cumulative</div></div>
    <div class="card kpi"><h2>Whitelisted</h2><div class="v" id="k-wl">0</div><div class="s">compliance adds</div></div>
    <div class="card kpi"><h2>Addresses</h2><div class="v" id="k-addr">0</div><div class="s">unique seen</div></div>
    <div class="card kpi"><h2>Alerts</h2><div class="v" id="k-alerts">0</div><div class="s" id="k-crit">0 critical</div></div>
  </section>

  <section class="card">
    <h2>Event throughput</h2>
    <div class="chart" id="chart"></div>
  </section>

  <section class="grid cols">
    <div class="card">
      <h2>Live events</h2>
      <div class="toolbar">
        <select id="f-action">
          <option value="">all actions</option>
          <option>mint</option><option>transfer</option>
          <option>wl_add</option><option>yield</option><option>init</option>
        </select>
        <input id="f-addr" placeholder="filter address…" size="20" />
        <button id="btn-replay">Replay from disk</button>
        <button id="btn-clear">Clear</button>
        <span class="muted" id="count"></span>
      </div>
      <table>
        <thead><tr><th>Action</th><th>Ledger</th><th>Details</th><th class="right">Amount</th></tr></thead>
        <tbody id="events"></tbody>
      </table>
      <div class="empty" id="events-empty">Waiting for events…</div>
    </div>

    <div class="card">
      <h2>Alerts</h2>
      <table>
        <thead><tr><th>Sev</th><th>Rule</th><th>Message</th></tr></thead>
        <tbody id="alerts"></tbody>
      </table>
      <div class="empty" id="alerts-empty">No alerts fired.</div>

      <h2 style="margin-top:18px">Top addresses</h2>
      <table><tbody id="top"></tbody></table>
    </div>
  </section>
</main>

<script>
const $ = (id) => document.getElementById(id);
let events = [], alerts = [], totalAlerts = 0, criticalAlerts = 0;

const short = (a) => (typeof a === 'string' && a.length > 12) ? a.slice(0,5)+'…'+a.slice(-4) : (a ?? '—');
const fmt = (n) => { if (n==null) return '—'; try { return BigInt(n).toLocaleString('en-US'); } catch { return String(n); } };

function connect() {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const ws = new WebSocket(proto + '://' + location.host + '/ws');

  ws.onopen = () => { $('dot').className = 'dot live'; $('conn').textContent = 'live'; };
  ws.onclose = () => {
    $('dot').className = 'dot down'; $('conn').textContent = 'reconnecting…';
    setTimeout(connect, 1500);
  };
  ws.onmessage = (e) => {
    let msg; try { msg = JSON.parse(e.data); } catch { return; }
    if (msg.type === 'hello') {
      $('network').textContent = 'network: ' + (msg.payload.network ?? '—');
      $('transport').textContent = 'transport: ' + (msg.payload.transport ?? '—');
      (msg.payload.recent || []).forEach(addEvent);
      if (msg.payload.analytics) renderAnalytics(msg.payload.analytics);
      render();
    } else if (msg.type === 'event' || msg.type === 'replay') {
      addEvent(msg.payload); render();
    } else if (msg.type === 'alert') {
      alerts.unshift(msg.payload); alerts = alerts.slice(0, 60);
      totalAlerts++; if (msg.payload.severity === 'critical') criticalAlerts++;
      renderAlerts();
    } else if (msg.type === 'analytics' && msg.payload) {
      renderAnalytics(msg.payload);
    } else if (msg.type === 'transport') {
      $('transport').textContent = 'transport: ' + msg.payload;
    }
  };
}

function addEvent(ev) { events.unshift(ev); events = events.slice(0, 200); }

function passes(ev) {
  const a = $('f-action').value, addr = $('f-addr').value.trim();
  if (a && ev.action !== a) return false;
  if (addr) {
    const pool = JSON.stringify(ev.subjects || []) + JSON.stringify(ev.fields || {});
    if (!pool.includes(addr)) return false;
  }
  return true;
}

function details(ev) {
  const f = ev.fields || {};
  switch (ev.action) {
    case 'transfer': return short(f.from) + ' → ' + short(f.to);
    case 'mint':     return '→ ' + short(f.to);
    case 'wl_add':   return short(f.user) + ' by ' + short(f.admin);
    case 'yield':    return 'admin ' + short(f.admin);
    case 'init':     return 'admin ' + short(f.admin);
    default:         return (ev.topics || []).map(String).join(' / ').slice(0, 60);
  }
}

function render() {
  const rows = events.filter(passes);
  $('count').textContent = rows.length + ' shown';
  $('events-empty').style.display = rows.length ? 'none' : 'block';
  $('events').innerHTML = rows.slice(0, 80).map((ev, i) =>
    '<tr class="' + (i === 0 ? 'flash' : '') + '">' +
      '<td><span class="tag ' + (ev.action || 'init') + '">' + (ev.action || ev.type || '?') + '</span></td>' +
      '<td class="mono">' + (ev.ledger ?? '—') + '</td>' +
      '<td class="mono">' + details(ev) + '</td>' +
      '<td class="right mono">' + fmt(ev.fields && ev.fields.amount) + '</td>' +
    '</tr>').join('');
}

function renderAlerts() {
  $('k-alerts').textContent = totalAlerts;
  $('k-crit').textContent = criticalAlerts + ' critical';
  $('alerts-empty').style.display = alerts.length ? 'none' : 'block';
  $('alerts').innerHTML = alerts.slice(0, 30).map((a) =>
    '<tr><td class="sev-' + a.severity + '">●</td>' +
    '<td class="mono">' + a.rule + '</td>' +
    '<td>' + (a.message || '').replace(/</g, '&lt;') + '</td></tr>').join('');
}

function renderAnalytics(s) {
  const t = s.totals || {};
  $('k-events').textContent = t.events ?? 0;
  $('k-minted').textContent = fmt(t.minted);
  $('k-transferred').textContent = fmt(t.transferred);
  $('k-wl').textContent = t.whitelisted ?? 0;
  $('k-addr').textContent = t.uniqueAddresses ?? 0;
  $('k-eps').textContent = ((s.window && s.window.eventsPerSecond) || 0) + '/s';
  if (s.lastLedger) $('ledger').textContent = 'ledger: ' + s.lastLedger;

  const series = s.series || [];
  const max = Math.max(1, ...series.map((b) => b.count));
  $('chart').innerHTML = series.slice(-60).map((b) =>
    '<div class="bar" style="height:' + Math.max(3, (b.count / max) * 100) + '%" title="' +
    new Date(b.ts).toLocaleTimeString() + ': ' + b.count + '"></div>').join('') ||
    '<div class="empty">No data in window</div>';

  const top = (s.window && s.window.topAddresses) || [];
  $('top').innerHTML = top.map((a) =>
    '<tr><td class="mono">' + short(a.address) + '</td><td class="right mono">' + a.count + '</td></tr>'
  ).join('') || '<tr><td class="empty">—</td></tr>';
}

$('f-action').onchange = render;
$('f-addr').oninput = render;
$('btn-clear').onclick = () => { events = []; render(); };
$('btn-replay').onclick = async () => {
  const action = $('f-action').value;
  await fetch('/api/replay', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ filter: action ? { action } : null, limit: 200, includeEvents: false }),
  });
};

fetch('/api/alerts?limit=30').then((r) => r.json()).then((d) => {
  alerts = d.alerts || []; totalAlerts = alerts.length;
  criticalAlerts = alerts.filter((a) => a.severity === 'critical').length;
  renderAlerts();
}).catch(() => {});

connect();
</script>
</body>
</html>`;
