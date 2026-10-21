import {icon,mountIcons} from './icons.js';
mountIcons();
import {exec,binary,validPackage} from './bridge.js';
import {$,notify,button,paragraph,setBusy,updateSelectors} from './ui.js';
import {renderControls,advanced} from './controls.js';
let snapshot = null, busy = false, polling = false, hardwareSignature = '', selectedApp = null, apps = [], appsLoaded = false, loadingApps = false, appRulesSignature = '';
const models = {bluejay:'Pixel 6a',panther:'Pixel 7',cheetah:'Pixel 7 Pro',lynx:'Pixel 7a',shiba:'Pixel 8',husky:'Pixel 8 Pro',akita:'Pixel 8a',tokay:'Pixel 9',caiman:'Pixel 9 Pro',komodo:'Pixel 9 Pro XL',tegu:'Pixel 9a'};
for (const [value,label] of Object.entries(models)) { const option = document.createElement('option'); option.value = value; option.textContent = label; $('identity').append(option); }
function setPage() {
  const page = ['overview','apps','settings'].includes(location.hash.slice(1)) ? location.hash.slice(1) : 'overview';
  for (const name of ['overview','apps','settings']) $('page-'+name).hidden = name !== page;
  document.querySelectorAll('[data-page]').forEach(link => { if (link.dataset.page === page) link.setAttribute('aria-current','page'); else link.removeAttribute('aria-current'); });
  window.scrollTo(0,0); if (page === 'apps' && !appsLoaded) loadApps();
}
window.addEventListener('hashchange',setPage);
window.addEventListener('error',event=>notify('Interface error: '+event.message,true));
window.addEventListener('unhandledrejection',event=>notify('Request failed: '+(event.reason?.message || event.reason),true));
for (const node of document.querySelectorAll('[data-close]')) node.addEventListener('click',()=>$(node.dataset.close).close());
function detailsRow(host,name,value) { const row = document.createElement('div'), dt = document.createElement('dt'), dd = document.createElement('dd'); dt.textContent = name; dd.textContent = value; row.append(dt,dd); host.append(row); }
function render(state) {
  snapshot = state;
  const supervisor = state.supervisor || {}, config = supervisor.config || {}, runtime = supervisor.runtime || {}, health = supervisor.health || {}, telemetry = runtime.telemetry || {};
  const sampled = runtime.sampled_at > 0, age = health.sampleAgeSeconds ?? (sampled ? supervisor.uptime-runtime.sampled_at : null), running = health.running === true;
  $('engine-state').textContent = health.error || config.error || runtime.error ? 'Needs attention' : running ? sampled ? 'Monitoring active' : 'Starting monitoring' : 'Monitoring stopped';
  $('engine-reason').textContent = health.error || config.error || runtime.error || runtime.reason || (running ? 'Waiting for the first sensor sample.' : 'Tap Start / retry monitoring in Settings.');
  if (running && age > 30) { $('engine-state').textContent = 'Readings are stale'; $('engine-reason').textContent = 'The supervisor has not produced a recent sample. Export diagnostics.'; }
  $('preset-label').textContent = runtime.active ? runtime.active[0].toUpperCase()+runtime.active.slice(1) : 'Balanced';
  $('automatic').checked = config.automatic === true; $('logging').checked = config.logcat === true;
  $('watts').textContent = Number.isFinite(telemetry.batteryWatts) ? (telemetry.batteryWatts > 0 ? '+' : '')+telemetry.batteryWatts.toFixed(1) : '—';
  $('power-status').textContent = telemetry.batteryStatus || (sampled ? 'Sensor not exposed' : running ? 'Collecting a sample' : 'Monitoring stopped');
  $('battery').textContent = telemetry.batteryPercent == null ? '—' : telemetry.batteryPercent+'%';
  $('temperature').textContent = telemetry.batteryTempC == null ? '—' : telemetry.batteryTempC.toFixed(1)+'°C';
  $('display').textContent = telemetry.screenHz == null ? '—' : Math.round(telemetry.screenHz)+' Hz';
  $('sample-age').textContent = age == null ? '' : age < 3 ? 'Just updated' : age+'s ago';
  const metrics = $('device-metrics'); metrics.replaceChildren();
  for (const [name,value] of [['SoC temperature',telemetry.socTempC == null ? null : telemetry.socTempC.toFixed(1)+' °C'],['CPU activity',telemetry.cpuBusyPercent == null ? null : telemetry.cpuBusyPercent.toFixed(0)+'%'],['GPU activity',telemetry.gpuBusyPercent == null ? null : telemetry.gpuBusyPercent.toFixed(0)+'%'],['Charge cycles',telemetry.cycleCount],['Battery health',telemetry.healthPercent == null ? telemetry.health : telemetry.healthPercent.toFixed(0)+'% of design capacity']]) if (value != null) detailsRow(metrics,name,String(value));
  $('sensor-note').textContent = sampled ? 'Only readings exposed by your firmware are shown. Missing readings do not imply a sensor failure.' : running ? 'Monitoring is starting. Readings appear after the first sample.' : 'Sensor readings need the supervisor. Diagnostics remain accessible while it is stopped.';
  const signature = JSON.stringify([state.cpuPolicies,state.gpuRange,state.touch,state.tcp,...Object.keys(advanced).map(key=>state[key])]);
  if (signature !== hardwareSignature && !$('choice-dialog')?.open) { renderControls(state,run,requestAdvanced); hardwareSignature = signature; }
  $('hardware-details').textContent = JSON.stringify({cpu:state.cpuPolicies,gpu:state.gpuRange,touch:state.touch,tcp:state.tcp},null,2);
  const compatibility = state.compatibility || {};
  if (models[compatibility.selectedModel]) { if (!$('choice-dialog')?.open) $('identity').value = compatibility.selectedModel; $('identity-summary').textContent = models[compatibility.selectedModel]+' · configured'; }
  const spoofs = $('spoof-options'); spoofs.replaceChildren();
  for (const [key,name] of [['spoofBuild','Build spoofing'],['spoofVendingBuild','Play Store build spoofing'],['spoofProps','Scoped property spoofing']]) detailsRow(spoofs,name,compatibility[key] == null ? 'Cannot read configuration' : compatibility[key] ? 'Enabled' : 'Disabled');
  $('version').textContent = 'Duchamp · butter '+(state.engine?.version || 'Rust');
  const rulesSignature = JSON.stringify(config.rules || []);
  if (appsLoaded && rulesSignature !== appRulesSignature) { appRulesSignature = rulesSignature; renderApps(); }
  updateSelectors();
}
async function refresh() {
  if (polling) return; polling = true;
  try { render(JSON.parse(await exec(binary+' status'))); }
  catch (error) { $('engine-state').textContent = 'Connection failed'; $('engine-reason').textContent = error.message; notify(error.message,true); }
  finally { polling = false; }
}
async function run(command,message,quiet=false) {
  if (!command) { notify(message || 'Invalid request.',true); return; }
  if (busy) return;
  busy = true; setBusy(true); if (!quiet) notify('Applying your request…');
  try { const result = await exec(binary+' '+command); if (!quiet) notify(result || 'Request completed.'); }
  catch (error) { notify(error.message,true); }
  finally { busy = false; await refresh(); setBusy(false); updateSelectors(); }
}
$('refresh').addEventListener('click',refresh);
$('open-presets').addEventListener('click',()=>$('preset-dialog').showModal());
document.querySelectorAll('[data-preset]').forEach(node=>node.addEventListener('click',()=>{ $('preset-dialog').close(); run('preset '+node.dataset.preset); }));
$('boost').addEventListener('click',()=>run('boost 5')); $('release').addEventListener('click',()=>run('reset'));
$('automatic').addEventListener('change',event=>run('auto '+(event.target.checked ? 'on' : 'off')));
$('logging').addEventListener('change',event=>run('logging '+(event.target.checked ? 'on' : 'off')));
$('restart').addEventListener('click',()=>run('monitor'));
let requestedAdvanced = '';
function requestAdvanced(key,title,description) { requestedAdvanced=key; $('risk-title').textContent=title; $('risk-copy').textContent=description; $('risk-ack').checked=false; $('risk-apply').disabled=true; $('risk-dialog').showModal(); }
$('risk-ack').addEventListener('change',event=>{ $('risk-apply').disabled=!event.target.checked; });
$('risk-form').addEventListener('submit',event=>{ event.preventDefault(); if ($('risk-ack').checked && Object.hasOwn(advanced,requestedAdvanced)) { $('risk-dialog').close(); run('enable '+requestedAdvanced+' --accept-risk'); } });
async function loadApps() {
  if (loadingApps) return; loadingApps=true; $('app-message').textContent='Reading installed apps…'; $('reload-apps').disabled=true;
  try { const catalogue=JSON.parse(await exec(binary+' apps')); apps=(catalogue.apps || []).filter(app=>validPackage(app.package)); appsLoaded=true; $('app-message').textContent=catalogue.warning || 'Tap an app to configure its preset and refresh rate.'; renderApps(); }
  catch (error) { $('app-message').textContent=error.message; notify('Could not load apps: '+error.message,true); }
  finally { loadingApps=false; $('reload-apps').disabled=false; }
}
$('reload-apps').addEventListener('click',loadApps); $('app-search').addEventListener('input',renderApps);
function renderApps() {
  const term=$('app-search').value.trim().toLocaleLowerCase(), rules=snapshot?.supervisor?.config?.rules || [];
  const visible=apps.filter(app=>(String(app.label)+' '+app.package).toLocaleLowerCase().includes(term)).sort((a,b)=>Number(rules.some(rule=>rule.package===b.package))-Number(rules.some(rule=>rule.package===a.package)) || String(a.label).localeCompare(String(b.label)));
  $('app-count').textContent=visible.length+' installed app'+(visible.length===1 ? '' : 's'); const host=$('app-list'); host.replaceChildren();
  if (!visible.length) { host.append(paragraph(term ? 'No apps match your search.' : 'No launchable apps were returned. Tap Reload to retry.','empty-state')); return; }
  for (const app of visible) {
    const row=button('',()=>openApp(app),true,'app-row');
    let icon; if (typeof app.icon==='string' && /^data:image\/png;base64,[A-Za-z0-9+/=]{1,3000}$/.test(app.icon)) { icon=document.createElement('img'); icon.src=app.icon; icon.alt=''; icon.loading='lazy'; } else { icon=document.createElement('span'); icon.textContent=String(app.label || app.package).slice(0,1).toUpperCase(); icon.setAttribute('aria-hidden','true'); } icon.className='app-icon';
    const info=document.createElement('span'); info.className='app-name'; const label=document.createElement('strong'); label.textContent=app.label || app.package; const detail=document.createElement('small'); detail.textContent=app.package; info.append(label,detail); row.append(icon,info);
    const rule=rules.find(rule=>rule.package===app.package); if (rule) { const badge=document.createElement('span'); badge.className='app-badge'; badge.textContent=rule.preset; row.append(badge); } else if (app.game) { const badge=document.createElement('span'); badge.className='app-badge'; badge.textContent='Game'; row.append(badge); }
    const chevron=document.createElement('span'); chevron.className='chevron'; chevron.append(icon('caret-right')); row.append(chevron); host.append(row);
  }
}
function openApp(app) {
  selectedApp=app; const rule=snapshot?.supervisor?.config?.rules?.find(rule=>rule.package===app.package);
  $('app-title').textContent=app.label || app.package; $('app-package').textContent=app.package; $('app-preset').value=rule?.preset || 'balanced';
  const rate=$('app-hz'); rate.replaceChildren(); const rom=document.createElement('option'); rom.value='rom'; rom.textContent='Let the ROM decide'; rate.append(rom);
  const rates=snapshot?.supervisor?.runtime?.telemetry?.supportedHz || [];
  for (const hz of rates.filter(hz=>[60,90,120].includes(hz))) { const option=document.createElement('option'); option.value=String(hz); option.textContent=hz+' Hz'; rate.append(option); }
  rate.value=rule?.hz && rates.includes(rule.hz) ? String(rule.hz) : 'rom'; $('remove-app').disabled=!rule; updateSelectors(); $('app-dialog').showModal();
}
$('app-form').addEventListener('submit',event=>{ event.preventDefault(); if (!selectedApp || !validPackage(selectedApp.package)) return; const preset=$('app-preset').value, hz=$('app-hz').value; if (!['balanced','performance','battery'].includes(preset) || !['rom','60','90','120'].includes(hz)) return; $('app-dialog').close(); run('app '+selectedApp.package+' '+preset+' '+hz); });
$('remove-app').addEventListener('click',()=>{ if (selectedApp && validPackage(selectedApp.package)) { $('app-dialog').close(); run('app '+selectedApp.package+' remove rom'); } });
$('save-identity').addEventListener('click',async()=>{
  const model=$('identity').value; if (!Object.hasOwn(models,model) || busy) return;
  busy=true; setBusy(true);
  try { notify(await exec('/data/adb/modules/butterd/bin/butterctl identity '+model)); }
  catch (error) { notify(error.message,true); }
  finally { busy=false; await refresh(); setBusy(false); }
});
async function report(command,download) {
  try {
    notify('Collecting diagnostics…'); const text=JSON.stringify(JSON.parse(await exec(binary+' '+command)),null,2); $('report').textContent=text; $('report').hidden=false; location.hash='settings';
    if (download) { const url=URL.createObjectURL(new Blob([text],{type:'application/json'})), link=document.createElement('a'); link.href=url; link.download='butter-diagnostics.json'; document.body.append(link); link.click(); link.remove(); setTimeout(()=>URL.revokeObjectURL(url),60000); }
    notify('Report ready below. Review app names before sharing.');
  } catch (error) { notify(error.message,true); }
}
$('export').addEventListener('click',()=>report('diagnostics',true));
$('measure-app').addEventListener('click',()=>{ if (selectedApp && validPackage(selectedApp.package)) { $('app-dialog').close(); report('measure '+selectedApp.package,false); } });
try { const theme=localStorage.getItem('butter-theme'); if (['light','dark'].includes(theme)) { document.documentElement.dataset.theme=theme; $('theme').value=theme; } } catch (_) { /* Appearance remains system-controlled. */ }
$('theme').addEventListener('change',event=>{ const theme=event.target.value; if (!['system','light','dark'].includes(theme)) return; if (theme==='system') delete document.documentElement.dataset.theme; else document.documentElement.dataset.theme=theme; try { localStorage.setItem('butter-theme',theme); } catch (_) { /* Theme still applies for this session. */ } });
setPage(); updateSelectors();
(async()=>{
  try {
    const uid=await exec('/system/bin/id -u'); if (uid!=='0') throw new Error('The WebUI host has no root access. Grant its app root permission in KernelSU.');
    await refresh();
    if (snapshot && snapshot.supervisor?.health?.running!==true) { await run('monitor',null,true); }
  } catch (error) { $('engine-state').textContent='Root connection needed'; $('engine-reason').textContent=error.message; notify(error.message,true); }
})();
setInterval(()=>{ if (!document.hidden && !busy) refresh(); },5000);
