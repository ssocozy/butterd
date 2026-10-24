import {$,paragraph} from './ui.js';
import {clusterNames,formatClock} from './controls.js';
function group(host,title) {
  const section=document.createElement('section'); section.className='hardware-group';
  const heading=document.createElement('h3'); heading.textContent=title;
  const list=document.createElement('dl'); list.className='detail-list'; section.append(heading,list);host.append(section);return list;
}
function row(host,title,value) {
  if(value==null || value==='') return;
  const entry=document.createElement('div'),label=document.createElement('dt'),detail=document.createElement('dd');label.textContent=title;detail.textContent=value;entry.append(label,detail);host.append(entry);
}
export function renderHardware(state) {
  const host=$('hardware-details');host.replaceChildren();
  const policies=state.cpuPolicies || [],names=clusterNames(policies);
  if(!policies.length)host.append(paragraph('CPU details appear when exposed by the kernel.'));
  for(const policy of policies) {
    const cluster=names.get(policy.id),list=group(host,cluster.name);
    row(list,'Current clock',Number(policy.current)>0?formatClock(policy.current):null);
    if(Number(policy.requestedMin)>0)row(list,'Preset request',formatClock(policy.requestedMin)+' – '+formatClock(policy.requestedMax));
    row(list,'Cores',String(cluster.cores));row(list,'Governor',policy.governor);
    if(Number(policy.min)>0 && Number(policy.max)>0)row(list,'Effective clock range',formatClock(policy.min)+' – '+formatClock(policy.max));
    if(cluster.peak>0)row(list,'Supported maximum',formatClock(cluster.peak));
    row(list,'Available governors',(policy.choices || []).join(', '));
  }
  const gpu=state.gpuRange || {},graphics=group(host,'Graphics');row(graphics,'GPU','Mali');
  row(graphics,'Current clock',Number(gpu.current)>0?formatClock(gpu.current,true):null);
  if(Number(gpu.requestedMin)>0)row(graphics,'Preset request',formatClock(gpu.requestedMin,true)+(Number(gpu.requestedMax)>0?' – '+formatClock(gpu.requestedMax,true):' minimum'));
  if(Number(gpu.min)>0 && Number(gpu.max)>0)row(graphics,'Effective clock range',formatClock(gpu.min,true)+' – '+formatClock(gpu.max,true));
  else if(Number(gpu.max)>0)row(graphics,'Effective ceiling',formatClock(gpu.max,true));
  else row(graphics,'Clock range','Provided when exposed by the driver');
  const touch=state.touch || {},input=group(host,'Touch');row(input,'Control',touch.available ? 'Supported' : 'Managed by device firmware');
  row(input,'Sampling rate',touch.driverHz ? touch.driverHz+' Hz' : 'Driver does not report an exact rate');
  row(input,'Request',touch.state);if(touch.controller)row(input,'Controller',touch.controller);
  const tcp=state.tcp || {},network=group(host,'Network');
  row(network,'TCP congestion control',tcp.current || 'Provided when exposed by the kernel');row(network,'Available algorithms',(tcp.choices || []).join(', '));
}
