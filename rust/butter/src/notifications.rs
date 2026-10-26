//! Optional, coalesced Android status notices. Never runs under the control lock.
use crate::{android,presets::Mode,runtime::RuntimeState,Result};
#[derive(Default)]
pub struct Notices { last:Option<String>, attempted_at:Option<u64>, charging:Option<bool> }
impl Notices {
    pub fn update(&mut self, state:&RuntimeState, now:u64)->Option<String> {
        let charging=state.app_controls.charging_owned || state.advanced_deadlines.contains_key("charging-pause") || state.advanced_deadlines.contains_key("bypass");
        let changed=self.charging.is_some_and(|previous|previous!=charging);
        let text=if state.error.is_some() || state.blocked || !state.app_controls.errors.is_empty() || !state.preference_errors.is_empty() { message(state) }
            else if changed || (self.charging.is_none() && charging) { Some(if charging {"Bypass charging active."}else{"Bypass charging stopped."}.into()) }
            else if charging {Some("Bypass charging active.".into())}
            else {message(state)};
        let text=text?;
        if !self.next(&text,now) {return None;}
        self.charging=Some(charging);
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
    if state.reason=="Screen off" { return None; }
    if state.reason=="Cooling down" || state.reason.starts_with("Cooling:") || state.reason.starts_with("Recording assist:") || state.reason=="Idle workload; frequency floors released" { return None; }
    if matches!(state.reason.as_str(),"Foreground app unknown or multiple resumed apps"|"Screen off or display state unknown") || state.active==Mode::Boost { return None; }
    let mode=match state.active { Mode::Balanced=>"Balanced",Mode::Battery=>"Saver",Mode::Performance=>"Performance",Mode::Aggressive=>"Aggressive",Mode::Boost=>return None };
    let suffix=if state.reason=="Foreground app rule" { "app preset applied" } else if state.reason=="Monitoring only" { return Some("Monitoring active. No performance preset requested.".into()); } else { "preset active" };
    Some(format!("{mode} {suffix}."))
}
pub fn confirm(reply:&str)->Result<()> {
    if !reply.lines().any(|line|line.trim()=="posted:") || reply.contains("couldn't find notification") { return Err("Android did not confirm the notification. Check Shell notification permissions and its notification channel.".into()); }
    Ok(())
}
pub fn post(text:&str)->Result<()> {
    let result=android::execute("/system/bin/app_process",&["/system/bin","dev.butterd.module.EngineNotification",text],&[("CLASSPATH","/data/adb/modules/butterd/bin/catalog.dex")],8).and_then(|reply|confirm(&reply));
    crate::runtime::record_notification(&result);
    result
}
