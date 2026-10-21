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
