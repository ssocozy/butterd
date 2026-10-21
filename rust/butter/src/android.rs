//! Bounded Android service calls. No shell, service transaction numbers or ROM patches.
use crate::Result;
use std::{io::Read, process::{Command, Stdio}, time::{Duration, Instant}};

pub fn command(binary: &str, args: &[&str]) -> Result<String> {
    let mut child = Command::new(binary).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>> {
        std::thread::spawn(move || {
            let mut output = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let count = pipe.read(&mut chunk)?;
                if count == 0 { break; }
                if output.len() + count > 524288 { return Err(std::io::Error::other("Android response exceeded limit")); }
                output.extend_from_slice(&chunk[..count]);
            }
            Ok(output)
        })
    }
    let out = drain(child.stdout.take().ok_or("Missing stdout")?);
    let err = drain(child.stderr.take().ok_or("Missing stderr")?);
    let deadline = Instant::now() + Duration::from_secs(3);
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => { let _ = child.kill(); let _ = child.wait(); break Err("Android service timed out"); }
            Err(_) => { let _ = child.kill(); let _ = child.wait(); break Err("Android service failed"); }
        }
    };
    let output = out.join().map_err(|_| "Android output reader failed")??;
    let errors = err.join().map_err(|_| "Android error reader failed")??;
    if !result?.success() || !errors.is_empty() { return Err("Android service rejected the request".into()); }
    Ok(String::from_utf8(output)?.trim().into())
}
pub fn valid_package(value: &str) -> bool {
    value.len() <= 180 && value.contains('.') && value.split('.').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}
pub fn foreground(text: &str) -> Option<String> {
    let mut packages = std::collections::BTreeSet::new();
    for line in text.lines().filter(|line| line.contains("topResumedActivity=") || line.contains("mResumedActivity:")) {
        for token in line.split_whitespace() {
            if let Some((package, _)) = token.split_once('/') {
                if valid_package(package) { packages.insert(package.to_string()); }
            }
        }
    }
    if packages.len() == 1 { packages.into_iter().next() } else { None }
}
pub fn display_rates(text: &str) -> Vec<u32> {
    // Only parse the physical built-in display's advertised mode list.
    let mut rates = Vec::new();
    for line in text.lines().filter(|line| line.contains("DisplayDeviceInfo{") && line.contains("type INTERNAL")) {
        if let Some(modes) = line.split("supportedModes").nth(1).and_then(|tail| tail.split(']').next()) {
            for tail in modes.split("fps=").skip(1) {
                let raw: String = tail.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                if let Ok(value) = raw.parse::<f64>() {
                    let rounded = value.round() as u32;
                    if (value - f64::from(rounded)).abs() < 0.1 && (30..=144).contains(&rounded) { rates.push(rounded); }
                }
            }
        }
    }
    rates.sort_unstable(); rates.dedup(); rates
}
pub fn setting_path(user: u32, key: &str) -> String { format!("android:{user}:{key}") }
pub fn parse_setting(path: &str) -> Option<(u32, &str)> {
    let mut parts = path.split(':');
    if parts.next()? != "android" { return None; }
    let user = parts.next()?.parse::<u32>().ok()?;
    let key = parts.next()?;
    if user > 99999 || parts.next().is_some() || !matches!(key, "min_refresh_rate" | "peak_refresh_rate") { return None; }
    Some((user, key))
}
pub fn setting(user: u32, key: &str, value: Option<&str>) -> Result<String> {
    let user = user.to_string();
    let args = match value {
        Some("null") => vec!["--user", &user, "delete", "system", key],
        Some(value) => vec!["--user", &user, "put", "system", key, value],
        None => vec!["--user", &user, "get", "system", key],
    };
    command("/system/bin/settings", &args)
}
