let sequence = 0;
export function exec(command) {
  return new Promise((resolve, reject) => {
    const bridge = globalThis.ksu;
    if (!bridge || typeof bridge.exec !== 'function') { reject(new Error('Open Butterd in KernelSU or a compatible WebUI host.')); return; }
    const callback = 'butterdResult' + (++sequence);
    const timer = setTimeout(() => { delete globalThis[callback]; reject(new Error('The root host timed out. Export diagnostics or check boot.log.')); }, 20000);
    globalThis[callback] = (code, stdout, stderr) => {
      clearTimeout(timer); delete globalThis[callback];
      if (Number(code) !== 0) reject(new Error(String(stderr || stdout || 'Command failed.').trim()));
      else resolve(String(stdout || '').trim());
    };
    try { bridge.exec(command, '{}', callback); } catch (error) { clearTimeout(timer); delete globalThis[callback]; reject(error); }
  });
}
export const binary = '/data/adb/modules/butterd/bin/butter';
export const validPackage = value => typeof value === 'string' && value.length <= 180 && /^[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)+$/.test(value);
