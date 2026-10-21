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
function selection(id, title, choices, value, host) {
  const label = document.createElement('label'); label.className = 'field-label'; label.htmlFor = id; label.textContent = title;
  const select = document.createElement('select'); select.id = id;
  for (const choice of choices) { const option = document.createElement('option'); option.value = choice; option.textContent = choice; select.append(option); }
  select.value = value; host.append(label,select); return select;
}
function range(host, id, data, run) {
  const frequencies = [...new Set(String(data.frequencies || '').trim().split(/\s+/).filter(validNumber).map(Number))].filter(v => v > 0 && v >= Number(data.min) && v <= Number(data.max)).sort((a,b) => a-b).map(String);
  if (!frequencies.length) { host.append(paragraph('This kernel does not expose a supported frequency table.')); return; }
  const grid = document.createElement('div'); grid.className = 'range-fields'; const lowHost = document.createElement('div'), highHost = document.createElement('div');
  const units = id === 'gpu' ? 'driver units' : 'kHz';
  const low = selection('minimum-'+id,'Minimum · '+units,frequencies,frequencies[0],lowHost);
  const high = selection('maximum-'+id,'Maximum · '+units,frequencies,frequencies.at(-1),highHost);
  grid.append(lowHost, highHost); host.append(grid,actions(button('Apply range', () => {
    if (Number(low.value) > Number(high.value)) { run(null,'Minimum must not exceed maximum.'); return; }
    run('range '+id+' '+low.value+':'+high.value);
  }),button('Restore', () => run('restore-range '+id),data.active === true)));
}
export function renderControls(state, run, requestAdvanced) {
  const cpu = $('cpu-controls'); cpu.replaceChildren();
  for (const policy of state.cpuPolicies || []) {
    if (!/^policy[0-9]{1,3}$/.test(policy.id)) continue;
    const host = document.createElement('div'); host.className = 'control'; const title = document.createElement('h3'); title.textContent = policy.id+' · cores '+policy.cores; host.append(title);
    const allowed = ['schedutil','ondemand','conservative','powersave','interactive']; const choices = [...new Set([policy.governor,...(policy.choices || [])])].filter(v => /^[a-z][a-z0-9_]{0,31}$/.test(v));
    const governor = selection('governor-'+policy.id,'CPU governor',choices,policy.governor,host);
    host.append(actions(button('Apply governor', () => { if (allowed.includes(governor.value)) run('governor '+policy.id+' '+governor.value); else run(null,'This governor is shown for reference but cannot be selected by butter.'); }),button('Restore governor', () => run('restore '+policy.id),policy.original === true)));
    range(host,policy.id,{frequencies:policy.frequencies,min:policy.min,max:policy.max,active:policy.rangeActive},run); cpu.append(host);
  }
  if (!cpu.childElementCount) cpu.append(paragraph('CPU controls could not be discovered. Export diagnostics for the exact failure.'));
  const gpu = $('gpu-controls'); gpu.replaceChildren(); range(gpu,'gpu',state.gpuRange || {},run);
  const touch = state.touch || {}; $('touch-state').textContent = touch.available ? (touch.driverHz ? 'Driver reports '+touch.driverHz+' Hz. ' : '')+touch.state : touch.reason || 'Touch control is not exposed by this driver.';
  const touchActions = $('touch-controls'); touchActions.replaceChildren();
  for (const value of ['240','480','high']) touchActions.append(button(value === 'high' ? 'High sampling' : value+' Hz',() => run('enable touch '+value),touch.available && (value === 'high' || Number.isFinite(touch.driverHz))));
  touchActions.append(button('Restore',()=>run('restore touch'),touch.available && touch.state !== 'Off'));
  const tcp = $('tcp-controls'); tcp.replaceChildren();
  if (state.tcp?.available) {
    const choices = state.tcp.choices.filter(v => /^[a-z][a-z0-9_-]{0,31}$/.test(v)); const selected = selection('tcp-choice','TCP congestion control',choices,state.tcp.current,tcp);
    tcp.append(actions(button('Apply',()=>run('tcp '+selected.value)),button('Restore',()=>run('restore tcp'),state.tcp.original === true)));
  } else tcp.append(paragraph(state.tcp?.reason || 'The kernel does not expose TCP controls.'));
  const host = $('advanced-controls'); host.replaceChildren();
  for (const [key,[title,description]] of Object.entries(advanced)) {
    const capability = state[key] || {}, row = document.createElement('div'); row.className = 'control'; const heading = document.createElement('h3'); heading.textContent = title;
    row.append(heading,paragraph(capability.available ? capability.state : capability.reason || 'Not exposed by this kernel.'));
    row.append(actions(button('Configure',()=>requestAdvanced(key,title,description),capability.available === true),button('Restore',()=>run('restore '+key),capability.available && capability.state !== 'Off'))); host.append(row);
  }
  updateSelectors();
}
