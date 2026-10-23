//! Foreground-scoped touch and charging leases, separate from manual saved controls.
use crate::{engine::Engine,hardware::Hardware,journal::Store,model::Feature,runtime::{Config,Context},Result};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
#[derive(Debug,Default,Clone,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct State {
    pub key:Option<String>,pub touch_owned:bool,pub charging_owned:bool,
    pub touch_attempted:bool,pub charging_attempted:bool,pub errors:BTreeMap<String,String>,
}
fn release<H:Hardware,S:Store>(e:&Engine<H,S>,cfg:&Config,feature:Feature,owned:&mut bool)->Result<()> {
    if !*owned { return Ok(()); }
    if !cfg.controls.contains_key(&feature.key()) { e.apply(feature,"",true,false)?; }
    *owned=false;Ok(())
}
fn claim<H:Hardware,S:Store>(e:&Engine<H,S>,cfg:&Config,feature:Feature,owned:&mut bool,attempted:&mut bool)->Result<()> {
    if cfg.controls.contains_key(&feature.key()) { *owned=false;*attempted=true;return Ok(()); }
    if *owned {
        if e.session(&feature)?.is_none() { *owned=false;return Err("Request released manually; waiting until the next app session".into()); }
        if e.state(&feature)? != "Request active" {
            release(e,cfg,feature,owned)?;
            return Err("Control changed externally; waiting until the next app session".into());
        }
        return Ok(());
    }
    if *attempted { return Ok(()); }
    *attempted=true;
    if e.session(&feature)?.is_some() { return Err("An existing manual session takes priority".into()); }
    if feature==Feature::ChargingPause && (e.session(&Feature::Bypass)?.is_some() || e.hardware.read(crate::hardware::PAUSE)?.split_whitespace().nth(1)!=Some("0")) { return Err("Charging is already controlled outside this app rule".into()); }
    let result=e.apply(feature.clone(),if feature==Feature::Touch { "high" } else { "" },false,feature.advanced());
    if result.is_err() {
        let cleanup=e.apply(feature.clone(),"",true,false);
        *owned=e.session(&feature)?.is_some();
        if let Err(error)=cleanup { return Err(format!("App control failed; cleanup needs attention: {error}").into()); }
    } else { *owned=true; }
    result.map(|_|())
}
pub fn reconcile<H:Hardware,S:Store>(e:&Engine<H,S>,cfg:&Config,state:&mut State,ctx:&Context) {
    // Manual selections take over an existing lease without restoring underneath them.
    if cfg.controls.contains_key("touch") { state.touch_owned=false;state.errors.remove("touch"); }
    if cfg.controls.contains_key("charging-pause") { state.charging_owned=false;state.errors.remove("charging-pause"); }
    let rule=if cfg.automatic && ctx.awake==Some(true) { cfg.rules.iter().find(|rule|Some(&rule.package)==ctx.app.as_ref()) } else { None };
    let key=rule.map(|rule|format!("{}:{}:{:?}",rule.package,rule.high_touch,rule.charging_pause_at));
    if key!=state.key {
        let touch=release(e,cfg,Feature::Touch,&mut state.touch_owned);
        let charging=release(e,cfg,Feature::ChargingPause,&mut state.charging_owned);
        if let Err(error)=touch.and(charging) { state.errors.insert("release".into(),error.to_string());return; }
        *state=State{key,..Default::default()};
    }
    let Some(rule)=rule else { return; };
    if rule.high_touch {
        if let Err(error)=claim(e,cfg,Feature::Touch,&mut state.touch_owned,&mut state.touch_attempted) { state.errors.insert("touch".into(),error.to_string()); }
    }
    if let Some(threshold)=rule.charging_pause_at {
        let keep=ctx.online==Some(true) && ctx.battery.is_some_and(|level|if state.charging_owned { level>u64::from(threshold.saturating_sub(3)) } else { level>=u64::from(threshold) });
        if keep {
            if let Err(error)=claim(e,cfg,Feature::ChargingPause,&mut state.charging_owned,&mut state.charging_attempted) { state.errors.insert("charging-pause".into(),error.to_string()); }
        } else {
            match release(e,cfg,Feature::ChargingPause,&mut state.charging_owned) {
                Ok(())=>{state.charging_attempted=false;state.errors.remove("charging-pause");},
                Err(error)=>{state.errors.insert("charging-pause".into(),error.to_string());},
            }
        }
    }
}
