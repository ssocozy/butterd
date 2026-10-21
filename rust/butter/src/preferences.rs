//! Saved user intent is separate from per-boot ownership journals.
use crate::{engine::Engine, hardware::Hardware, journal::Store, model::{valid_policy,Feature}, runtime::{Context,RuntimeState}, Result};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedControl { pub setting: String, pub expires_utc: Option<u64> }
pub fn feature(key: &str) -> Result<Feature> {
    let value = if let Some(policy) = key.strip_prefix("governor-").filter(|policy| valid_policy(policy)) { Feature::Governor(policy.into()) }
        else if let Some(policy) = key.strip_prefix("range-").filter(|policy| valid_policy(policy)) { Feature::CpuRange(policy.into()) }
        else { Feature::parse(key)? };
    if matches!(value,Feature::Profile|Feature::Refresh(_)) { return Err("Managed profiles cannot be saved as manual controls".into()); }
    Ok(value)
}
pub fn duration(feature: &Feature) -> u64 { if matches!(feature,Feature::CpuLock|Feature::GpuLock|Feature::GpuThermal) { 60 } else { 1800 } }
pub fn validate(controls: &BTreeMap<String,SavedControl>) -> Result<()> {
    if controls.len()>32 { return Err("Too many saved controls".into()); }
    for (key,saved) in controls {
        let feature = feature(key)?;
        if saved.setting.len()>64 || saved.expires_utc.is_some() != feature.advanced() { return Err("Invalid saved control".into()); }
    }
    Ok(())
}
pub fn remaining(saved: &SavedControl, feature: &Feature, epoch: u64) -> Option<u64> {
    if epoch < 1_735_689_600 { return None; }
    saved.expires_utc?.checked_sub(epoch).filter(|seconds| *seconds>0 && *seconds<=duration(feature))
}
fn ready(feature: &Feature, ctx: &Context) -> bool {
    if !feature.advanced() { return true; }
    if ctx.temperature.is_none_or(|value| !(0..400).contains(&value)) || ctx.battery.is_none_or(|value| value<=20) || ctx.saver!=Some(false) { return false; }
    if matches!(feature,Feature::ChargingPause|Feature::ChargingThermal|Feature::Bypass) { ctx.online==Some(true) }
    else { ctx.awake==Some(true) && ctx.soc_millic.is_some_and(|value| value<75000) }
}
pub fn resume<H:Hardware,S:Store>(engine: &Engine<H,S>, controls: &BTreeMap<String,SavedControl>, state: &mut RuntimeState, ctx: &Context, epoch: u64, now: u64) {
    if !controls.contains_key("phone-mic") && engine.session(&Feature::PhoneMic).ok().flatten().is_some() {
        if let Err(error) = engine.apply(Feature::PhoneMic,"",true,false) { state.preference_errors.insert("phone-mic".into(),error.to_string()); }
    }
    for (key,saved) in controls {
        let result = (|| -> Result<()> {
            let feature = feature(key)?;
            if feature.advanced() && remaining(saved,&feature,epoch).is_none() {
                engine.apply(feature.clone(),"",true,false)?;
                state.advanced_deadlines.remove(key); state.resumed_controls.insert(key.clone()); return Ok(());
            }
            if feature == Feature::PhoneMic && !state.preference_errors.contains_key(key) {
                if now.saturating_sub(state.audio_checked_at)<5 { return Ok(()); }
                state.audio_checked_at=now;
                let audio = crate::audio::status(&engine.hardware)?;
                if audio["bluetoothConnected"] != true {
                    engine.apply(feature,"",true,false)?; state.resumed_controls.remove(key); return Ok(());
                }
                if engine.session(&feature)?.is_some() && engine.state(&feature)? != "Request active" {
                    engine.apply(feature,"",true,false)?;
                    return Err("Microphone preferences changed externally; enable again to retry".into());
                }
            }
            if state.resumed_controls.contains(key) || state.preference_errors.contains_key(key) || !ready(&feature,ctx) { return Ok(()); }
            let other = match feature { Feature::ChargingPause=>Some(Feature::Bypass),Feature::Bypass=>Some(Feature::ChargingPause),_=>None };
            if other.is_some_and(|other| engine.session(&other).ok().flatten().is_some()) { return Err("Another charging mode is already active".into()); }
            if feature == Feature::ChargingPause && engine.session(&feature)?.is_none() && engine.hardware.read(crate::hardware::PAUSE)?.split_whitespace().nth(1)!=Some("0") { return Err("Charging is already paused outside butter".into()); }
            if feature.advanced() { state.advanced_deadlines.insert(key.clone(),now+remaining(saved,&feature,epoch).ok_or("Override expired")?); }
            state.resumed_controls.insert(key.clone());
            match engine.apply(feature.clone(),&saved.setting,false,feature.advanced()) {
                Ok(_) => {
                    if feature == Feature::Bypass && engine.hardware.read(crate::hardware::BYPASS_ACTIVE)? != "1" { engine.apply(feature,"",true,false)?; return Err("Kernel did not confirm battery bypass".into()); }
                    Ok(())
                }
                Err(error) => { let _ = engine.apply(feature,"",true,false); Err(error) }
            }
        })();
        if let Err(error) = result {
            let cleanup = if key == "phone-mic" { engine.apply(Feature::PhoneMic,"",true,false).err().map(|error| error.to_string()) } else { None };
            state.preference_errors.insert(key.clone(),format!("{error}{}",cleanup.map(|error| format!("; cleanup: {error}")).unwrap_or_default()));
        }
    }
}
