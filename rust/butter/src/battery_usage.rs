//! Android's historical battery window, not instantaneous per-app power.
use crate::{android,Result};
use serde_json::{json,Value};
pub fn duration(raw:&str)->Option<f64>{
    let mut seconds=0.0;let mut found=false;
    for token in raw.split_whitespace(){
        let (number,multiplier)=if let Some(v)=token.strip_suffix("ms"){(v,0.001)}else if let Some(v)=token.strip_suffix('s'){(v,1.0)}else if let Some(v)=token.strip_suffix('m'){(v,60.0)}else if let Some(v)=token.strip_suffix('h'){(v,3600.0)}else if let Some(v)=token.strip_suffix('d'){(v,86400.0)}else{continue};
        let value=number.parse::<f64>().ok()?;if !value.is_finite()||value<0.0{return None;}seconds+=value*multiplier;found=true;
    }
    if found && seconds<=366.0*86400.0 {Some(seconds)}else{None}
}
pub fn parse(text:&str)->Result<Value>{
    if !text.contains("Statistics since last charge:"){return Err("Battery statistics window unavailable".into());}
    let line=|prefix:&str|text.lines().map(str::trim).find_map(|line|line.strip_prefix(prefix));
    let numeric=|prefix:&str|line(prefix).and_then(|value|value.split_whitespace().next()?.parse::<f64>().ok()).filter(|v|v.is_finite()&&*v>=0.0);
    let times=|prefix:&str|->Option<(f64,f64)>{let raw=line(prefix)?;let (real,awake)=raw.split_once("realtime,")?;Some((duration(real.split('(').next()?)?,duration(awake.split('(').next()?)?))};
    let total=times("Time on battery:").ok_or("Battery duration unavailable")?;
    let off=times("Time on battery screen off:").ok_or("Screen-off duration unavailable")?;
    if off.0>total.0 || total.1>total.0 || off.1>off.0{return Err("Inconsistent battery durations".into());}
    let on_seconds=total.0-off.0;
    let rate=|amount:Option<f64>,seconds:f64|if seconds>=900.0{amount.map(|value|value*3600.0/seconds)}else{None};
    let sleep=total.0-total.1;
    Ok(json!({"window":"Android statistics since last charge","start":line("Start clock time:").map(str::trim),
        "onBatterySeconds":total.0,"screenOnSeconds":on_seconds,"screenOffSeconds":off.0,"awakeSeconds":total.1,
        "sleepEstimateSeconds":sleep,"sleepEstimatePercent":if total.0>=900.0{Some(sleep*100.0/total.0)}else{None},
        "dischargeMah":numeric("Discharge:"),"screenOnMah":numeric("Screen on discharge:"),"screenOffMah":numeric("Screen off discharge:"),
        "screenOnMahPerHour":rate(numeric("Screen on discharge:"),on_seconds),"screenOffMahPerHour":rate(numeric("Screen off discharge:"),off.0),
        "screenOnPercentPerHour":rate(numeric("Amount discharged while screen on:"),on_seconds),"screenOffPercentPerHour":rate(numeric("Amount discharged while screen off:"),off.0),
        "note":"Historical estimates, not a live power reading. Rates require 15 minutes in that state. Sleep is elapsed minus awake uptime; its energy use is not separately measured. Charging and bypass can make fuel-gauge estimates unreliable."}))
}
pub fn read()->Result<Value>{parse(&android::execute("/system/bin/dumpsys",&["batterystats","--charged"],&[],8)?)}
