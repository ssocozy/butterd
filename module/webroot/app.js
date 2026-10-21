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
  'cpu-lock': ['CPU clock lock', 'Requests each CPU cluster’s highest advertised clock within its current maximum limit. This can substantially increase battery use and heat.'],
  'gpu-lock': ['GPU clock lock', 'Requests the highest advertised Mali clock within the current maximum limit. This can substantially increase battery use and heat.'],
  'gpu-thermal': ['GPU cooling override', 'Requests cooling state zero for the Duchamp Mali cooling device. This removes that cooling restriction temporarily; the ROM may replace it. It does not disable all thermal protection.'],
  'charging-thermal': ['Charging thermal vote override', 'Enables the stock thermal_remove flag. The audited driver ignores one charging thermal vote; it does not remove all charging limits or guarantee uncapped wattage. Charging temperature may rise.']
};
let engineBusy = false, pendingFeature = '';
const engineFeedback = document.querySelector('#engine-feedback'), riskDialog = document.querySelector('#risk-dialog');
function engineButtons(disabled) { document.querySelectorAll('[data-enable],[data-restore],[data-touch],.cpu-control').forEach(button => { button.disabled = disabled || button.dataset.available !== 'true'; }); }
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
    const container = document.querySelector('#advanced-controls'); container.replaceChildren();
    for (const [feature, [title, description]] of Object.entries(featureDescriptions)) {
      const capability = state[feature], row = document.createElement('div'); row.className = 'control';
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
    actions.append(applyGovernor,restore); row.append(label,select,actions); host.append(row);
  }
}
