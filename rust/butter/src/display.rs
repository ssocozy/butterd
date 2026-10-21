//! Journal-compatible display preferences, including ROM sentinel values.
use crate::{android, hardware::Hardware, Result};
pub fn app_node(path: &str) -> Option<(u32,&str,&str,&str)> {
    let parts: Vec<_> = path.split(':').collect();
    match parts.as_slice() {
        ["rom-app",user,service,descriptor,app] if matches!(*service,"refresh_rate_ext"|"refresh_rate")
            && valid_descriptor(descriptor) && android::valid_package(app) => Some((user.parse::<u32>().ok().filter(|user| *user <= 99999)?,service,descriptor,app)),
        _ => None,
    }
}
pub fn valid_descriptor(value: &str) -> bool {
    value.len() <= 180 && value.ends_with(".display.IRefreshRateManagerService")
        && value.split('.').all(|part| !part.is_empty() && part.as_bytes()[0].is_ascii_alphabetic() && part.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
}
fn bridge(args: &[&str]) -> Result<String> {
    let mut command = vec!["/system/bin","dev.butterd.module.RefreshRateBridge"];
    command.extend_from_slice(args);
    android::execute("/system/bin/app_process", &command, &[("CLASSPATH","/data/adb/modules/butterd/bin/catalog.dex")], 5)
}
pub fn probe(user: u32) -> Result<serde_json::Value> {
    let result: serde_json::Value = serde_json::from_str(&bridge(&["probe",&user.to_string()])?)?;
    if result.get("available").and_then(serde_json::Value::as_bool).is_none() { return Err("Invalid refresh adapter response".into()); }
    Ok(result)
}
pub fn probe_current() -> Result<serde_json::Value> {
    let user: u32 = android::command("/system/bin/am", &["get-current-user"])?.parse()?;
    if user > 99999 { return Err("Invalid Android user".into()); }
    probe(user)
}
pub fn node(path: &str) -> Option<(u32,&str,&str)> {
    if let Some((user,_,_,app)) = app_node(path) { return Some((user,"app",app)); }
    if let Some((user,key)) = android::parse_setting(path) { return Some((user,"system",key)); }
    let parts: Vec<_> = path.split(':').collect();
    let user = parts.get(1)?.parse::<u32>().ok().filter(|user| *user <= 99999)?;
    match parts.as_slice() {
        ["display",_,"preferred"] => Some((user,"mode","preferred")),
        ["rom-refresh",_,table,key] if matches!(*table,"system"|"secure") && matches!(*key,"user_refresh_rate"|"miui_refresh_rate"|"is_smart_fps") => Some((user,table,key)),
        _ => None,
    }
}
pub fn normalize(table: &str, key: &str, raw: &str) -> Result<String> {
    let raw = raw.trim();
    if table == "app" {
        let value = raw.parse::<i32>()?;
        if value == -1 || value == 0 { return Ok("-1".into()); }
        if (30..=240).contains(&value) { return Ok(value.to_string()); }
        return Err("Invalid ROM app refresh rate".into());
    }
    if raw == "null" { return Ok(raw.into()); }
    if table == "mode" {
        let parts: Vec<_> = raw.split_whitespace().collect();
        if parts.len() != 3 { return Err("Unsupported preferred display mode response".into()); }
        // Android may construct an unset Mode rather than return a null object.
        // Canonicalize that exact sentinel so release clears the preference again.
        if parts[0] == "-1" && parts[1] == "-1" && parts[2].parse::<f64>()? == 0.0 { return Ok("null".into()); }
        let width = parts[0].parse::<u32>()?; let height = parts[1].parse::<u32>()?; let hz = parts[2].parse::<f64>()?;
        if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) || !hz.is_finite() || !(1.0..=144.1).contains(&hz) { return Err("Invalid preferred display mode".into()); }
        let rate = if (hz-hz.round()).abs() < 0.1 { hz.round() } else { hz };
        return Ok(format!("{width} {height} {rate}"));
    }
    if key == "peak_refresh_rate" && raw.eq_ignore_ascii_case("Infinity") { return Ok("Infinity".into()); }
    let hz = raw.parse::<f64>()?;
    let valid = if key == "is_smart_fps" { hz == 0.0 || hz == 1.0 }
        else if matches!(key,"user_refresh_rate"|"miui_refresh_rate") { (30.0..=240.0).contains(&hz) }
        else { (0.0..=144.1).contains(&hz) };
    if !hz.is_finite() || !valid { return Err(format!("Invalid {table} display preference {key}: {raw}").into()); }
    Ok(hz.to_string())
}
pub fn preferred_reply(raw: &str) -> Result<String> {
    normalize("mode","preferred",raw.trim().strip_prefix("User preferred display mode:").ok_or("Preferred mode API unavailable")?.trim())
}
pub fn read(path: &str) -> Option<Result<String>> {
    if let Some((user,service,descriptor,app)) = app_node(path) {
        return Some(bridge(&["read",&user.to_string(),service,descriptor,app]).and_then(|raw| normalize("app",app,&raw)));
    }
    let (user,table,key) = node(path)?;
    Some(if table == "mode" {
        android::command("/system/bin/cmd", &["display","get-user-preferred-display-mode","0"]).and_then(|raw| preferred_reply(&raw))
    } else { android::setting_in(user,table,key,None).and_then(|raw| normalize(table,key,&raw)) })
}
pub fn write(path: &str, value: &str) -> Option<Result<()>> {
    if let Some((user,service,descriptor,app)) = app_node(path) {
        return Some((|| { let value = normalize("app",app,value)?; bridge(&["write",&user.to_string(),service,descriptor,app,&value])?; Ok(()) })());
    }
    let (user,table,key) = node(path)?;
    Some((|| {
        let value = normalize(table,key,value)?;
        if table == "mode" {
            if value == "null" { android::command("/system/bin/cmd", &["display","clear-user-preferred-display-mode","0"])?; }
            else { let parts: Vec<_> = value.split_whitespace().collect(); android::command("/system/bin/cmd", &["display","set-user-preferred-display-mode",parts[0],parts[1],parts[2],"0"])?; }
        } else { android::setting_in(user,table,key,Some(&value))?; }
        Ok(())
    })())
}
pub fn physical_mode(text: &str, hz: u32) -> Option<(u32,u32)> {
    for modes in text.lines().filter_map(android::advertised_modes) {
        for block in modes.split('{').skip(1).filter_map(|tail| tail.split('}').next()) {
            let number = |key: &str| -> Option<f64> { block.split_once(key)?.1.chars().take_while(|ch| ch.is_ascii_digit() || *ch == '.').collect::<String>().parse().ok() };
            if number("fps=").is_some_and(|value| (value-f64::from(hz)).abs()<0.1) {
                let width = number("width=")? as u32; let height = number("height=")? as u32;
                if (1..=8192).contains(&width) && (1..=8192).contains(&height) { return Some((width,height)); }
            }
        }
    }
    None
}
pub fn targets(hardware: &impl Hardware, user: u32, hz: u32, dump: &str) -> Result<Vec<(String,String)>> {
    if !android::display_rates(dump).contains(&hz) { return Err("Refresh mode is not advertised by the internal display".into()); }
    let mut targets = Vec::new();
    let preferred = format!("display:{user}:preferred");
    if let Some((width,height)) = physical_mode(dump,hz) {
        if hardware.read(&preferred).is_ok() { targets.push((preferred,format!("{width} {height} {hz}"))); }
    }
    for key in ["peak_refresh_rate","min_refresh_rate"] { targets.push((android::setting_path(user,key),hz.to_string())); }
    let mut miui = false;
    for table in ["system","secure"] {
        for key in ["user_refresh_rate","miui_refresh_rate"] {
            let path = format!("rom-refresh:{user}:{table}:{key}");
            if hardware.read(&path).is_ok_and(|raw| raw.parse::<f64>().is_ok_and(|value| value.is_finite() && (30.0..=240.0).contains(&value))) {
                miui |= key == "miui_refresh_rate";
                targets.push((path,hz.to_string()));
            }
        }
    }
    let smart = format!("rom-refresh:{user}:system:is_smart_fps");
    if miui && hardware.read(&smart).is_ok_and(|value| matches!(value.as_str(),"0"|"1")) { targets.push((smart,"0".into())); }
    Ok(targets)
}

/// Add only the active package's ROM rule, never rewrite the entire per-app settings table.
pub fn app_targets(hardware: &impl Hardware, user: u32, app: &str, hz: u32, adapter: &serde_json::Value) -> Result<Vec<(String,String)>> {
    if !android::valid_package(app) || user > 99999 || !matches!(hz,60|90|120) { return Err("Invalid per-app refresh request".into()); }
    if adapter.get("available").and_then(serde_json::Value::as_bool) == Some(false) { return Ok(Vec::new()); }
    let service = adapter.get("service").and_then(serde_json::Value::as_str).ok_or("Missing ROM refresh service")?;
    let descriptor = adapter.get("descriptor").and_then(serde_json::Value::as_str).ok_or("Missing ROM refresh interface")?;
    let path = format!("rom-app:{user}:{service}:{descriptor}:{app}");
    if app_node(&path).is_none() { return Err("Unsupported ROM refresh interface".into()); }
    normalize("app",app,&hardware.read(&path)?)?;
    Ok(vec![(path,hz.to_string())])
}
