import {$,button,paragraph,actions,updateSelectors} from './ui.js';
const validNumber = value => /^[0-9]{1,11}$/.test(String(value));
export const advanced = {
  'cpu-lock':['CPU clock lock','Requests each cluster’s highest advertised clock inside its current maximum. This can substantially increase battery use and heat. Expires after 60 seconds.'],
  'gpu-lock':['GPU clock lock','Requests the highest advertised Mali clock inside the current maximum. This can substantially increase heat and battery use. Expires after 60 seconds.'],
  'gpu-thermal':['GPU cooling override','Requests cooling state zero for the Mali cooling device. This does not disable every thermal safeguard. Expires after 60 seconds.'],
  'charging-thermal':['Charging thermal override','Removes one exposed charging thermal vote. It does not bypass every charging safeguard or guarantee uncapped wattage. Expires after 30 minutes at most.'],
  'bypass':['Battery bypass','Requires an advertised kernel bypass control and a separate active-state readback. Unsupported stock kernels cannot enable it. Ends on unplug, low battery, heat or after 30 minutes.'],
  'charging-pause':['Experimental charging pause','Disables the primary MediaTek charger. This is not confirmed direct-power bypass: the battery may still discharge, and charge-pump behavior depends on the ROM. No charge-current workaround is forced. Ends on unplug, low battery, heat or after 30 minutes.']
};
export function formatClock(value, gpu = false) {
  const hz = Number(value) * (gpu ? 1 : 1000);
  if (!Number.isFinite(hz) || hz <= 0) return 'Not reported';
  const amount = hz >= 1e9 ? hz / 1e9 : hz / 1e6;
  return Number(amount.toFixed(3)) + (hz >= 1e9 ? ' GHz' : ' MHz');
}
export function clusterNames(policies) {
  const peak = policy => Number(policy.hardwareMax) || Math.max(0,...String(policy.frequencies || '').split(/\s+/).filter(validNumber).map(Number));
  const peaks = [...new Set(policies.map(peak).filter(value => value > 0))].sort((a,b) => a-b);
  return new Map(policies.map((policy,index) => {
    const rank = peaks.indexOf(peak(policy));
    const cores = String(policy.cores || '').split(/\s+/).filter(value => /^[0-9]+$/.test(value)).length;
    let name = 'CPU cluster '+(index+1);
    if (peaks.length > 1 && rank === 0) name = 'Efficiency cores (little)';
    else if (peaks.length > 2 && rank === peaks.length-1 && cores === 1) name = 'Prime core';
    else if (rank > 0) name = 'Performance cores (big)';
    return [policy.id,{name,cores,peak:peak(policy)}];
  }));
}
function selection(id, title, choices, value, host, format = value => value) {
  const label = document.createElement('label'); label.className = 'field-label'; label.htmlFor = id; label.textContent = title;
  const select = document.createElement('select'); select.id = id;
  for (const choice of choices) { const option = document.createElement('option'); option.value = choice; option.textContent = format(choice); select.append(option); }
  select.value = value; host.append(label,select); return select;
}
function range(host, id, data, run) {
  const frequencies = [...new Set(String(data.frequencies || '').trim().split(/\s+/).filter(validNumber).map(Number))].filter(v => v > 0 && v >= Number(data.min) && v <= Number(data.max)).sort((a,b) => a-b).map(String);
  if (!frequencies.length) { host.append(paragraph('This kernel does not expose a supported frequency table.')); return; }
  const grid = document.createElement('div'); grid.className = 'range-fields'; const lowHost = document.createElement('div'), highHost = document.createElement('div');
  const format = value => formatClock(value,id === 'gpu');
  const low = selection('minimum-'+id,'Minimum clock',frequencies,frequencies.includes(String(data.min)) ? String(data.min) : frequencies[0],lowHost,format);
  const high = selection('maximum-'+id,'Maximum clock',frequencies,frequencies.includes(String(data.max)) ? String(data.max) : frequencies.at(-1),highHost,format);
  grid.append(lowHost, highHost); host.append(grid,actions(button('Apply range', () => {
    if (Number(low.value) > Number(high.value)) { run(null,'Minimum must not exceed maximum.'); return; }
    run('range '+id+' '+low.value+':'+high.value);
  }),button('Restore', () => run('restore-range '+id),data.active === true)));
}
function toggle(title, checked, enabled, command, run, summary) {
  const row = document.createElement('div'); row.className = 'setting-row control-switch';
  const heading = document.createElement('h3'); heading.textContent = title;
  const label = document.createElement('label'); label.className = 'switch';
  const input = document.createElement('input'); input.type = 'checkbox'; input.checked = checked;
  input.setAttribute('aria-label',title); input.setAttribute('role','switch'); input.dataset.capable = String(enabled); input.disabled = !enabled;
  const track = document.createElement('span'); track.setAttribute('aria-hidden','true');
  input.addEventListener('change',async()=>{
    const requested = input.checked;
    if (requested && command.confirm) { input.checked = checked; command.confirm(); return; }
    input.disabled = true; row.setAttribute('aria-busy','true');
    try { if (!await run(requested ? command.on : command.off)) input.checked = checked; }
    finally { row.removeAttribute('aria-busy'); input.disabled = input.dataset.capable === 'false'; }
  });
  const copy = document.createElement('div'); copy.className = 'control-copy'; copy.append(heading);
  if (summary) copy.append(paragraph(summary,'caption'));
  label.append(input,track); row.append(copy,label); return row;
}
export function renderControls(state, run, requestAdvanced) {
  const cpu = $('cpu-controls'); cpu.replaceChildren();
  const policies = state.cpuPolicies || [];
  const allowed = ['schedutil','ondemand','conservative','powersave','interactive'];
  const common = allowed.filter(value => policies.length && policies.every(policy => (policy.choices || []).includes(value)));
  const current = new Set(policies.map(policy => policy.governor));
  const unified = document.createElement('div'); unified.className = 'control';
  const heading = document.createElement('h3'); heading.textContent = 'All CPU clusters';
  unified.append(heading,paragraph('Current governor: '+(current.size === 1 ? policies[0].governor : 'Mixed'),'caption'),paragraph('Optional. Keep the ROM governor unless you have a reason to change it.','caption'));
  if (common.length) {
    const selected = selection('governor-all','CPU governor',common,current.size === 1 && common.includes(policies[0].governor) ? policies[0].governor : '',unified);
    if (!selected.value) { const placeholder = document.createElement('option'); placeholder.value = ''; placeholder.textContent = 'Choose a governor'; placeholder.disabled = true; placeholder.selected = true; selected.prepend(placeholder); }
    unified.append(actions(button('Apply to all clusters',()=> selected.value ? run('governor all '+selected.value) : run(null,'Choose a governor first.'))));
  } else unified.append(paragraph('No selectable governor is shared by every cluster.','caption'));
  unified.append(actions(button('Restore all governors',()=>run('restore governors'),state.governors?.original === true)));
  cpu.append(unified);
  const details = document.createElement('details'); details.className = 'disclosure';
  const summary = document.createElement('summary'); summary.textContent = 'Advanced cluster controls'; details.append(summary); cpu.append(details);
  const names = clusterNames(policies);
  for (const policy of state.cpuPolicies || []) {
    if (!/^policy[0-9]{1,3}$/.test(policy.id)) continue;
    const host = document.createElement('div'); host.className = 'control'; const title = document.createElement('h3'); const cluster = names.get(policy.id); title.textContent = cluster.name; host.append(title,paragraph(cluster.cores+' '+(cluster.cores === 1 ? 'core' : 'cores')+' · supported up to '+formatClock(cluster.peak),'cluster-meta'),paragraph('Effective limits: '+formatClock(policy.min)+' – '+formatClock(policy.max),'caption'));
    if (validNumber(policy.requestedMin) && validNumber(policy.requestedMax)) host.append(paragraph('Preset request: '+formatClock(policy.requestedMin)+' – '+formatClock(policy.requestedMax),'preset-request'));
    const allowed = ['schedutil','ondemand','conservative','powersave','interactive']; const choices = [...new Set([policy.governor,...(policy.choices || [])])].filter(v => /^[a-z][a-z0-9_]{0,31}$/.test(v));
    const governor = selection('governor-'+policy.id,'CPU governor',choices,policy.governor,host);
    host.append(actions(button('Apply governor', () => { if (allowed.includes(governor.value)) run('governor '+policy.id+' '+governor.value); else run(null,'This governor is shown for reference but cannot be selected by butter.'); },state.governors?.original !== true),button('Restore governor', () => run('restore '+policy.id),policy.original === true)));
    range(host,policy.id,{frequencies:policy.frequencies,min:policy.min,max:policy.max,active:policy.rangeActive},run); details.append(host);
  }
  if (!policies.length) cpu.append(paragraph('CPU controls could not be discovered. Export diagnostics for the exact failure.'));
  const gpu = $('gpu-controls'); gpu.replaceChildren(); const gpuRange = state.gpuRange || {};
  if (validNumber(gpuRange.min) && validNumber(gpuRange.max)) gpu.append(paragraph('Effective limits: '+formatClock(gpuRange.min,true)+' – '+formatClock(gpuRange.max,true),'caption'));
  if (validNumber(gpuRange.requestedMin) && validNumber(gpuRange.requestedMax)) gpu.append(paragraph('Preset request: '+formatClock(gpuRange.requestedMin,true)+' – '+formatClock(gpuRange.requestedMax,true),'preset-request'));
  range(gpu,'gpu',gpuRange,run);
  const touch = state.touch || {}; $('touch-state').textContent = touch.available ? (touch.driverHz ? 'Driver reports '+touch.driverHz+' Hz. ' : '')+(touch.state === 'Readback changed; request no longer active' ? 'The requested state did not persist. A ROM service or the driver may manage this setting.' : touch.state) : touch.reason || 'Touch control is not exposed by this driver.';
  if (touch.available && !touch.driverHz) $('touch-state').textContent += ' HTSR is a request; ROM touch services may reapply their own state.';
  const touchActions = $('touch-controls'); touchActions.replaceChildren();
  for (const value of ['240','480','high']) touchActions.append(button(value === 'high' ? 'High sampling' : value+' Hz',() => run('enable touch '+value),touch.available && (value === 'high' || Number.isFinite(touch.driverHz))));
  touchActions.append(button('Restore',()=>run('restore touch'),touch.available && touch.state !== 'Off'));
  const tcp = $('tcp-controls'); tcp.replaceChildren();
  if (state.tcp?.available) {
    const choices = state.tcp.choices.filter(v => /^[a-z][a-z0-9_-]{0,31}$/.test(v)); const selected = selection('tcp-choice','TCP congestion control',choices,state.tcp.current,tcp);
    tcp.append(actions(button('Apply',()=>run('tcp '+selected.value)),button('Restore',()=>run('restore tcp'),state.tcp.original === true)));
  } else tcp.append(paragraph(state.tcp?.reason || 'The kernel does not expose TCP controls.'));
  const audio = state.audio || {}, audioHost = $('audio-controls'); audioHost.replaceChildren();
  const savedMic = !!state.supervisor?.config?.controls?.['phone-mic'];
  let audioSummary = !audio.available ? audio.reason || 'Input-routing API is not exposed by this ROM.' : !audio.bluetoothConnected ? 'Connect Bluetooth earbuds. A saved preference will resume when they reconnect.' : audio.active ? 'Phone microphone preference is set.' : 'Ready to set a phone microphone preference.';
  const observed = audio.activeInputs || [];
  if (observed.length) audioSummary += observed.every(input=>input.phoneMic) ? ' Observed recording inputs use the phone microphone.' : observed.some(input=>input.phoneMic) ? ' Observed inputs include both phone and external microphones.' : ' Observed recording inputs still use an external microphone.';
  else audioSummary += ' No active app input is reported; actual routing is unverified.';
  audioHost.append(toggle('Prefer phone microphone',savedMic || audio.active === true,savedMic || audio.active === true || (audio.available && audio.bluetoothConnected),{on:'enable phone-mic',off:'restore phone-mic'},run,audioSummary));
  const errors = state.supervisor?.runtime?.preference_errors || {};
  $('preferences-status').textContent = Object.keys(errors).length ? 'Some saved options could not resume: '+Object.entries(errors).map(([name,error])=>name+': '+error).join(' · ') : 'Applied options are remembered after reboot. Temporary overrides retain their original expiry.';
  const host = $('advanced-controls'); host.replaceChildren();
  for (const [key,[title,description]] of Object.entries(advanced)) {
    const capability = state[key] || {};
    const summary = capability.available ? capability.state : 'Not supported by this kernel';
    host.append(toggle(title,capability.state === 'Request active',capability.available === true,{off:'restore '+key,confirm:()=>requestAdvanced(key,title,description)},run,summary));
  }
  updateSelectors();
}
