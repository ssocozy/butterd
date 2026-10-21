'use strict';
const models = new Set(['bluejay','panther','cheetah','lynx','shiba','husky','akita','tokay','caiman','komodo','tegu']);
const selector = document.querySelector('#model'), apply = document.querySelector('#apply'), feedback = document.querySelector('#feedback');
let current = '', busy = false, serial = 0;
function exec(command) {
  return new Promise((resolve, reject) => {
    if (!window.ksu || typeof window.ksu.exec !== 'function') { reject(new Error('Open this page in KernelSU WebUI or a compatible Magisk WebUI host.')); return; }
    const callback = 'butterdResult' + (++serial);
    const timer = setTimeout(() => { delete window[callback]; reject(new Error('The root manager did not respond.')); }, 15000);
    window[callback] = (code, stdout, stderr) => {
      clearTimeout(timer); delete window[callback];
      if (Number(code) !== 0) reject(new Error(String(stderr || stdout || 'Command failed.')));
      else resolve(String(stdout || ''));
    };
    try { window.ksu.exec(command, '{}', callback); }
    catch (error) { clearTimeout(timer); delete window[callback]; reject(error); }
  });
}
function controls() { selector.disabled = busy; apply.disabled = busy || !current || current === selector.value; }
selector.addEventListener('change', controls);
apply.addEventListener('click', async () => {
  const model = selector.value;
  if (!models.has(model) || busy) return;
  busy = true; controls(); feedback.textContent = 'Saving profile…';
  try {
    feedback.textContent = await exec('/data/adb/modules/butterd/bin/butterctl identity ' + model);
    current = model;
  } catch (error) { feedback.textContent = error.message; }
  finally { busy = false; controls(); }
});
async function readings() {
  const button = document.querySelector('#refresh'); button.disabled = true;
  try { document.querySelector('#readings').textContent = await exec('/data/adb/modules/butterd/bin/butterctl status'); }
  catch (error) { document.querySelector('#readings').textContent = error.message; }
  finally { button.disabled = false; }
}
document.querySelector('#refresh').addEventListener('click', readings);
(async () => {
  try {
    current = (await exec('/data/adb/modules/butterd/bin/butterctl identity')).trim();
    if (!models.has(current)) throw new Error('The active profile could not be identified. Reinstall the module to restore it.');
    selector.value = current; feedback.textContent = 'Profile active. No manual fetch required.'; controls();
  } catch (error) { feedback.textContent = error.message; }
  await readings();
})();
const featureDescriptions = {
  'bypass': ['Battery bypass', 'Requires a kernel-advertised bypass control and a separate active-state readback. This is not available on every stock kernel. Ends on unplug, low battery, heat or after 30 minutes.'],
  'charging-pause': ['Experimental charging pause', 'Disables the primary charger using the MediaTek current command. This is not verified direct-power bypass: the battery may still discharge, and charge-pump behavior depends on your ROM. No wattage or charge-current workaround is forced. Ends on unplug, low battery, heat or after 30 minutes.'],
  'cpu-lock': ['CPU clock lock', 'Requests each CPU cluster’s highest advertised clock within its current maximum limit. This can substantially increase battery use and heat.'],
  'gpu-lock': ['GPU clock lock', 'Requests the highest advertised Mali clock within the current maximum limit. This can substantially increase battery use and heat.'],
  'gpu-thermal': ['GPU cooling override', 'Requests cooling state zero for the Duchamp Mali cooling device. This removes that cooling restriction temporarily; the ROM may replace it. It does not disable all thermal protection.'],
  'charging-thermal': ['Charging thermal vote override', 'Enables the stock thermal_remove flag. The audited driver ignores one charging thermal vote; it does not remove all charging limits or guarantee uncapped wattage. Charging temperature may rise.']
};
let engineBusy = false, pendingFeature = '';
const engineFeedback = document.querySelector('#engine-feedback'), riskDialog = document.querySelector('#risk-dialog');
function engineButtons(disabled) { document.querySelectorAll('[data-enable],[data-restore],[data-touch],.cpu-control,.tcp-control,.engine-control').forEach(button => { button.disabled = disabled || button.dataset.available !== 'true'; }); }
async function engineCommand(command) {
  if (engineBusy) return;
  engineBusy = true; engineButtons(true); engineFeedback.textContent = 'Applying and checking the request…';
  try { engineFeedback.textContent = await exec('/data/adb/modules/butterd/bin/butter ' + command); }
  catch (error) { engineFeedback.textContent = error.message; }
  finally { engineBusy = false; await engineStatus(); }
}
async function engineStatus() {
  try {
    const state = JSON.parse(await exec('/data/adb/modules/butterd/bin/butter status'));
    renderCpuPolicies(state.cpuPolicies || []);
    renderTcp(state.tcp);
    renderSupervisor(state.supervisor);
    renderRange(document.querySelector('#gpu-range'), 'gpu', state.gpuRange || {});
    const container = document.querySelector('#advanced-controls'); container.replaceChildren();
    for (const [feature, [title, description]] of Object.entries(featureDescriptions)) {
      const capability = state[feature] || {available:false}, row = document.createElement('div'); row.className = 'control';
      const heading = document.createElement('h3'); heading.textContent = title; row.append(heading);
      const status = document.createElement('p'); status.textContent = capability.available ? capability.state : 'Not exposed by this kernel'; row.append(status);
      const actions = document.createElement('div'); actions.className = 'actions';
      const enable = document.createElement('button'); enable.textContent = 'Configure'; enable.dataset.enable = feature; enable.dataset.available = String(capability.available);
      enable.addEventListener('click', () => {
        pendingFeature = feature; document.querySelector('#risk-title').textContent = title;
        document.querySelector('#risk-description').textContent = description;
        document.querySelector('#risk-ack').checked = false; document.querySelector('#risk-apply').disabled = true; riskDialog.returnValue = 'cancel'; riskDialog.showModal();
      });
      const restore = document.createElement('button'); restore.textContent = 'Restore'; restore.dataset.restore = feature;
      restore.dataset.available = String(capability.available && capability.state !== 'Off');
      restore.addEventListener('click', () => engineCommand('restore ' + feature));
      if (!capability.available && capability.reason) status.textContent += ' · ' + capability.reason;
      actions.append(enable, restore); row.append(actions); container.append(row);
    }
    const touch = state.touch;
    document.querySelector('#touch-status').textContent = touch.available ? touch.driverHz ? 'Driver reports ' + touch.driverHz + ' Hz. ' + touch.state : 'Mode flag available; physical Hz is not measured. ' + touch.state : 'Touch control is not exposed by this kernel.';
    document.querySelectorAll('[data-touch]').forEach(button => { button.dataset.available = String(touch.available && (button.dataset.touch === 'high' || Number.isFinite(touch.driverHz))); });
    const restoreTouch = document.querySelector('[data-restore="touch"]'); restoreTouch.dataset.available = String(touch.available && touch.state !== 'Off');
    engineButtons(engineBusy);
  } catch (error) { engineFeedback.textContent = error.message; engineButtons(true); }
}
document.querySelector('#risk-ack').addEventListener('change', event => { document.querySelector('#risk-apply').disabled = !event.target.checked; });
riskDialog.addEventListener('close', () => {
  if (riskDialog.returnValue === 'apply' && document.querySelector('#risk-ack').checked && Object.hasOwn(featureDescriptions, pendingFeature))
    engineCommand('enable ' + pendingFeature + ' --accept-risk');
});
document.querySelectorAll('[data-touch]').forEach(button => button.addEventListener('click', () => engineCommand('enable touch ' + button.dataset.touch)));
document.querySelector('[data-restore="touch"]').addEventListener('click', () => engineCommand('restore touch'));
document.querySelector('#refresh').addEventListener('click', engineStatus);
engineStatus();

