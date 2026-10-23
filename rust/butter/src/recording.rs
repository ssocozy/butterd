//! Read-only media-projection diagnostics. A projection is not proof of encoded frames.
use crate::{android, Result};
use serde_json::{json, Value};

pub fn parse(text: &str) -> Result<Value> {
    if text.len() > 16384 { return Err("Media projection response exceeds limit".into()); }
    let lines: Vec<_> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    if lines.first() != Some(&"MEDIA PROJECTION MANAGER (dumpsys media_projection)") {
        return Err("Media projection service output is unsupported or inaccessible".into());
    }
    let index = lines.iter().position(|line| *line == "Media Projection:").ok_or("Missing media projection state")?;
    let line = *lines.get(index + 1).ok_or("Missing media projection session")?;
    if line == "null" { return Ok(json!({"available":true,"active":false,"screenCapture":false,"package":null,"kind":null})); }
    let (identity, kind) = line.split_once("): ").ok_or("Unsupported media projection session")?;
    let (package, uid) = identity.strip_prefix('(').and_then(|value|value.split_once(", uid=")).ok_or("Invalid media projection identity")?;
    if !android::valid_package(package) || uid.parse::<u32>().is_err() { return Err("Invalid media projection identity".into()); }
    let screen = match kind { "TYPE_SCREEN_CAPTURE" => true, "TYPE_MIRRORING" | "TYPE_PRESENTATION" => false, _ => return Err("Unknown media projection type".into()) };
    Ok(json!({"available":true,"active":true,"screenCapture":screen,"package":package,"kind":kind}))
}
pub fn sample(now: u64) -> Value {
    let result = android::command("/system/bin/dumpsys", &["media_projection"]).and_then(|text|parse(&text));
    let mut value = result.unwrap_or_else(|error| json!({"available":false,"active":null,"screenCapture":null,"error":error.to_string()}));
    value["sampledAtUptime"] = json!(now);
    value["source"] = json!("Android media_projection");
    value["encodingVerified"] = json!(false);
    value
}
