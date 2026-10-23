//! Optional, coalesced Android status notices. Never runs under the control lock.
use crate::{android,presets::Mode,runtime::RuntimeState,Result};
#[derive(Default)]
pub struct Notices { last:Option<String>, attempted_at:Option<u64> }
impl Notices {
    pub fn next(&mut self, message:&str, now:u64)->bool {
        if self.last.as_deref()==Some(message) || self.attempted_at.is_some_and(|last|now.saturating_sub(last)<15) { return false; }
        self.last=Some(message.into()); self.attempted_at=Some(now); true
    }
}
pub fn message(state:&RuntimeState)->Option<String> {
    if state.error.is_some() || state.blocked { return Some("Tuning paused. Open Butterd for details.".into()); }
    if !state.preference_errors.is_empty() { return Some("Some saved settings need attention. Open Butterd for details.".into()); }
    if state.reason=="Cooling down" { return Some("Performance limits released while the device cools.".into()); }
    if matches!(state.reason.as_str(),"Foreground app unknown or multiple resumed apps"|"Screen off or display state unknown") || state.active==Mode::Boost { return None; }
    let mode=match state.active { Mode::Balanced=>"Balanced",Mode::Battery=>"Saver",Mode::Performance=>"Performance",Mode::Boost=>return None };
    let suffix=if state.reason=="Foreground app rule" { "app preset applied" } else if state.reason=="Monitoring only" { return Some("Monitoring active. No performance preset requested.".into()); } else { "preset active" };
    Some(format!("{mode} {suffix}."))
}
pub fn post(text:&str)->Result<()> {
    // A fixed tag updates one notification rather than accumulating game alerts.
    android::command("/system/bin/cmd",&["notification","post","-t","Butterd","butterd-engine",text])?;
    Ok(())
}