function renderCpuPolicies(policies) {
  const host = document.querySelector('#cpu-controls'); host.replaceChildren();
  if (!policies.length) { host.textContent = 'CPU governor controls are not exposed.'; return; }
  const allowedGovernors = new Set(['schedutil','ondemand','conservative','powersave','interactive']);
  for (const policy of policies) {
    if (!/^policy[0-9]{1,3}$/.test(policy.id)) continue;
    const row = document.createElement('div'); row.className = 'control';
    const label = document.createElement('label'); label.htmlFor = 'governor-' + policy.id;
    label.textContent = policy.id + ' / cores ' + policy.cores;
    const select = document.createElement('select'); select.id = label.htmlFor; select.className = 'cpu-control'; select.dataset.available = 'true';
    const options = [...new Set([policy.governor, ...policy.choices.filter(value => allowedGovernors.has(value))])];
    for (const governor of options) {
      const option = document.createElement('option'); option.value = governor; option.textContent = governor;
      option.disabled = !allowedGovernors.has(governor); select.append(option);
    }
    select.value = policy.governor;
    const actions = document.createElement('div'); actions.className = 'actions';
    const applyGovernor = document.createElement('button'); applyGovernor.textContent = 'Apply governor'; applyGovernor.className = 'cpu-control'; applyGovernor.dataset.available = 'false'; applyGovernor.disabled = true;
    select.addEventListener('change', () => { applyGovernor.dataset.available = String(select.value !== policy.governor && allowedGovernors.has(select.value)); applyGovernor.disabled = engineBusy || applyGovernor.dataset.available !== 'true'; });
    async function changeGovernor(restoring) {
      if (engineBusy || (!restoring && !allowedGovernors.has(select.value))) return;
      engineBusy = true; engineButtons(true); engineFeedback.textContent = 'Applying governor…';
      const command = '/data/adb/modules/butterd/bin/butterctl ' + (restoring ? 'restore ' + policy.id : 'governor ' + policy.id + ' ' + select.value);
      try { engineFeedback.textContent = await exec(command); }
      catch (error) { engineFeedback.textContent = error.message; }
      finally { engineBusy = false; await engineStatus(); }
    }
    applyGovernor.addEventListener('click', () => changeGovernor(false));
    const restore = document.createElement('button'); restore.textContent = 'Restore'; restore.className = 'cpu-control'; restore.dataset.available = String(policy.original === true); restore.addEventListener('click', () => changeGovernor(true));
    actions.append(applyGovernor,restore); row.append(label,select,actions);
    const range = document.createElement('div'); renderRange(range, policy.id, {frequencies:policy.frequencies,min:policy.min,max:policy.max,active:policy.rangeActive}); row.append(range); host.append(row);
  }
}

