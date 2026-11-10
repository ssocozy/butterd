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

/// One Android process for a fresh snapshot; no values are cached across operations.
pub fn read_many(paths: &[&str]) -> Result<Vec<String>> {
    let values: Value = serde_json::from_str(&bridge(&["preferences"])?)?;
    paths.iter().map(|path| normalize(&values.get(*path).ok_or("Missing capture preference")?.to_string())).collect()
}

/// A single Android process verifies and changes the three input presets.
fn batch_request(changes:&[(String,String,String)])->Result<String> {
    let mut values=serde_json::Map::new();
    for (path,before,target) in changes {
        if preset(path).is_none() || values.contains_key(path) { return Err("Invalid audio batch preset".into()); }
        values.insert(path.clone(),json!({"before":serde_json::from_str::<Value>(&normalize(before)?)?,"target":serde_json::from_str::<Value>(&normalize(target)?)?}));
    }
    if values.is_empty() || values.len()>3 { return Err("Invalid audio batch size".into()); }
    Ok(Value::Object(values).to_string())
}
pub fn write_many(changes:&[(String,String,String)])->Result<()> {
    if bridge(&["write-preferences",&batch_request(changes)?])? != "verified" { return Err("Audio batch not verified".into()); }
    Ok(())
}
pub fn matches_snapshot(snapshot:&Value,journal:&crate::journal::Journal)->Result<bool> {
    journal.entries.iter().try_fold(true,|matches,entry| {
        let current=normalize(&snapshot.get("preferences").and_then(|values|values.get(&entry.path)).ok_or("Audio preference snapshot missing")?.to_string())?;
        Ok(matches && entry.before.is_none() && entry.expected==current)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_keeps_expected_originals_and_validates_all_targets() {
        let changes=vec![("audio-capture:1".into(),"[]".into(),r#"[{"type":15,"address":"bottom"}]"#.into())];
        let value:Value=serde_json::from_str(&batch_request(&changes).unwrap()).unwrap();
        assert_eq!(value["audio-capture:1"]["before"],json!([]));
        assert_eq!(value["audio-capture:1"]["target"][0]["type"],15);
        for bad in [vec![],vec![changes[0].clone(),changes[0].clone()],vec![("audio-output:1".into(),"[]".into(),"[]".into())],vec![("audio-capture:1".into(),"[]".into(),r#"[{"type":0,"address":""}]"#.into())]] {
            assert!(batch_request(&bad).is_err());
        }
    }
}
