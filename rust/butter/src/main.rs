#![forbid(unsafe_code)]
use butter::{engine::Engine, hardware::{read_bounded, Hardware, SystemHardware, BOOT}, journal::DiskStore, model::{valid_policy, Feature}, Result};
use std::path::Path;
fn run() -> Result<()> {
    let status = read_bounded(Path::new("/proc/self/status"), 16384)?;
    let uid: Vec<_> = status.lines().find_map(|line| line.strip_prefix("Uid:")).ok_or("Unable to verify root")?.split_whitespace().collect();
    if uid.len() != 4 || uid.iter().any(|value| *value != "0") { return Err("Root is required".into()); }
    let property = |name| -> Result<String> {
        let result = std::process::Command::new("/system/bin/getprop").arg(name).output()?;
        if !result.status.success() || result.stdout.len() > 256 { return Err("Device identity unavailable".into()); }
        Ok(String::from_utf8(result.stdout)?.trim().into())
    };
    if property("ro.product.device")? != "duchamp" && property("ro.product.vendor.device")? != "duchamp" { return Err("Duchamp is required".into()); }
    let hardware = SystemHardware; let boot = hardware.read(BOOT)?;
    if boot.len() != 36 || !boot.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'-') { return Err("Boot identity unavailable".into()); }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["status"] {
        let engine = Engine { hardware, store: DiskStore::new("/data/adb/butterd"), boot };
        println!("{}", serde_json::to_string(&engine.status())?); return Ok(());
    }
    let (feature, setting, restoring, accepted) = match args.as_slice() {
        [command, algorithm] if command == "tcp" => (Feature::Tcp, algorithm.as_str(), false, false),
        [command, policy, governor] if command == "governor" && valid_policy(policy) => (Feature::Governor(policy.clone()), governor.as_str(), false, false),
        [command, policy] if command == "restore" && valid_policy(policy) => (Feature::Governor(policy.clone()), "", true, false),
        [command, feature] if command == "restore" => (Feature::parse(feature)?, "", true, false),
        [command, feature, setting] if command == "enable" && feature == "touch" => (Feature::Touch, setting.as_str(), false, false),
        [command, feature, accepted] if command == "enable" && accepted == "--accept-risk" => (Feature::parse(feature)?, "", false, true),
        _ => return Err("Usage: butter status | governor policyN VALUE | enable CONTROL --accept-risk | enable touch 240|480|high | restore CONTROL".into()),
    };
    let engine = Engine { hardware, store: DiskStore::locked("/data/adb/butterd")?, boot };
    println!("{}", engine.apply(feature, setting, restoring, accepted)?); Ok(())
}
fn main() { if let Err(error) = run() { eprintln!("{error}"); std::process::exit(1); } }