function renderTcp(tcp) {
  const host = document.querySelector('#tcp-controls'); host.replaceChildren();
  if (!tcp || !tcp.available) { host.textContent = 'TCP controls are not exposed by this kernel.'; return; }
  const valid = value => typeof value === 'string' && /^[a-z][a-z0-9_-]{0,31}$/.test(value);
  const label = document.createElement('label'); label.htmlFor = 'tcp-algorithm'; label.textContent = 'Congestion-control algorithm';
  const select = document.createElement('select'); select.id = label.htmlFor; select.className = 'tcp-control'; select.dataset.available = 'true';
  for (const algorithm of [...new Set(tcp.choices.filter(valid))]) {
    const option = document.createElement('option'); option.value = algorithm; option.textContent = algorithm; select.append(option);
  }
  select.value = tcp.current;
  const actions = document.createElement('div'); actions.className = 'actions';
  const apply = document.createElement('button'); apply.textContent = 'Apply algorithm'; apply.className = 'tcp-control'; apply.dataset.available = 'false';
  select.addEventListener('change', () => { apply.dataset.available = String(valid(select.value) && select.value !== tcp.current); apply.disabled = engineBusy || apply.dataset.available !== 'true'; });
  apply.addEventListener('click', () => { if (valid(select.value)) engineCommand('tcp ' + select.value); });
  const restore = document.createElement('button'); restore.textContent = 'Restore'; restore.className = 'tcp-control'; restore.dataset.available = String(tcp.original === true);
  restore.addEventListener('click', () => engineCommand('restore tcp')); actions.append(apply,restore); host.append(label,select,actions);
}

