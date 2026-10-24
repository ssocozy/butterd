#![forbid(unsafe_code)]
use butter::{
    engine::Engine,
    hardware::{read_bounded, Hardware, SystemHardware, BOOT},
    journal::DiskStore,
    model::{valid_policy, Feature},
    Result,
};
use std::path::Path;
fn run() -> Result<()> {
    let status = read_bounded(Path::new("/proc/self/status"), 16384)?;
    let uid: Vec<_> = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .ok_or("Unable to verify root")?
        .split_whitespace()
        .collect();
    if uid.len() != 4 || uid.iter().any(|value| *value != "0") {
        return Err("Root is required".into());
    }
    let property = |name| -> Result<String> {
        let result = std::process::Command::new("/system/bin/getprop")
            .arg(name)
            .output()?;
        if !result.status.success() || result.stdout.len() > 256 {
            return Err("Device identity unavailable".into());
        }
        Ok(String::from_utf8(result.stdout)?.trim().into())
    };
    if property("ro.product.device")? != "duchamp"
        && property("ro.product.vendor.device")? != "duchamp"
    {
        return Err("Duchamp is required".into());
    }
    let hardware = SystemHardware;
    let boot = hardware.read(BOOT)?;
    if boot.len() != 36
        || !boot
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err("Boot identity unavailable".into());
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["status"] {
        let engine = Engine {
            hardware,
            store: DiskStore::new("/data/adb/butterd"),
            boot,
        };
        let mut status = engine.status();
        status["compatibility"] = butter::runtime::compatibility_status();
        status["supervisor"] = serde_json::json!({"config": butter::runtime::config().map(|value| serde_json::json!(value)).unwrap_or_else(|error| serde_json::json!({"error":error.to_string()})), "runtime": butter::runtime::state(&engine.boot).map(|value| serde_json::json!(value)).unwrap_or_else(|error| serde_json::json!({"error":error.to_string()})), "uptime": butter::runtime::uptime()?, "notificationDelivery":butter::runtime::notification_status(), "health":butter::runtime::supervisor_status(&engine.boot)});
        println!("{}", serde_json::to_string(&status)?);
        return Ok(());
    }
    match args.as_slice() {
        [command] if command == "notify-test" => { butter::notifications::post("Butterd notification test")?; println!("Android accepted the test notification. Check the notification shade."); return Ok(()); },
        [command, rest @ ..] if command == "activity" => { println!("{}",butter::app_activity::run(rest)?); return Ok(()); },
        [command] if command == "battery-usage" => { println!("{}",butter::battery_usage::read()?); return Ok(()); },
        [command] if command == "apps" => { println!("{}", butter::android::installed_apps()?); return Ok(()); },
        [command] if command == "daemon" => return butter::runtime::daemon(&boot),
        [command] if command == "boot" => return butter::runtime::launch_if_requested(&boot),
        [command] if command == "diagnostics" => { println!("{}", serde_json::to_string_pretty(&butter::runtime::diagnostics(&boot)?)?); return Ok(()); }
        [command] if command == "shutdown" => { println!("{}", butter::runtime::shutdown(&boot)?); return Ok(()); }
        [command, package] if command == "measure" => { println!("{}", butter::runtime::frame_sample(package)?); return Ok(()); }
        [command, ..] if matches!(command.as_str(), "preset" | "auto" | "app" | "boost" | "logging" | "recording" | "notifications" | "reset" | "monitor") => { println!("{}", butter::runtime::configure(&boot, &args)?); return Ok(()); }
        _ => {}
    }
    let (feature, setting, restoring, accepted) = match args.as_slice() {
        [command, policy, range] if command == "range" && valid_policy(policy) => (Feature::CpuRange(policy.clone()), range.as_str(), false, false),
        [command, gpu, range] if command == "range" && gpu == "gpu" => (Feature::GpuRange, range.as_str(), false, false),
        [command, range] if command == "restore-range" && valid_policy(range) => (Feature::CpuRange(range.clone()), "", true, false),
        [command, range] if command == "restore-range" && range == "gpu" => (Feature::GpuRange, "", true, false),
        [command, algorithm] if command == "tcp" => (Feature::Tcp, algorithm.as_str(), false, false),
        [command, all, governor] if command == "governor" && all == "all" => (Feature::Governors, governor.as_str(), false, false),
        [command, policy, governor] if command == "governor" && valid_policy(policy) => (Feature::Governor(policy.clone()), governor.as_str(), false, false),
        [command, policy] if command == "restore" && valid_policy(policy) => (Feature::Governor(policy.clone()), "", true, false),
        [command, feature] if command == "restore" => (Feature::parse(feature)?, "", true, false),
        [command, feature] if command == "enable" && feature == "phone-mic" => (Feature::PhoneMic, "", false, false),
        [command, feature, setting] if command == "enable" && feature == "touch" => (Feature::Touch, setting.as_str(), false, false),
        [command, feature, accepted] if command == "enable" && accepted == "--accept-risk" => (Feature::parse(feature)?, "", false, true),
        _ => return Err("Usage: butter status | governor policyN VALUE | enable CONTROL --accept-risk | enable touch 240|480|high | restore CONTROL".into()),
    };
    if feature.advanced() && !restoring && accepted {
        println!("{}", butter::runtime::guarded_apply(&boot, feature)?);
        return Ok(());
    }
    let engine = Engine {
        hardware,
        store: DiskStore::locked_wait("/data/adb/butterd",std::time::Duration::from_secs(5))?,
        boot,
    };
    let result = engine.apply(feature.clone(), setting, restoring, accepted);
    if restoring || result.is_ok() {
        if let Err(error) = butter::runtime::remember_applied(&engine.boot,&feature,setting,restoring,None) {
            let cleanup=engine.apply_targets(feature.clone(),Vec::new(),true);
            return Err(format!("Could not save the control preference: {error}; cleanup: {}",cleanup.unwrap_or_else(|error| error.to_string())).into());
        }
    }
    butter::runtime::log_control(&args, result.as_ref().err().map(|error| error.to_string()).as_deref());
    println!("{}", result?);
    drop(engine); butter::runtime::ensure_daemon()?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        butter::runtime::fatal(&error.to_string());
        std::process::exit(1);
    }
}
