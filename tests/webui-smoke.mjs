// Browser smoke tests run only in GitHub Actions, using the runner's Chrome.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
const root=path.resolve('module/webroot'), output=path.resolve('dist');
await fs.mkdir(output,{recursive:true});
const server=createServer(async(request,response)=>{
  try {
    const filename=path.resolve(root,'.'+new URL(request.url,'http://localhost').pathname);
    if (!filename.startsWith(root+path.sep)) { response.writeHead(403).end(); return; }
    const types={'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.json':'application/json'};
    const bytes=await fs.readFile(filename);
    response.writeHead(200,{'Content-Type':types[path.extname(filename)] || 'application/octet-stream'}); response.end(bytes);
  } catch (_) { response.writeHead(404).end(); }
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const directory=await fs.mkdtemp(path.join(os.tmpdir(),'butter-webui-'));
const chrome=spawn(process.env.CHROME_BIN || '/usr/bin/google-chrome',['--headless=new','--disable-gpu','--remote-allow-origins=*','--no-sandbox','--disable-dev-shm-usage','--no-first-run','--disable-extensions','--disable-background-networking','--remote-debugging-port=0','--user-data-dir='+directory,'about:blank'],{stdio:['ignore','ignore','pipe']});
let chromeErrors=''; chrome.stderr.on('data',chunk=>{ if(chromeErrors.length<16384) chromeErrors+=chunk; });
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
let socket;
try {
  let port;
  for(let attempt=0;attempt<600;attempt++) {
    const listening=chromeErrors.match(/DevTools listening on ws:\/\/127\.0\.0\.1:([0-9]+)/);
    if(listening) { port=Number(listening[1]); break; }
    try { port=Number((await fs.readFile(path.join(directory,'DevToolsActivePort'),'utf8')).split('\n')[0]); break; } catch (_) { if(chrome.exitCode!==null) throw new Error('Chrome exited: '+chromeErrors); await delay(50); }
  }
  assert.ok(port,'Chrome debugging endpoint did not start: '+chromeErrors);
  const target=await(await fetch('http://127.0.0.1:'+port+'/json/new?about:blank',{method:'PUT'})).json();
  socket=new WebSocket(target.webSocketDebuggerUrl); await new Promise((resolve,reject)=>{ socket.addEventListener('open',resolve,{once:true}); socket.addEventListener('error',reject,{once:true}); });
  let serial=0; const pending=new Map(), exceptions=[];
  socket.addEventListener('message',event=>{
    const message=JSON.parse(event.data);
    if(message.method==='Runtime.exceptionThrown') exceptions.push(message.params.exceptionDetails);
    if(message.id) { const task=pending.get(message.id); if(task){clearTimeout(task.timer);pending.delete(message.id);message.error?task.reject(new Error(JSON.stringify(message.error))):task.resolve(message.result);} }
  });
  const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++serial;const timer=setTimeout(()=>{pending.delete(id);reject(new Error('CDP timeout: '+method));},10000);pending.set(id,{resolve,reject,timer});socket.send(JSON.stringify({id,method,params}));});
  const evaluate=async expression=>{const result=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(result.exceptionDetails)throw new Error(JSON.stringify(result.exceptionDetails));return result.result.value;};
  const until=async(expression)=>{for(let attempt=0;attempt<100;attempt++){if(await evaluate(expression))return;await delay(50);}throw new Error('Condition failed: '+expression);};
  await send('Page.enable'); await send('Runtime.enable');
  await send('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});
  const fixture={
    engine:{version:'0.3.0'},cpuPolicies:[{id:'policy0',cores:'0 1 2 3',governor:'schedutil',choices:['schedutil','powersave'],frequencies:'500000 1000000 2000000',min:'500000',max:'2000000',requestedMin:'1000000',requestedMax:'2000000',original:false,rangeActive:false}],
    gpuRange:{frequencies:'300000000 600000000 1000000000',min:'300000000',max:'1000000000',active:false},
    audio:{available:true,bluetoothConnected:true,active:false,activeInputs:[]},
    touch:{available:true,state:'Off',driverHz:240},tcp:{available:true,current:'cubic',choices:['cubic','bbr'],original:false},
    compatibility:{selectedModel:'bluejay',spoofBuild:true,spoofVendingBuild:true,spoofProps:true},
    supervisor:{health:{running:false,sampleAgeSeconds:null},uptime:102,config:{automatic:false,logcat:false,rules:[]},runtime:{sampled_at:0,active:'balanced',reason:'',telemetry:{}}}
  };
  for(const name of ['cpu-lock','gpu-lock','gpu-thermal','charging-thermal','bypass','charging-pause'])fixture[name]={available:name!=='bypass',state:'Off',reason:'Kernel does not advertise observable battery bypass'};
  await send('Page.addScriptToEvaluateOnNewDocument',{source:`
    globalThis.__micPaints=[];globalThis.__micPaintObserved=false;
    document.addEventListener('change',event=>{if(event.target.matches('#audio-controls input')){globalThis.__micPaintObserved=false;requestAnimationFrame(()=>{globalThis.__micPaintObserved=true;});}});
    globalThis.__butterCommands=[];globalThis.__butterFixture=${JSON.stringify(fixture)};
    globalThis.ksu={exec(command,options,callback){
      if(command.endsWith('phone-mic'))__micPaints.push(__micPaintObserved);
      __butterCommands.push(command);let stdout='Request saved.',code=0,stderr='';const state=__butterFixture;
      if(command.endsWith('id -u'))stdout='0';
      else if(command.endsWith(' monitor')){state.supervisor.health={running:true,sampleAgeSeconds:2};state.supervisor.runtime.sampled_at=100;state.supervisor.runtime.reason='Monitoring only';state.supervisor.runtime.telemetry={batteryPercent:72,batteryWatts:-2.4,batteryTempC:34.2,socTempC:46,screenHz:120,supportedHz:[60,120],cpuBusyPercent:12,gpuBusyPercent:8,cycleCount:125,health:'Good',batteryStatus:'Discharging'};}
      else if(command.endsWith(' enable phone-mic')){state.audio.active=true;state.supervisor.config.controls={'phone-mic':{setting:'',expires_utc:null}};}
      else if(command.endsWith(' restore phone-mic')){if(globalThis.__failMic){code=1;stderr='Example routing rejection';}else{state.audio.active=false;state.supervisor.config.controls={};}}
      else if(command.endsWith(' notify-test'))state.supervisor.notificationDelivery={accepted:true,error:null};
      else if(command.endsWith(' recording on'))state.supervisor.config.recording_assist=true;
      else if(command.endsWith(' recording off'))state.supervisor.config.recording_assist=false;
      else if(command.endsWith(' notifications on'))state.supervisor.config.notifications=true;
      else if(command.endsWith(' notifications off'))state.supervisor.config.notifications=false;
      else if(command.endsWith(' status'))stdout=JSON.stringify(state);
      else if(command.endsWith(' activity'))stdout=JSON.stringify({sampleSeconds:1,apps:[{package:'com.example.game',label:'Pocket Game',rssKb:524288,cpuPercent:12.5,foreground:false,canStop:true},{package:'com.android.systemui',label:'System UI',rssKb:262144,cpuPercent:2,foreground:true,canStop:false,protectedReason:'Protected system app'}]});
      else if(command.includes(' activity stop --confirm '))stdout=JSON.stringify({results:[{package:'com.example.game',result:'stopped',detail:'Android accepted force-stop'}]});
      else if(command.endsWith(' battery-usage'))stdout=JSON.stringify({start:'2026-10-24-03:34',screenOnSeconds:6000,screenOffSeconds:18000,awakeSeconds:7200,sleepEstimateSeconds:16800,sleepEstimatePercent:70,screenOnMahPerHour:500,screenOffMahPerHour:26,screenOnPercentPerHour:13,screenOffPercentPerHour:0.7,note:'Historical estimates; sleep energy is not separately measured.'});
      else if(command.endsWith(' apps'))stdout=JSON.stringify({apps:[{package:'com.example.game',label:'Pocket Game',game:true},{package:'com.example.notes',label:'<img src=x onerror=alert(1)>',game:false}]});
      else if(command.includes('preset performance'))state.supervisor.runtime.active='performance';
      else if(command.includes(' app com.example.game '))state.supervisor.config.rules=[{package:'com.example.game',preset:'performance',hz:120,high_touch:true,charging_pause_at:80}];
      else if(command.includes(' measure '))stdout=JSON.stringify({frames:0,p50Ms:null,p95Ms:null});
      else if(command.endsWith(' diagnostics'))stdout=JSON.stringify({test:true,supervisor:state.supervisor});
      setTimeout(()=>globalThis[callback]?.(code,stdout,stderr),command.endsWith('phone-mic') ? 500 : 1);
    }};`});
  await send('Page.navigate',{url:'http://127.0.0.1:'+server.address().port+'/index.html'});
  await until(`document.getElementById('engine-state')?.textContent==='Monitoring active'`);
  assert.equal(await evaluate(`document.getElementById('watts').textContent`),'-2.4');
  await evaluate(`document.getElementById('refresh').click()`);
  assert.equal(await evaluate(`document.getElementById('refresh').getAttribute('aria-busy')`),'true');
  await until(`document.getElementById('feedback').textContent==='Readings updated.'`);
  await until(`!document.getElementById('refresh').disabled`);

  assert.equal(await evaluate(`document.getElementById('boost')`),null,'Manual launch boost should not be in the interface');
  assert.equal(await evaluate(`document.querySelectorAll('#spoof-options dd').length`),3);
  assert.equal(await evaluate(`document.documentElement.scrollWidth<=innerWidth`),true,'Mobile overview overflows horizontally');
  const screenshot=async name=>{await evaluate(`window.scrollTo(0,0)`);await delay(300);const image=await send('Page.captureScreenshot',{format:'png'});await fs.writeFile(path.join(output,name),Buffer.from(image.data,'base64'));};
  await screenshot('webui-overview.png');
  await evaluate(`document.getElementById('open-presets').click()`); await until(`document.getElementById('preset-dialog').open`);
  await evaluate(`document.querySelector('[data-preset=performance]').click()`); await until(`document.getElementById('preset-label').textContent==='Performance'`);
  await evaluate(`document.getElementById('open-presets').click();document.querySelector('[data-preset=aggressive]').click()`);
  await until(`document.getElementById('aggressive-dialog').open`);
  assert.equal(await evaluate(`document.getElementById('aggressive-apply').disabled`),true);
  assert.equal(await evaluate(`__butterCommands.some(command=>command.includes('preset aggressive'))`),false);
  await evaluate(`document.querySelector('[data-close=aggressive-dialog]').click()`);
  await until(`!document.getElementById('aggressive-dialog').open`);
  await evaluate(`location.hash='apps'`); await until(`document.querySelectorAll('.app-row').length===2`);
  assert.equal(await evaluate(`document.querySelectorAll('#app-list img').length`),0,'An app label was interpreted as HTML');
  await evaluate(`document.getElementById('app-search').value='pocket';document.getElementById('app-search').dispatchEvent(new Event('input'))`);
  assert.equal(await evaluate(`document.querySelectorAll('.app-row').length`),1);
  await evaluate(`document.querySelector('.app-row').click()`); await until(`document.getElementById('app-dialog').open`);
  assert.equal(await evaluate(`document.getElementById('app-package').textContent`),'com.example.game');
  await evaluate(`document.querySelector('[data-select=app-preset]').click()`); await until(`document.getElementById('choice-dialog').open`);
  await evaluate(`[...document.querySelectorAll('#choice-list button')].find(node=>node.textContent.includes('Performance')).click()`);
  await evaluate(`document.querySelector('[data-select=app-hz]').click()`); await until(`document.getElementById('choice-dialog').open`);
  await evaluate(`[...document.querySelectorAll('#choice-list button')].find(node=>node.textContent.includes('120 Hz')).click()`);
  assert.equal(await evaluate(`document.getElementById('app-hz').value`),'120');
  assert.equal(await evaluate(`document.getElementById('measure-app')===null && document.getElementById('frame-result')===null`),true);
  await evaluate(`document.getElementById('app-touch').click();document.getElementById('app-charge').click()`);
  assert.equal(await evaluate(`document.getElementById('app-charge-options').hidden`),false);
  assert.equal(await evaluate(`document.getElementById('app-charge-level').value`),'80');
  await screenshot('webui-app-profile.png');
  await evaluate(`document.getElementById('app-form').dispatchEvent(new Event('submit',{cancelable:true,bubbles:true}))`);
  await until(`__butterCommands.some(command=>command.endsWith('app com.example.game performance 120 high 80'))`);
  await until(`document.body.getAttribute('aria-busy')==='false'`);
  await evaluate(`document.querySelector('.app-row').click()`); await until(`document.getElementById('app-dialog').open`);
  await evaluate(`document.getElementById('app-preset').value='aggressive';document.getElementById('app-form').dispatchEvent(new Event('submit',{cancelable:true,bubbles:true}))`);
  await until(`document.getElementById('aggressive-dialog').open`);
  assert.equal(await evaluate(`__butterCommands.some(command=>command.includes('app com.example.game aggressive'))`),false);
  await evaluate(`document.getElementById('aggressive-ack').click();document.getElementById('aggressive-apply').click()`);
  await until(`__butterCommands.some(command=>command.includes('app com.example.game aggressive') && command.endsWith('--accept-risk')) && document.body.getAttribute('aria-busy')==='false'`);
  assert.equal(await evaluate(`document.querySelectorAll('#hardware-details .hardware-group').length>=4 && document.querySelectorAll('#hardware-details dt').length>5 && !document.querySelector('#hardware-details').textContent.includes('cpuPolicies')`),true);
  await evaluate(`location.hash='settings'`); await until(`!document.getElementById('page-settings').hidden`);
  await evaluate(`document.querySelector('[data-select=theme]').click()`); await until(`document.getElementById('choice-dialog').open`);
  await evaluate(`[...document.querySelectorAll('#choice-list button')].find(node=>node.textContent.includes('Dark')).click()`);
  assert.equal(await evaluate(`document.documentElement.dataset.theme`),'dark');
  const clockLabels = await evaluate(`import('./js/controls.js').then(m=>[m.formatClock('1900000'),m.formatClock('740000000',true),[...m.clusterNames([{id:'policy0',cores:'0 1 2 3',frequencies:'480000 2200000'},{id:'policy4',cores:'4 5 6',frequencies:'400000 3200000'},{id:'policy7',cores:'7',frequencies:'400000 3350000'}]).values()].map(c=>c.name)])`);
  assert.deepEqual(clockLabels,['1.9 GHz','740 MHz',['Efficiency cores (little)','Performance cores (big)','Prime core']]);
  assert.equal(await evaluate(`document.querySelectorAll('.bottom-nav .ph-icon').length`),3);
  assert.equal(await evaluate(`[...document.querySelectorAll('.select-trigger')].every(node=>node.querySelector('svg.ph-icon') && !node.textContent.includes('⌄'))`),true);
  await evaluate(`document.querySelector('#cpu-controls').closest('details').open=true`); await delay(250);
  assert.equal(await evaluate(`document.querySelector('#governor-all').value`),'schedutil');
  assert.equal(await evaluate(`document.querySelectorAll('#touch-controls button').length===0 && document.querySelector('#touch-controls input[role=switch]')!==null`),true);
  assert.equal(await evaluate(`document.querySelector('#recording-assist').checked`),false);
  await evaluate(`document.querySelector('#recording-assist').click()`);
  await until(`document.querySelector('#recording-assist').checked && document.body.getAttribute('aria-busy')==='false'`);
  assert.equal(await evaluate(`__butterCommands.some(command=>command.endsWith(' recording on'))`),true);
  assert.equal(await evaluate(`document.querySelector('#notifications').checked`),false);
  await evaluate(`document.querySelector('#notifications').click()`);
  await until(`document.querySelector('#notifications').checked && document.body.getAttribute('aria-busy')==='false'`);
  assert.equal(await evaluate(`__butterCommands.some(command=>command.endsWith(' notifications on'))`),true);
  await evaluate(`document.querySelector('#test-notification').click()`);
  await until(`document.querySelector('#notification-status').textContent.includes('Android accepted the last notification') && document.body.getAttribute('aria-busy')==='false'`);
  assert.equal(await evaluate(`__butterCommands.some(command=>command.endsWith(' notify-test'))`),true);
  assert.equal(await evaluate(`document.querySelector('#audio-controls').textContent.includes('Prefer phone microphone')`),true);
  assert.equal(await evaluate(`document.querySelector('#audio-controls input[role=switch]').checked`),false);
  await evaluate(`document.querySelector('#audio-controls input').click()`);
  assert.equal(await evaluate(`document.querySelector('#audio-controls input').checked && document.querySelector('#audio-controls input').disabled && document.querySelector('#audio-controls .caption').textContent==='Turning on…'`),true);
  await until(`document.querySelector('#audio-controls input').checked && !document.querySelector('#audio-controls input').disabled && document.body.getAttribute('aria-busy')==='false'`);
  await evaluate(`document.querySelector('#audio-controls input').click()`);
  await until(`!document.querySelector('#audio-controls input').checked && !document.querySelector('#audio-controls input').disabled && document.body.getAttribute('aria-busy')==='false'`);
  await evaluate(`document.querySelector('#audio-controls input').click()`);
  await until(`document.querySelector('#audio-controls input').checked && !document.querySelector('#audio-controls input').disabled && document.body.getAttribute('aria-busy')==='false'`);
  await evaluate(`globalThis.__failMic=true;document.querySelector('#audio-controls input').click()`);
  assert.equal(await evaluate(`!document.querySelector('#audio-controls input').checked && document.querySelector('#audio-controls .caption').textContent==='Turning off…'`),true);
  await until(`document.querySelector('#audio-controls input').checked && !document.querySelector('#audio-controls input').disabled && document.body.getAttribute('aria-busy')==='false'`);
  assert.equal(await evaluate(`document.querySelector('#feedback').textContent.includes('Example routing rejection')`),true);
  await evaluate(`globalThis.__failMic=false`);
  assert.equal(await evaluate(`__micPaints.length>=4 && __micPaints.every(Boolean)`),true,'Mic requests entered the root bridge before a pending frame could paint');
  await evaluate(`document.querySelector('#advanced-controls').closest('details').open=true`);
  assert.equal(await evaluate(`Array.from(document.querySelectorAll('#advanced-controls > .control-switch')).every(row=>{const copy=row.querySelector('.control-copy').getBoundingClientRect(), control=row.querySelector('.switch').getBoundingClientRect();return copy.right+15<=control.left && row.querySelector('.caption')?.parentElement.classList.contains('control-copy');})`),true);
  await evaluate(`document.querySelector('#advanced-controls').closest('details').open=false`);
  assert.equal(await evaluate(`document.querySelector('#audio-controls').textContent.includes('Use previous preferences')`),false);
  await evaluate(`globalThis.__butterFixture.audio.bluetoothConnected=false;globalThis.__butterFixture.audio.active=false;document.querySelector('#refresh').click()`);
  await until(`document.querySelector('#audio-controls .connection-notice')?.textContent==='Bluetooth earbuds disconnected'`);
  assert.equal(await evaluate(`document.querySelector('#audio-controls input').checked && !document.querySelector('#audio-controls input').disabled`),true);
  assert.equal(await evaluate(`document.querySelector('#audio-controls').textContent.includes('automatically when Bluetooth earbuds reconnect')`),true);

  await evaluate(`location.hash='apps';document.querySelector('#running-panel').open=true`);
  await until(`document.querySelectorAll('.running-row').length===2`);
  assert.equal(await evaluate(`document.querySelectorAll('.running-row input:disabled').length`),1);
  assert.equal(await evaluate(`document.querySelector('#running-list').textContent.includes('12.5% CPU') && document.querySelector('#running-list').textContent.includes('512 MiB RSS')`),true);
  await evaluate(`document.querySelector('.running-row input:not(:disabled)').click();document.querySelector('#stop-apps').click()`);
  await until(`document.querySelector('#stop-apps-dialog').open`);
  assert.equal(await evaluate(`__butterCommands.some(command=>command.includes(' activity stop '))`),false);
  await evaluate(`document.querySelector('#confirm-stop-apps').click()`);
  await until(`document.querySelector('#stop-results').textContent.includes('stopped')`);
  assert.equal(await evaluate(`__butterCommands.some(command=>command.endsWith(' activity stop --confirm com.example.game'))`),true);
  await evaluate(`document.querySelector('#running-panel').open=false;location.hash='overview';document.querySelector('#battery-usage-panel').open=true`);
  await until(`document.querySelector('#battery-usage-details').textContent.includes('500.0 mAh/h')`);
  assert.equal(await evaluate(`document.querySelector('#battery-window').textContent.includes('2026-10-24')`),true);
  await evaluate(`location.hash='settings'`);
  for(const theme of ['light','dark']){
    await evaluate(`document.documentElement.dataset.theme='${theme}'`);
    assert.equal(await evaluate(`[...document.querySelectorAll('#page-settings>.control-switch')].every(row=>{const copy=row.querySelector('.control-copy').getBoundingClientRect(),rect=row.getBoundingClientRect(),toggle=row.querySelector('.switch').getBoundingClientRect();return copy.bottom+16<=rect.bottom && copy.right+15<=toggle.left;})`),true,'Setting dividers overlap copy in '+theme);
  }
  assert.equal(await evaluate(`document.querySelector('#preferences-status').textContent.includes('remembered after reboot')`),true);
  assert.equal(await evaluate(`document.querySelector('#cpu-controls > details').open`),false);
  await evaluate(`document.querySelector('#cpu-controls > details').open=true`);
  assert.equal(await evaluate(`document.querySelector('[data-select=maximum-policy0]').textContent.includes('GHz')`),true);
  assert.match(await evaluate(`document.querySelector('#cpu-controls .preset-request').textContent`),/Preset request: 1 GHz – 2 GHz/);
  await evaluate(`globalThis.__butterFixture.cpuPolicies[0].requestedMin='1100000';document.querySelector('#refresh').click()`);
  await until(`document.querySelector('#cpu-controls .preset-request').textContent.includes('1.1 GHz') && document.querySelector('#refresh').disabled===false`);
  assert.equal(await evaluate(`document.querySelector('#cpu-controls > details').open`),true,'Live readings collapsed the expanded cluster controls');
  await evaluate(`document.querySelector('#cpu-controls > details').open=false;globalThis.__butterFixture.cpuPolicies[0].requestedMin='1000000';document.querySelector('#refresh').click()`);
  await until(`document.querySelector('#cpu-controls .preset-request').textContent.includes('1 GHz') && document.querySelector('#refresh').disabled===false`);
  assert.equal(await evaluate(`document.querySelector('#cpu-controls > details').open`),false,'Live readings reopened manually collapsed cluster controls');
  await evaluate(`document.querySelector('#cpu-controls > details').open=true`);

  assert.equal(await evaluate(`document.querySelector('#cpu-controls').closest('details').querySelector('summary').textContent.includes('−')`),false);
  const arrowX = await evaluate(`document.querySelector('#cpu-controls').closest('details').querySelector('summary>.ph-icon').getBoundingClientRect().right`);
  assert.equal(await evaluate(`getComputedStyle(document.querySelector('#cpu-controls').closest('details').querySelector('summary'),'::after').content`),'none');
  await evaluate(`document.querySelector('#cpu-controls').closest('details').open=false`); await delay(250);
  const closedArrowX = await evaluate(`document.querySelector('#cpu-controls').closest('details').querySelector('summary>.ph-icon').getBoundingClientRect().right`);
  assert.ok(Math.abs(arrowX-closedArrowX)<1,'Disclosure arrow moved sideways when opened');
  await screenshot('webui-settings-dark.png');
  assert.equal(await evaluate(`document.documentElement.scrollWidth<=innerWidth`),true,'Mobile settings overflow horizontally');
  await evaluate(`document.querySelector('[data-page=apps]').click()`); await until(`!document.getElementById('page-apps').hidden`);
  await screenshot('webui-apps-dark.png');
  await evaluate(`__butterFixture.supervisor.health={running:false,error:'Example startup failure'};__butterFixture.supervisor.runtime.error='Example startup failure';document.getElementById('refresh').click()`);
  await until(`document.getElementById('engine-state').textContent==='Needs attention'`);
  assert.match(await evaluate(`document.getElementById('engine-reason').textContent`),/Example startup failure/);
  assert.deepEqual(exceptions,[],'Browser JavaScript threw an exception');
  console.log('WebUI smoke passed: monitoring startup, signed watts, preset modal, app discovery, selectors, app rule, theme, root error state and mobile overflow.');
} finally {
  socket?.close();chrome.kill('SIGTERM');server.close();
  await new Promise(resolve=>{if(chrome.exitCode!==null)resolve();else{chrome.once('exit',resolve);setTimeout(()=>{chrome.kill('SIGKILL');resolve();},3000).unref();}});
  await fs.rm(directory,{recursive:true,force:true,maxRetries:5,retryDelay:200});
}
