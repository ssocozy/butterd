import {exec,binary,validPackage} from './bridge.js';
import {$,notify} from './ui.js';
let rows=[],selected=new Set(),busy=false,pending=[];
const section=$('running-panel'), list=$('running-list');
const paint=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
function visible(){return section.open && !$('page-apps').hidden && !document.hidden;}
function render(){
  list.replaceChildren();
  for(const app of rows){
    const row=document.createElement('label');row.className='running-row';
    const box=document.createElement('input');box.type='checkbox';box.checked=selected.has(app.package);box.disabled=busy||!app.canStop;box.setAttribute('aria-label','Select '+app.label);
    box.addEventListener('change',()=>{if(box.checked){if(selected.size>=16){box.checked=false;notify('Select up to 16 apps at a time.',true);return;}selected.add(app.package);}else selected.delete(app.package);updateButton();});
    const copy=document.createElement('div'),name=document.createElement('strong'),state=document.createElement('p');name.textContent=app.label||app.package;state.className='caption';
    state.textContent=(app.foreground?'Foreground':'Background / cached')+(app.canStop?'':' · '+(app.protectedReason||'Protected'));
    const metrics=document.createElement('div');metrics.className='running-metrics';
    const cpu=document.createElement('span'),ram=document.createElement('span');cpu.textContent=Number.isFinite(app.cpuPercent)?app.cpuPercent.toFixed(1)+'% CPU':'CPU not reported';ram.textContent=Number.isFinite(app.rssKb)?(app.rssKb/1024).toFixed(0)+' MiB RSS':'RAM not reported';metrics.append(cpu,ram);
    copy.append(name,state,metrics);row.append(box,copy);list.append(row);
  }
  updateButton();
}
function updateButton(){$('stop-apps').disabled=busy||selected.size===0;$('stop-apps').textContent=selected.size?'Force stop '+selected.size+' selected':'Force stop selected';}
async function load(){
  if(busy)return;busy=true;$('refresh-running').disabled=true;updateButton();
  $('running-status').textContent='Sampling CPU and reading resident memory…';
  render();
  await paint();
  try{
    const result=JSON.parse(await exec(binary+' activity'));
    if(!Array.isArray(result.apps))throw new Error('Running app list was not returned.');
    rows=result.apps.filter(app=>validPackage(app.package));
    selected=new Set([...selected].filter(pkg=>rows.some(app=>app.package===pkg&&app.canStop)));
    $('running-status').textContent=rows.length+' apps · '+Number(result.sampleSeconds||1).toFixed(1)+'s CPU sample. 100% CPU ≈ one core. RSS includes shared pages.';
  }catch(error){$('running-status').textContent='App activity could not be read: '+error.message;}
  finally{busy=false;$('refresh-running').disabled=false;render();}
}
$('refresh-running').addEventListener('click',load);
section.addEventListener('toggle',()=>{if(visible()&&!rows.length)load();});
window.addEventListener('hashchange',()=>{if(visible()&&!rows.length)load();});
setInterval(()=>{if(visible()&&!$('stop-apps-dialog').open)load();},15000);
$('stop-apps').addEventListener('click',()=>{
  if(busy||!selected.size)return;pending=rows.filter(app=>selected.has(app.package)&&app.canStop).slice(0,16);
  const names=$('stop-app-names');names.replaceChildren();
  for(const app of pending){const item=document.createElement('li');item.textContent=app.label||app.package;names.append(item);}
  $('stop-apps-dialog').showModal();
});
$('confirm-stop-apps').addEventListener('click',async()=>{
  if(busy||!pending.length)return;
  const packages=pending.map(app=>app.package).filter(validPackage);pending=[];$('stop-apps-dialog').close();
  busy=true;render();$('refresh-running').disabled=true;$('running-status').textContent='Stopping selected apps…';
  await paint();
  try{
    const response=JSON.parse(await exec(binary+' activity stop --confirm '+packages.join(' ')));
    const results=$('stop-results');results.replaceChildren();
    for(const result of response.results||[]){const item=document.createElement('p');item.className='caption';const app=rows.find(row=>row.package===result.package);item.textContent=(app?.label||result.package)+': '+result.result+(result.detail?' · '+result.detail:'');results.append(item);}
    selected.clear();notify('Force-stop results are shown below the app list.');
  }catch(error){notify('Could not finish force-stop: '+error.message,true);}
  finally{busy=false;$('refresh-running').disabled=false;await load();}
});

function batteryStartLabel(value){
  const match=typeof value==='string' && value.match(/^(\d{4})-(\d{2})-(\d{2})[ T-](\d{2})[:-](\d{2})(?:[:-](\d{2}))?$/);
  if(!match)return 'Android’s last charge statistics reset';
  const [,year,month,day,hour,minute,second='0']=match;
  const date=new Date(Number(year),Number(month)-1,Number(day),Number(hour),Number(minute),Number(second));
  if(date.getFullYear()!==Number(year)||date.getMonth()!==Number(month)-1||date.getDate()!==Number(day)||date.getHours()!==Number(hour)||date.getMinutes()!==Number(minute)||date.getSeconds()!==Number(second))return 'Android’s last charge statistics reset';
  return new Intl.DateTimeFormat(undefined,{day:'numeric',month:'short',year:'numeric',hour:'numeric',minute:'2-digit'}).format(date);
}
let batteryBusy=false,batteryLoaded=false;
async function loadBattery(){
  if(batteryBusy)return;batteryBusy=true;$('refresh-battery-usage').disabled=true;
  $('battery-usage-note').textContent='Reading Android battery statistics…';
  await paint();
  try{
    const data=JSON.parse(await exec(binary+' battery-usage')),host=$('battery-usage-details');host.replaceChildren();
    const hours=value=>Number.isFinite(value)?(value/3600).toFixed(1)+' h':null;
    const rate=(value,unit)=>Number.isFinite(value)?value.toFixed(1)+' '+unit:'Collecting data (15 min minimum)';
    const rows=[['Screen on',hours(data.screenOnSeconds)],['Screen-on drain',rate(data.screenOnMahPerHour,'mAh/h')],['Screen-on battery',rate(data.screenOnPercentPerHour,'%/h')],['Screen off / idle',hours(data.screenOffSeconds)],['Screen-off drain',rate(data.screenOffMahPerHour,'mAh/h')],['Screen-off battery',rate(data.screenOffPercentPerHour,'%/h')],['Awake on battery',hours(data.awakeSeconds)],['Sleep estimate',hours(data.sleepEstimateSeconds)],['Time in deep sleep (estimated)',Number.isFinite(data.sleepEstimatePercent)?data.sleepEstimatePercent.toFixed(1)+'%':null]];
    for(const [title,value] of rows){if(value==null)continue;const row=document.createElement('div'),name=document.createElement('dt'),reading=document.createElement('dd');name.textContent=title;reading.textContent=value;row.append(name,reading);host.append(row);}
    $('battery-window').textContent='Since '+batteryStartLabel(data.start);$('battery-usage-note').textContent=data.note;batteryLoaded=true;
  }catch(error){$('battery-usage-note').textContent='Battery history could not be read: '+error.message;}
  finally{batteryBusy=false;$('refresh-battery-usage').disabled=false;}
}
$('refresh-battery-usage').addEventListener('click',loadBattery);
$('battery-usage-panel').addEventListener('toggle',()=>{if($('battery-usage-panel').open&&!batteryLoaded)loadBattery();});
