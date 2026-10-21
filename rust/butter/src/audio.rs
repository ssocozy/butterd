//! Narrow capture-preset preferences. Output routing is never written.
use crate::{android, hardware::Hardware, Result};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
pub const STATUS: &str = "audio-status";
pub const PRESETS: [u32;3] = [1,5,7];
#[derive(Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
struct Device { #[serde(rename="type")] kind: u32, address: String }
pub fn preset(path: &str) -> Option<u32> { path.strip_prefix("audio-capture:")?.parse().ok().filter(|value| PRESETS.contains(value)) }
pub fn normalize(raw: &str) -> Result<String> {
    if raw.len() > 2048 { return Err("Audio preference exceeds its limit".into()); }
    let devices: Vec<Device> = serde_json::from_str(raw)?;
    if devices.len() > 1 || devices.iter().any(|device| !(1..=100).contains(&device.kind) || device.address.len()>256 || device.address.chars().any(char::is_control)) { return Err("Invalid input preference".into()); }
    Ok(serde_json::to_string(&devices)?)
}
fn bridge(args: &[&str]) -> Result<String> {
    let mut command = vec!["/system/bin","dev.butterd.module.AudioRouteBridge"]; command.extend_from_slice(args);
    android::execute("/system/bin/app_process",&command,&[("CLASSPATH","/data/adb/modules/butterd/bin/catalog.dex")],5)
}
pub fn read(path: &str) -> Option<Result<String>> {
    if path == STATUS { return Some(bridge(&["status"])); }
    preset(path).map(|source| bridge(&["read",&source.to_string()]).and_then(|value| normalize(&value)))
}
pub fn write(path: &str, value: &str) -> Option<Result<()>> {
    let source=preset(path)?;
    Some((|| { let value = normalize(value)?; let reply = normalize(&bridge(&["write",&source.to_string(),&value])?)?; if reply != value { return Err("Capture preference readback rejected".into()); } Ok(()) })())
}
pub fn status(hardware: &impl Hardware) -> Result<Value> { Ok(serde_json::from_str(&hardware.read(STATUS)?)?) }
pub fn plan(hardware: &impl Hardware) -> Result<Vec<(String,String)>> {
    let status = status(hardware)?;
    if status["available"] != true || status["bluetoothConnected"] != true { return Err("Connect Bluetooth earbuds before enabling phone microphone preference".into()); }
    let value = normalize(&json!([status.get("phoneMic").ok_or("Built-in microphone not exposed")?]).to_string())?;
    let mic: Vec<Device> = serde_json::from_str(&value)?;
    if mic.first().is_none_or(|device| device.kind != 15) { return Err("Built-in microphone not exposed".into()); }
    Ok(PRESETS.iter().map(|source| (format!("audio-capture:{source}"),value.clone())).collect())
}
