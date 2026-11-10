//! Optional, coalesced Android status notices. Never runs under the control lock.
use crate::{android,presets::Mode,runtime::RuntimeState,Result};
#[derive(Default)]
pub struct Notices { last:Option<String>, attempted_at:Option<u64>, charging:Option<bool>, metrics_visible:bool, status:Option<String>, title:Option<String> }
impl Notices {
    pub fn update(&mut self, state:&RuntimeState, now:u64)->Option<String> {
        let charging=state.app_controls.charging_owned || state.advanced_deadlines.contains_key("charging-pause") || state.advanced_deadlines.contains_key("bypass");
        let changed=self.charging.is_some_and(|previous|previous!=charging);
        let warning=state.error.is_some() || state.blocked || !state.app_controls.errors.is_empty() || !state.preference_errors.is_empty() || state.gpu_profile_paused;
        let status=if warning { message(state) }
            else if changed || (self.charging.is_none() && charging) { Some(if charging {"Bypass charging active."}else{"Bypass charging stopped."}.into()) }
            else if charging {Some("Bypass charging active.".into())}
            else {message(state)};
        let metrics=charging_metrics(state);
        let status=status.or_else(||(metrics.is_some() || self.metrics_visible).then(||fallback_status(state)))?;
        let title=title(state);
        let status_changed=self.status.as_deref()!=Some(&status) || self.title.as_deref()!=Some(&title);
        let text=metrics.as_ref().map(|metrics|if warning {format!("{status}\n{metrics}")}else{format!("{metrics}\n{status}")}).unwrap_or_else(||status.clone());
        if self.metrics_visible && metrics.is_none() {
            // Remove charging-only readings immediately when charging stops.
            self.last=Some(format!("{title}\n{text}")); self.attempted_at=Some(now);
        } else {
            if !status_changed && self.attempted_at.is_some_and(|last|now.saturating_sub(last)<30) {return None;}
            if !self.next(&format!("{title}\n{text}"),now) {return None;}
        }
        self.charging=Some(charging); self.metrics_visible=metrics.is_some();
        self.status=Some(status); self.title=Some(title);
        Some(text)
    }
    pub fn next(&mut self, message:&str, now:u64)->bool {
        if self.last.as_deref()==Some(message) || self.attempted_at.is_some_and(|last|now.saturating_sub(last)<15) { return false; }
        self.last=Some(message.into()); self.attempted_at=Some(now); true
    }
}
pub fn message(state:&RuntimeState)->Option<String> {
    if state.error.is_some() || state.blocked { return Some("Tuning paused. Open Butterd for details.".into()); }
    if !state.app_controls.errors.is_empty() { return Some("App controls need attention. Open Butterd for details.".into()); }
    if !state.preference_errors.is_empty() { return Some("Some saved settings need attention. Open Butterd for details.".into()); }
    if state.gpu_profile_paused { return Some("GPU tuning paused after a ROM conflict. Other controls continue.".into()); }
    if state.reason=="Screen off" { return None; }
    if state.reason=="Cooling down" {
        let requested=match state.telemetry["requestedPreset"].as_str() { Some("aggressive")=>"Aggressive",Some("performance")=>"Performance",Some("battery")=>"Saver",_=>return None };
        return Some(if requested=="Saver" { "Saver active. Cooling; power limits retained.".into() } else { format!("{requested} requested. Cooling; boost released.") });
    }
    if state.reason.starts_with("Cooling:") {
        let mode=mode_name(state.telemetry["requestedPreset"].as_str()).unwrap_or_else(||active_name(state));
        return Some(format!("{mode} requested. Cooling; boost reduced."));
    }
    if state.reason.starts_with("Recording assist:") || state.reason=="Idle workload; frequency floors released" { return None; }
    if matches!(state.reason.as_str(),"Foreground app unknown or multiple resumed apps"|"Screen off or display state unknown") || state.active==Mode::Boost { return None; }
    let mode=match state.active { Mode::Balanced=>"Balanced",Mode::Battery=>"Saver",Mode::Performance=>"Performance",Mode::Aggressive=>"Aggressive",Mode::Boost=>return None };
    let suffix=if state.reason=="Foreground app rule" { "app preset applied" } else if state.reason=="Monitoring only" { return Some("Monitoring active. No performance preset requested.".into()); } else { "preset active" };
    Some(format!("{mode} {suffix}."))
}
fn mode_name(mode:Option<&str>)->Option<&'static str> {
    match mode {Some("aggressive")=>Some("Aggressive"),Some("performance"|"boost")=>Some("Performance"),Some("battery")=>Some("Saver"),Some("balanced")=>Some("Balanced"),_=>None}
}
fn active_name(state:&RuntimeState)->&'static str {
    match state.active {Mode::Aggressive=>"Aggressive",Mode::Performance|Mode::Boost=>"Performance",Mode::Battery=>"Saver",Mode::Balanced=>"Balanced"}
}
pub fn title(state:&RuntimeState)->String {
    let cooling=state.reason=="Cooling down" || state.reason.starts_with("Cooling:");
    let mode=if cooling || state.blocked {mode_name(state.telemetry["requestedPreset"].as_str()).unwrap_or_else(||active_name(state))}else{active_name(state)};
    let suffix=if state.blocked || state.error.is_some() {" — Paused"}else if cooling {" — Cooling"}else{""};
    format!("Butterd · {mode}{suffix}")
}
fn fallback_status(state:&RuntimeState)->String {
    if state.reason=="Screen off" {"Screen off. Boost requests released.".into()}
    else if state.active==Mode::Boost {"Temporary performance boost active.".into()}
    else if state.reason.starts_with("Recording assist:") {"Recording assist active.".into()}
    else {format!("{} preset active.",active_name(state))}
}
pub fn charging_metrics(state:&RuntimeState)->Option<String> {
    let t=&state.telemetry;
    if t["batteryStatus"].as_str()!=Some("Charging") || t["usbOnline"].as_bool()==Some(false) {return None;}
    let temp=t["batteryTempC"].as_f64().filter(|v|v.is_finite() && (-40.0..=125.0).contains(v)).map(|v|format!("{v:.1}°C")).unwrap_or_else(||"—".into());
    let watts=t["batteryWatts"].as_f64().filter(|v|v.is_finite() && (0.0..=500.0).contains(v)).map(|v|format!("{v:.1} W")).unwrap_or_else(||"—".into());
    let percent=t["batteryPercent"].as_u64().filter(|v|*v<=100).map(|v|format!("{v}%")).unwrap_or_else(||"—".into());
    Some(format!("{temp} · Charging {watts} · {percent}"))
}
pub fn confirm(reply:&str)->Result<()> {
    if !reply.lines().any(|line|line.trim()=="posted:") || reply.contains("couldn't find notification") { return Err("Android did not confirm the notification. Check Shell notification permissions and its notification channel.".into()); }
    Ok(())
}
pub fn post(text:&str)->Result<()> { post_with_title("Butterd",text) }
pub fn post_status(state:&RuntimeState,text:&str)->Result<()> { post_with_title(&title(state),text) }
fn post_with_title(title:&str,text:&str)->Result<()> {
    let result=android::execute("/system/bin/app_process",&["/system/bin","dev.butterd.module.EngineNotification",title,text],&[("CLASSPATH","/data/adb/modules/butterd/bin/catalog.dex")],8).and_then(|reply|confirm(&reply));
    crate::runtime::record_notification(&result);
    result
}