function renderRange(host, id, range) {
  host.replaceChildren();
  const min = Number(range.min), max = Number(range.max);
  const clocks = [...new Set(String(range.frequencies || '').trim().split(/\s+/).filter(v => /^[0-9]{1,11}$/.test(v)).map(Number))].filter(v => v > 0 && v >= min && v <= max).sort((a,b) => a-b);
  if (!clocks.length) { const text = document.createElement('p'); text.textContent = 'Frequency ranges are not exposed by this kernel.'; host.append(text); return; }
  const fields = document.createElement('div'); fields.className = 'range-fields';
  const selects = ['Minimum','Maximum'].map((title,index) => {
    const label = document.createElement('label'); label.textContent = title + (id === 'gpu' ? ' · driver units' : ' · kHz');
    const select = document.createElement('select'); select.className = 'engine-control'; select.dataset.available = 'true';
    for (const clock of clocks) { const option = document.createElement('option'); option.value = String(clock); option.textContent = String(clock); select.append(option); }
    select.value = String(index === 0 ? clocks[0] : clocks[clocks.length-1]); label.append(select); fields.append(label); return select;
  });
  const actions = document.createElement('div'); actions.className = 'actions';
  const apply = document.createElement('button'); apply.textContent = 'Apply range'; apply.className = 'engine-control'; apply.dataset.available = 'true';
  apply.addEventListener('click', () => { const low = Number(selects[0].value), high = Number(selects[1].value); if (low > high) { engineFeedback.textContent = 'Minimum must not exceed maximum.'; return; } engineCommand('range ' + id + ' ' + low + ':' + high); });
  const restore = document.createElement('button'); restore.textContent = 'Restore range'; restore.className = 'engine-control'; restore.dataset.available = String(range.active === true); restore.addEventListener('click', () => engineCommand('restore-range ' + id));
  actions.append(apply, restore); host.append(fields, actions);
}
let lastSupervisor = null;
function renderSupervisor(supervisor) {
  if (!supervisor) return;
  lastSupervisor = supervisor;
  const config = supervisor.config || {}, state = supervisor.runtime || {}, data = state.telemetry || {};
  document.querySelector('#automatic').checked = config.automatic === true;
  document.querySelector('#logcat').checked = config.logcat === true;
  document.querySelector('#decision').textContent = (state.active || 'balanced') + ' · ' + (state.reason || 'Monitoring is stopped.') + (state.error ? ' · ' + state.error : '') + (state.sampled_at && supervisor.uptime - state.sampled_at > 30 ? ' · Readings are stale; restart monitoring.' : '');
  const watts = document.querySelector('#live-watts'); watts.textContent = Number.isFinite(data.batteryWatts) ? (data.batteryWatts > 0 ? '+' : '') + data.batteryWatts.toFixed(1) : '—';
  const metrics = document.querySelector('#live-metrics'); metrics.replaceChildren();
  const entries = [ ['Battery', data.batteryPercent == null ? 'Not reported' : data.batteryPercent + '%'], ['Temperature', data.batteryTempC == null ? 'Not reported' : data.batteryTempC.toFixed(1) + ' °C'], ['Display', data.screenHz == null ? 'Not reported' : data.screenHz.toFixed(1) + ' Hz'], ['CPU busy', data.cpuBusyPercent == null ? 'Collecting samples' : data.cpuBusyPercent.toFixed(0) + '%'], ['Charge cycles', data.cycleCount == null ? 'Not reported by firmware' : String(data.cycleCount)], ['Battery health', data.healthPercent == null ? (data.health || 'Not reported by firmware') : data.healthPercent.toFixed(0) + '% of design capacity'] ];
  for (const [name,value] of entries) { const dt = document.createElement('dt'), dd = document.createElement('dd'); dt.textContent = name; dd.textContent = value; metrics.append(dt,dd); }
  const host = document.querySelector('#app-rules'); host.replaceChildren();
  for (const rule of config.rules || []) {
    if (!/^[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)+$/.test(rule.package)) continue;
    const row = document.createElement('div'); row.className = 'control'; const text = document.createElement('p'); text.textContent = rule.package + ' · ' + rule.preset + ' · ' + (rule.hz ? rule.hz + ' Hz' : 'ROM refresh rate');
    const remove = document.createElement('button'); remove.textContent = 'Remove rule'; remove.className = 'engine-control'; remove.dataset.available = 'true'; remove.addEventListener('click', () => engineCommand('app ' + rule.package + ' remove rom')); row.append(text,remove); host.append(row);
  }
}
const presetDialog = document.querySelector('#preset-dialog');
document.querySelector('#open-presets').addEventListener('click', () => presetDialog.showModal());
document.querySelectorAll('[data-preset]').forEach(button => button.addEventListener('click', () => { presetDialog.close(); engineCommand('preset ' + button.dataset.preset); }));
document.querySelectorAll('[data-command]').forEach(button => button.addEventListener('click', () => engineCommand(button.dataset.command)));
document.querySelector('#automatic').addEventListener('change', event => engineCommand('auto ' + (event.target.checked ? 'on' : 'off')));
document.querySelector('#logcat').addEventListener('change', event => engineCommand('logging ' + (event.target.checked ? 'on' : 'off')));
document.querySelector('#app-rule').addEventListener('submit', event => {
  event.preventDefault(); const packageName = document.querySelector('#app-package').value.trim(), mode = document.querySelector('#app-preset').value, hz = document.querySelector('#app-hz').value;
  if (!/^[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)+$/.test(packageName) || !['balanced','performance','battery'].includes(mode) || !['rom','60','90','120'].includes(hz)) { engineFeedback.textContent = 'Enter a valid app package and supported options.'; return; }
  engineCommand('app ' + packageName + ' ' + mode + ' ' + hz);
});
async function report(command, download) {
  const feedback = document.querySelector('#report-feedback'), preview = document.querySelector('#report-preview');
  feedback.textContent = 'Collecting engine diagnostics…';
  try {
    const raw = await exec('/data/adb/modules/butterd/bin/butter ' + command); const text = JSON.stringify(JSON.parse(raw), null, 2);
    preview.hidden = false; preview.textContent = text; feedback.textContent = 'Report ready. Review app names before sharing.';
    if (download) { const url = URL.createObjectURL(new Blob([text], {type:'application/json'})); const link = document.createElement('a'); link.href = url; link.download = 'butter-diagnostics.json'; document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 60000); }
  } catch (error) { feedback.textContent = error.message; }
}
document.querySelector('#export-report').addEventListener('click', () => report('diagnostics', true));
document.querySelector('#measure-app').addEventListener('click', () => { const packageName = lastSupervisor?.runtime?.foreground; if (!packageName || !/^[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)+$/.test(packageName)) { document.querySelector('#report-feedback').textContent = 'Start monitoring with one app in the foreground first.'; return; } report('measure ' + packageName, false); });
setInterval(async () => {
  if (document.hidden || engineBusy) return;
  try { const state = JSON.parse(await exec('/data/adb/modules/butterd/bin/butter status')); if (!engineBusy) renderSupervisor(state.supervisor); } catch (_) { /* Explicit refresh retains actionable errors. */ }
}, 5000);
