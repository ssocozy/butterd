//! User-space supervisor: opt-in policy, expiring leases and conflict-aware cleanup.
use crate::{android, engine::Engine, hardware::{read_bounded, Hardware, SystemHardware}, journal::DiskStore, model::{number, Feature}, presets::Mode, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs::{self, File, OpenOptions}, io::Write, os::unix::fs::{OpenOptionsExt, PermissionsExt}, path::Path, process::{Command, Stdio}, time::Duration};
pub const DIRECTORY: &str = "/data/adb/butterd";
const MODULE: &str = "/data/adb/modules/butterd";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppRule { pub package: String, pub preset: Mode, pub hz: Option<u32> }
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config { pub automatic: bool, pub preset: Mode, pub logcat: bool, pub rules: Vec<AppRule> }
impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.rules.len() > 64 || self.preset == Mode::Boost { return Err("Invalid engine configuration".into()); }
        let mut seen = std::collections::HashSet::new();
        for rule in &self.rules {
            if !android::valid_package(&rule.package) || !seen.insert(&rule.package) || rule.preset == Mode::Boost || rule.hz.is_some_and(|hz| !matches!(hz, 60 | 90 | 120)) { return Err("Invalid or duplicate app rule".into()); }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease { pub source: String, pub expires: u64 }
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeState {
    pub boot: String, pub active: Mode, pub reason: String, pub foreground: Option<String>,
    pub tuning_requested: bool, pub selected: Mode, pub advanced_deadlines: BTreeMap<String, u64>, pub leases: Vec<Lease>, pub blocked: bool, pub refresh_user: Option<u32>, pub refresh_hz: Option<u32>, pub idle_samples: u8, pub sampled_at: u64,
    pub telemetry: Value, pub error: Option<String>, pub pending_preset: Option<Mode>,
    pub refresh_error: Option<String>, pub refresh_blocked: bool, pub profile_retries: u8,
}
pub fn uptime() -> Result<u64> {
    let text = read_bounded(Path::new("/proc/uptime"), 128)?;
    number(text.split('.').next().ok_or("Missing uptime")?)
}
fn load<T: serde::de::DeserializeOwned + Default>(name: &str) -> Result<T> {
    let path = Path::new(DIRECTORY).join(name);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(error.into()),
        Ok(meta) => {
            if !meta.is_file() || meta.file_type().is_symlink() { return Err("Invalid engine state file".into()); }
            Ok(serde_json::from_str(&read_bounded(&path, 65536)?)?)
        }
    }
}
fn save(name: &str, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > 65536 { return Err("Engine state exceeds limit".into()); }
    let temporary = Path::new(DIRECTORY).join(format!("state-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary)?;
    let result = (|| -> Result<()> {
        file.write_all(&bytes)?; file.sync_all()?;
        fs::rename(&temporary, Path::new(DIRECTORY).join(name))?;
        File::open(DIRECTORY)?.sync_all()?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary);
    result
}
pub fn config() -> Result<Config> { let value: Config = load("config.json")?; value.validate()?; Ok(value) }
pub fn state(boot: &str) -> Result<RuntimeState> {
    let value: RuntimeState = load("runtime.json")?;
    if value.boot != boot { return Ok(RuntimeState { boot: boot.into(), ..Default::default() }); }
    if value.advanced_deadlines.len() > 8 || value.advanced_deadlines.keys().any(|key| !matches!(key.as_str(), "cpu-lock" | "gpu-lock" | "charging-thermal" | "gpu-thermal" | "charging-pause" | "bypass")) || value.leases.len() > 16 || value.leases.iter().any(|lease| !matches!(lease.source.as_str(), "launch" | "manual")) || value.refresh_user.is_some_and(|user| user > 99999) { return Err("Invalid runtime state".into()); }
    Ok(value)
}
fn engine(boot: &str) -> Result<Engine<SystemHardware, DiskStore>> {
    for attempt in 0..40 {
        match DiskStore::locked(DIRECTORY) {
            Ok(store) => return Ok(Engine { hardware:SystemHardware, store, boot:boot.into() }),
            Err(error) if error.downcast_ref::<crate::lock::Busy>().is_some() && attempt < 39 => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(error),
        }
    }
    Err("Control lock retry limit reached".into())
}
fn event(config: &Config, kind: &str, detail: &str) {
    let record = json!({"uptime":uptime().ok(),"event":kind,"detail":detail});
    let path = Path::new(DIRECTORY).join("events.jsonl");
    if fs::symlink_metadata(&path).is_ok_and(|meta| !meta.is_file() || meta.file_type().is_symlink()) { return; }
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > 262144) { let _ = fs::rename(&path, Path::new(DIRECTORY).join("events.previous.jsonl")); }
    if let Ok(mut file) = OpenOptions::new().append(true).create(true).mode(0o600).open(&path) { let _ = writeln!(file, "{record}"); }
    if config.logcat { let _ = android::command("/system/bin/log", &["-p", if kind == "error" { "e" } else { "i" }, "-t", "butter", &record.to_string()]); }
}
pub fn ensure_daemon() -> Result<()> {
    let _ = fs::remove_file(Path::new(DIRECTORY).join("stop"));
    if !Path::new(MODULE).is_dir() || ["disable", "remove"].iter().any(|file| Path::new(MODULE).join(file).exists()) { return Err("Module is disabled or being removed".into()); }
    { let _ = engine(&crate::hardware::SystemHardware.read(crate::hardware::BOOT)?)?; }
    if crate::lock::FileLock::held(&Path::new(DIRECTORY).join("daemon.lock"))? { return Ok(()); }
    let log_path = Path::new(DIRECTORY).join("supervisor.log");
    if fs::symlink_metadata(&log_path).is_ok_and(|meta| !meta.is_file() || meta.file_type().is_symlink()) { return Err("Invalid supervisor log".into()); }
    if fs::metadata(&log_path).is_ok_and(|meta| meta.len() > 16384) { fs::rename(&log_path, Path::new(DIRECTORY).join("supervisor.previous.log"))?; }
    let log = OpenOptions::new().append(true).create(true).mode(0o600).open(log_path)?;
    let mut child = Command::new(std::env::current_exe()?).arg("daemon").stdin(Stdio::null()).stdout(Stdio::from(log.try_clone()?)).stderr(Stdio::from(log)).spawn()?;
    for _ in 0..30 {
        if crate::lock::FileLock::held(&Path::new(DIRECTORY).join("daemon.lock"))? {
                // Reap an already-exited duplicate; a detached live worker exits with Android.
                let _ = child.try_wait();
                return Ok(());
        }
        if child.try_wait()?.is_some() { return Err("Supervisor failed to start".into()); }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill(); let _ = child.wait();
    Err("Supervisor startup timed out".into())
}
pub fn configure(boot: &str, args: &[String]) -> Result<String> {
    {
        let e = engine(boot)?;
        let resetting = args == ["reset"];
        let mut cfg = match config() { Ok(cfg) => cfg, Err(_) if resetting => Config::default(), Err(error) => return Err(error) };
        let mut state = match state(boot) { Ok(state) => state, Err(_) if resetting => RuntimeState {boot:boot.into(), ..Default::default()}, Err(error) => return Err(error) };
        match args {
            [command] if command == "monitor" => {},
            [command, choice] if command == "auto" && matches!(choice.as_str(), "on" | "off") => { state.refresh_blocked = false; state.refresh_error = None; cfg.automatic = choice == "on"; state.tuning_requested = cfg.automatic; state.blocked = false; state.leases.clear(); state.selected = Mode::Balanced; state.pending_preset = None; }
            [command, choice] if command == "logging" && matches!(choice.as_str(), "on" | "off") => cfg.logcat = choice == "on",
            [command, mode] if command == "preset" => { let mode = Mode::parse(mode)?; if mode == Mode::Boost { return Err("Use boost SECONDS".into()); } state.tuning_requested = true; cfg.preset = mode; state.selected = mode; state.pending_preset = Some(mode); state.blocked = false; state.profile_retries = 0; state.leases.clear(); }
            [command, seconds] if command == "boost" => {
                let seconds = number(seconds)?;
                if !(1..=30).contains(&seconds) { return Err("Boost duration must be 1–30 seconds".into()); }
                state.tuning_requested = true; state.leases.retain(|lease| lease.source != "manual");
                state.leases.push(Lease { source: "manual".into(), expires: uptime()? + seconds }); state.blocked = false;
            }
            [command, package, mode, hz] if command == "app" => {
                if !android::valid_package(package) { return Err("Invalid app package".into()); }
                cfg.rules.retain(|rule| rule.package != *package);
                if mode != "remove" {
                    let preset = Mode::parse(mode)?;
                    let hz = if hz == "rom" { None } else { Some(hz.parse()?) };
                    cfg.rules.push(AppRule { package: package.clone(), preset, hz });
                    cfg.automatic = true; state.tuning_requested = true;
                }
                state.profile_retries = 0; state.blocked = false; state.refresh_blocked = false; state.refresh_error = None;
            }
            [command] if command == "reset" => {
                cfg.automatic = false; state.refresh_blocked = false; state.refresh_error = None; state.tuning_requested = false; cfg.preset = Mode::Balanced; state.leases.clear(); state.selected = Mode::Balanced; state.pending_preset = Some(Mode::Balanced); state.blocked = false;
                save("config.json", &cfg)?; save("runtime.json", &state)?;
                cleanup_all(&e, &mut state)?;
            }
            _ => return Err("Usage: preset balanced|performance|battery | auto on|off | app PACKAGE PRESET 60|90|120|rom | app PACKAGE remove rom | boost SECONDS | logging on|off | reset".into()),
        }
        cfg.validate()?;
        save("config.json", &cfg)?; save("runtime.json", &state)?;
        event(&cfg, "configuration", &args.join(" "));
    }
    ensure_daemon()?;
    let _ = android::command("/system/bin/log", &["-p", "i", "-t", "butterWake", "configuration"]);
    Ok("Request saved. The supervisor will apply it after checking device state.".into())
}
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub awake: Option<bool>, pub saver: Option<bool>, pub temperature: Option<i64>, pub battery: Option<u64>,
    pub soc_millic: Option<i64>, pub gpu_busy: Option<f64>, pub online: Option<bool>, pub user: Option<u32>, pub app: Option<String>, pub rates: Vec<u32>, pub hz: Option<f64>,
}
pub fn decide(config: &Config, state: &mut RuntimeState, context: &Context, now: u64) -> (Mode, &'static str) {
    state.leases.retain(|lease| lease.expires > now && lease.expires <= now.saturating_add(30));
    if !config.automatic && !state.tuning_requested { state.leases.clear(); return (Mode::Balanced, "Monitoring only"); }
    if state.blocked { state.leases.clear(); return (Mode::Balanced, "Paused after a conflict or failure; select a preset to retry"); }
    if context.awake != Some(true) { state.leases.clear(); return (Mode::Balanced, "Screen off or display state unknown"); }
    let cooling = state.reason == "Cooling down";
    let warm = context.temperature.is_none_or(|temp| temp >= if cooling { 390 } else { 420 }) || context.soc_millic.is_some_and(|temp| temp >= if cooling { 75000 } else { 85000 });
    if warm { state.leases.clear(); return (Mode::Balanced, "Cooling down"); }
    if context.saver != Some(false) || context.battery.is_none_or(|level| level <= 20) { state.leases.clear(); return (Mode::Battery, "Battery saver or low battery"); }
    if context.app.is_none() { state.leases.clear(); return (Mode::Balanced, "Foreground app unknown or multiple resumed apps"); }
    if state.foreground != context.app { state.leases.retain(|lease| lease.source != "launch"); }
    let rule = config.rules.iter().find(|rule| Some(&rule.package) == context.app.as_ref());
    let requested = state.pending_preset.take().unwrap_or(if config.automatic { rule.map_or(config.preset, |rule| rule.preset) } else { state.selected });
    if config.automatic && rule.is_some_and(|rule| rule.preset == Mode::Performance) && state.foreground != context.app && requested == Mode::Performance {
        state.leases.retain(|lease| lease.source != "launch");
        state.leases.push(Lease { source: "launch".into(), expires: now.saturating_add(3) });
    }
    let idle = state.telemetry.get("cpuBusyPercent").and_then(Value::as_f64).is_some_and(|load| load < 15.0) && context.gpu_busy.is_some_and(|load| load < 15.0);
    state.idle_samples = if idle { state.idle_samples.saturating_add(1).min(3) } else { 0 };
    if !state.leases.is_empty() { (Mode::Boost, "Temporary boost lease") }
    else if requested == Mode::Performance && state.idle_samples >= 3 { (Mode::Balanced, "Idle workload; frequency floors released") }
    else { (requested, if config.automatic && rule.is_some() { "Foreground app rule" } else { "Selected preset" }) }
}
fn context() -> Context {
    let hw = SystemHardware;
    let read = |path: &str| hw.read(path).ok();
    let power = android::command("/system/bin/dumpsys", &["power"]).ok();
    let awake = power.as_ref().and_then(|text| text.lines().find_map(|line| line.trim().strip_prefix("mWakefulness=").map(|value| value == "Awake")));
    let saver = android::command("/system/bin/settings", &["get", "global", "low_power"]).ok().and_then(|value| match value.as_str() { "0" => Some(false), "1" => Some(true), _ => None });
    let app = if awake == Some(true) { android::command("/system/bin/dumpsys", &["activity", "activities"]).ok().and_then(|text| android::foreground(&text)) } else { None };
    let user = android::command("/system/bin/am", &["get-current-user"]).ok().and_then(|value| value.parse().ok()).filter(|user| *user <= 99999);
    let display = if awake == Some(true) { android::command("/system/bin/dumpsys", &["display"]).unwrap_or_default() } else { String::new() };
    let soc_millic = hw.list(crate::hardware::THERMAL).unwrap_or_default().into_iter().filter(|name| name.strip_prefix("thermal_zone").is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))).filter_map(|name| {
        let base = format!("{}/{name}", crate::hardware::THERMAL);
        let kind = hw.read(&format!("{base}/type")).ok()?.to_ascii_lowercase();
        if !(kind == "soc_max" || kind.contains("cpu") || kind == "gpu") { return None; }
        hw.read(&format!("{base}/temp")).ok()?.parse::<i64>().ok().filter(|value| (0..=200000).contains(value))
    }).max();
    let gpu_busy = [crate::hardware::GPU,crate::hardware::GPU_ALT].iter().find_map(|node| {
        let raw = hw.read(&format!("{}/load", node.trim_end_matches("/min_freq"))).ok()?;
        raw.trim_end_matches('%').parse::<f64>().ok().filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
    });
    Context {
        awake, saver, soc_millic, gpu_busy, temperature: read("/sys/class/power_supply/battery/temp").and_then(|value| value.parse().ok()).filter(|v| (-200..=800).contains(v)),
        battery: read("/sys/class/power_supply/battery/capacity").and_then(|value| value.parse().ok()).filter(|value| *value <= 100),
        online: read("/sys/class/power_supply/usb/online").and_then(|value| match value.as_str() { "1" => Some(true), "0" => Some(false), _ => None }),
        user, app, rates: android::display_rates(&display), hz: active_hz(&display),
    }
}
pub fn active_hz(text: &str) -> Option<f64> {
    for line in text.lines().filter(|line| line.contains("DisplayDeviceInfo{") && line.contains("type INTERNAL")) {
        if let Some(id) = line.split("modeId ").nth(1).or_else(|| line.split("modeId=").nth(1)) {
            let id: String = id.chars().take_while(char::is_ascii_digit).collect();
            if !id.is_empty() {
                if let Some(mode) = android::advertised_modes(line).and_then(|modes| modes.split(&format!("id={id},")).nth(1)).and_then(|tail| tail.split('}').next()) {
                    if let Some(fps) = mode.split("fps=").nth(1) {
                        let number: String = fps.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                        if let Ok(hz) = number.parse::<f64>() { if hz.is_finite() && (1.0..=144.1).contains(&hz) { return Some(hz); } }
                    }
                }
            }
        }
        if let Some(tail) = line.split("refreshRate=").nth(1) {
            let number: String = tail.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if let Ok(hz) = number.parse::<f64>() { if hz.is_finite() && (1.0..=144.1).contains(&hz) { return Some(hz); } }
        }
    }
    None
}
fn restore_refresh(e: &Engine<SystemHardware, DiskStore>, state: &mut RuntimeState) -> Result<()> {
    if let Some(user) = state.refresh_user {
        e.apply_targets(Feature::Refresh(user), Vec::new(), true)?;
        state.refresh_user = None; state.refresh_hz = None;
    }
    Ok(())
}
/// Low refresh caps remain useful during heat/saver; CPU conflicts do not disable display rules.
pub fn requested_refresh(cfg: &Config, state: &RuntimeState, ctx: &Context) -> Option<u32> {
    if state.refresh_blocked || !cfg.automatic || ctx.awake != Some(true) { return None; }
    let hz = cfg.rules.iter().find(|rule| Some(&rule.package) == ctx.app.as_ref())?.hz?;
    if hz == 60 { return Some(hz); }
    if ctx.saver == Some(false) && ctx.battery.is_some_and(|level| level > 20)
        && ctx.temperature.is_some_and(|temp| temp < 420) && ctx.soc_millic.is_none_or(|temp| temp < 85000) { Some(hz) } else { None }
}
fn refresh(e: &Engine<SystemHardware, DiskStore>, cfg: &Config, state: &mut RuntimeState, ctx: &Context) -> Result<()> {
    let rule = requested_refresh(cfg, state, ctx);
    if state.foreground != ctx.app || state.refresh_user != ctx.user || state.refresh_hz != rule || rule.is_none() { restore_refresh(e, state)?; }
    let (Some(hz), Some(user)) = (rule, ctx.user) else { return Ok(()); };
    if !ctx.rates.contains(&hz) { return Err("Requested refresh mode is not advertised by the internal display".into()); }
    let feature = Feature::Refresh(user);
    if state.refresh_user.is_some() {
        if e.state(&feature)? != "Request active" { return Err("ROM changed refresh settings; automatic display control paused".into()); }
        return Ok(());
    }
    // Never seize an existing refresh lease left by an interrupted process.
    if e.session(&feature)?.is_some() { e.apply_targets(feature.clone(), Vec::new(), true)?; }
    state.refresh_user = Some(user); state.refresh_hz = Some(hz);
    // Save ownership before the first Android settings write.
    save("runtime.json", state)?;
    let dump = android::command("/system/bin/dumpsys", &["display"])?;
    let targets = crate::display::targets(&e.hardware,user,hz,&dump)?;
    let controls = targets.len();
    e.apply_targets(feature, targets, false)?;
    event(cfg,"display-request",&format!("Requested {hz} Hz for {} using {controls} display preferences",ctx.app.as_deref().unwrap_or("unknown app")));
    Ok(())
}
fn thermal_cleanup(e: &Engine<SystemHardware, DiskStore>, state: &mut RuntimeState, ctx: &Context, now: u64) -> Result<()> {
    let unsafe_temperature = ctx.temperature.is_none_or(|temp| !(0..420).contains(&temp)) || ctx.soc_millic.is_some_and(|temp| temp >= 85000);
    let mut errors = Vec::new();
    for feature in [Feature::ChargingPause, Feature::ChargingThermal, Feature::GpuThermal, Feature::CpuLock, Feature::GpuLock, Feature::Bypass] {
        let charging = matches!(feature, Feature::ChargingPause | Feature::ChargingThermal | Feature::Bypass);
        let should_restore = state.advanced_deadlines.get(&feature.key()).is_none_or(|expires| *expires <= now || *expires > now.saturating_add(1800)) || unsafe_temperature || ctx.battery.is_none_or(|level| level <= 20) || ctx.saver != Some(false) || (charging && ctx.online != Some(true)) || (!charging && (ctx.awake != Some(true) || ctx.soc_millic.is_none()));
        if should_restore {
            match e.session(&feature).and_then(|session| if session.is_some() { e.apply_targets(feature.clone(), Vec::new(), true).map(|_| ()) } else { Ok(()) }) {
                Ok(()) => { state.advanced_deadlines.remove(&feature.key()); },
                Err(error) => errors.push(format!("{}: {error}", feature.key())),
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ").into()) }
}
fn telemetry(ctx: &Context, cpu: Option<f64>) -> Value {
    let hw = SystemHardware;
    let read = |name: &str| hw.read(&format!("/sys/class/power_supply/battery/{name}")).ok();
    let integer = |name: &str| read(name).and_then(|value| value.parse::<i64>().ok());
    let watts = integer("current_now").zip(integer("voltage_now")).and_then(|(amps, volts)| {
        if amps.unsigned_abs() > 50_000_000 || !(1_000_000..=10_000_000).contains(&volts) { return None; }
        let watts = (amps as f64 * volts as f64 / 1e12).abs();
        // Battery current polarity differs by driver. Use status to normalize the display.
        match read("status").as_deref() { Some("Discharging") => Some(-watts), Some("Charging") => Some(watts), Some("Not charging") => Some(-watts), Some("Full") => Some(0.0), _ => None }
    });
    let health_percent = integer("charge_full").zip(integer("charge_full_design")).filter(|(full, design)| *full > 0 && *design > 0 && full <= &design.saturating_mul(2)).map(|(full, design)| full as f64 * 100.0 / design as f64);
    json!({"batteryPercent":ctx.battery,"batteryTempC":ctx.temperature.map(|v| v as f64 / 10.0),"batteryWatts":watts,"usbOnline":ctx.online,"batteryStatus":read("status"),"health":read("health"),"healthPercent":health_percent,"cycleCount":integer("cycle_count"),"voltageUv":integer("voltage_now"),"currentUa":integer("current_now"),"screenHz":ctx.hz,"supportedHz":ctx.rates,"cpuBusyPercent":cpu,"gpuBusyPercent":ctx.gpu_busy,"socTempC":ctx.soc_millic.map(|v| v as f64 / 1000.0)})
}
pub fn cpu_counters(text: &str) -> Option<(u64, u64)> {
    let fields: Vec<_> = text.lines().next()?.split_whitespace().collect();
    if fields.first() != Some(&"cpu") || fields.len() < 5 { return None; }
    let values: Vec<u64> = fields.iter().skip(1).take(8).map(|value| value.parse().ok()).collect::<Option<_>>()?;
    let total = values.iter().try_fold(0u64, |sum, value| sum.checked_add(*value))?;
    Some((total, values[3].checked_add(*values.get(4).unwrap_or(&0))?))
}
pub fn busy(previous: (u64, u64), next: (u64, u64)) -> Option<f64> {
    let total = next.0.checked_sub(previous.0)?; let idle = next.1.checked_sub(previous.1)?;
    if total == 0 || idle > total { return None; }
    Some((total - idle) as f64 * 100.0 / total as f64)
}
pub fn daemon(boot: &str) -> Result<()> {
    // Separate singleton lock; short-lived control locks let WebUI requests coexist.
    { let _ = engine(boot)?; }
    let lock_path = Path::new(DIRECTORY).join("daemon.lock");
    if fs::symlink_metadata(&lock_path).is_ok_and(|meta| !meta.is_file() || meta.file_type().is_symlink()) { return Err("Invalid supervisor lock".into()); }
    let Some(_daemon_lock) = crate::lock::FileLock::acquire(&lock_path)? else { return Ok(()); };
    fs::set_permissions(DIRECTORY, fs::Permissions::from_mode(0o700))?;
    let mut previous_cpu = None;
    let mut last_error = String::new();
    let mut last_sample_event = 0;
    let mut wake_events = crate::events::WakeEvents::new();
    loop {
        let ctx = context();
        let now = uptime()?;
        let e = match engine(boot) { Ok(e) => e, Err(_) => { std::thread::sleep(Duration::from_secs(1)); continue; } };
        let stopping = !Path::new(MODULE).is_dir() || ["disable", "remove"].iter().any(|file| Path::new(MODULE).join(file).exists()) || Path::new(DIRECTORY).join("stop").exists();
        let cfg = config();
        let mut state = match state(boot) { Ok(state) => state, Err(error) => {
            event(&Config::default(), "error", &error.to_string());
            // Corrupt metadata never authorizes a new write. Keep journals for explicit recovery.
            let mut fallback_state = RuntimeState { boot:boot.into(), ..Default::default() };
            let _ = cleanup_all(&e, &mut fallback_state);
            let _ = recover_refresh(&e);
            return Err(error);
        } };
        if stopping || cfg.is_err() {
            let fallback = cfg.unwrap_or_default();
            if let Err(error) = cleanup_all(&e, &mut state) { event(&fallback, "error", &error.to_string()); }
            if let Err(error) = recover_refresh(&e) { event(&fallback, "error", &error.to_string()); }
            state.leases.clear(); state.active = Mode::Balanced; state.reason = "Supervisor stopped".into(); state.sampled_at = now; save("runtime.json", &state)?;
            return Ok(());
        }
        let cfg = cfg?;
        if state.foreground != ctx.app { state.profile_retries = 0; }
        // A ROM/driver can return touch to its original state; no write is needed to release that session.
        if matches!(e.state(&Feature::Touch), Ok("Original values active; release session")) && e.apply_targets(Feature::Touch, Vec::new(), true).is_ok() {
            event(&cfg, "touch", "Touch returned to its original value; session released without a write");
        }
        let (desired, reason) = decide(&cfg, &mut state, &ctx, now);
        let result = (|| -> Result<()> {
            thermal_cleanup(&e, &mut state, &ctx, now)?;
            if e.session(&Feature::Profile)?.is_some() && !matches!(e.state(&Feature::Profile)?, "Request active" | "Off") {
                if desired == Mode::Balanced { e.preset(Mode::Balanced)?; }
                else if state.profile_retries < 3 {
                    state.profile_retries += 1;
                    e.reconcile_vendor_profile(desired)?;
                    event(&cfg, "vendor-request", "Reapplied the selected CPU QoS request; thermal policy remains unchanged");
                } else { return Err("Vendor CPU request changed repeatedly; profile paused after three retries".into()); }
            }
            if desired != state.active || (desired == Mode::Balanced && e.session(&Feature::Profile)?.is_some()) { e.preset(desired)?; }
            Ok(())
        })();
        match result {
            Ok(()) => {
                if state.active != desired || state.reason != reason { event(&cfg, "decision", &format!("{:?}: {reason}", desired)); }
                state.active = desired; state.reason = reason.into(); state.error = None; last_error.clear();
            }
            Err(error) => {
                let error = error.to_string();
                if last_error != error { event(&cfg, "error", &error); last_error = error.clone(); }
                state.blocked = true; state.error = Some(error); state.leases.clear();
                // Best effort cleanup never reasserts externally changed nodes.
                let _ = e.preset(Mode::Balanced);
                state.active = Mode::Balanced; state.reason = "Paused after a conflict or failure".into();
            }
        }
        // Display failures and CPU failures have independent state and ownership.
        match refresh(&e, &cfg, &mut state, &ctx) {
            Ok(()) => { if !state.refresh_blocked { state.refresh_error = None; } },
            Err(error) => {
                let message = error.to_string();
                if state.refresh_error.as_deref() != Some(message.as_str()) { event(&cfg, "display-error", &message); }
                state.refresh_error = Some(message); state.refresh_blocked = true;
                let _ = restore_refresh(&e, &mut state);
            }
        }
        let cpu = read_bounded(Path::new("/proc/stat"), 65536).ok().and_then(|text| cpu_counters(&text));
        let load = previous_cpu.zip(cpu).and_then(|(old, new)| busy(old, new)); previous_cpu = cpu;
        state.telemetry = telemetry(&ctx, load);
        state.telemetry["wakeSource"] = json!(wake_events.last_wake);
        state.telemetry["eventStreamRunning"] = json!(wake_events.available());
        state.telemetry["displayRequestHz"] = json!(state.refresh_hz);
        state.telemetry["displayRequestSatisfied"] = json!(state.refresh_hz.zip(ctx.hz).map(|(requested,actual)| (f64::from(requested)-actual).abs()<0.1));
        state.telemetry["cpuBackend"] = json!(if e.session(&Feature::Profile).ok().flatten().is_some_and(|journal| journal.entries.iter().any(|entry| entry.path == crate::vendor::POWERHAL)) { "powerhal-qos" } else { "cpufreq" });
        state.foreground = ctx.app; state.sampled_at = now;
        if now.saturating_sub(last_sample_event) >= if cfg.logcat { 10 } else { 30 } {
            event(&cfg, "sample", &state.telemetry.to_string()); last_sample_event = now;
        }
        save("runtime.json", &state)?;
        drop(e);
        wake_events.wait(next_wait(&state, ctx.awake == Some(true), now));
    }
}
pub fn next_wait(state: &RuntimeState, awake: bool, now: u64) -> Duration {
    let fallback = if !state.advanced_deadlines.is_empty() { 1 } else if state.tuning_requested { if awake { 2 } else { 8 } } else if awake { 5 } else { 15 };
    let seconds = state.leases.iter().map(|lease| lease.expires).chain(state.advanced_deadlines.values().copied())
        .filter(|expiry| *expiry > now).map(|expiry| expiry - now).fold(fallback, u64::min);
    Duration::from_secs(seconds.max(1))
}
pub fn diagnostics(boot: &str) -> Result<Value> {
    let e = Engine { hardware:SystemHardware, store:DiskStore::new(DIRECTORY), boot:boot.into() };
    let cfg = config().map(|cfg| json!(cfg)).unwrap_or_else(|error| json!({"error":error.to_string()}));
    let state = state(boot).map(|state| json!(state)).unwrap_or_else(|error| json!({"error":error.to_string()}));
    let events = read_bounded(&Path::new(DIRECTORY).join("events.jsonl"), 270000).unwrap_or_default();
    let events: Vec<Value> = events.lines().rev().take(100).filter_map(|line| serde_json::from_str(line).ok()).collect();
    let mut controls = e.controls();
    if let Some(user) = state.get("refresh_user").and_then(Value::as_u64).filter(|user| *user <= 99999) { controls.push(Feature::Refresh(user as u32)); }
    let sessions: Vec<Value> = controls.iter().map(|feature| match e.session(feature) { Ok(session) => json!({"control":feature.key(),"journal":session}), Err(error) => json!({"control":feature.key(),"error":error.to_string()}) }).collect();
    Ok(json!({"engine":e.status(),"sessions":sessions,"config":cfg,"runtime":state,"events":events,"supervisor":supervisor_status(boot),"compatibility":compatibility_status(),"startupLog":read_bounded(&Path::new(DIRECTORY).join("boot.log"),32768).ok(),"supervisorLog":read_bounded(&Path::new(DIRECTORY).join("supervisor.log"),32768).ok(),"kernel":read_bounded(Path::new("/proc/version"),4096).ok(),"displayPreferences":android::command("/system/bin/settings", &["get","system","peak_refresh_rate"]).ok(),"powerhalReadback":read_bounded(Path::new(crate::vendor::POWERHAL),4096).ok()}))
}
pub fn shutdown(boot: &str) -> Result<String> {
    let e = engine(boot)?;
    save("stop", &true)?;
    let cfg = config().unwrap_or_default();
    let mut state = state(boot)?;
    let cleanup = cleanup_all(&e, &mut state);
    state.leases.clear(); state.active = Mode::Balanced; state.reason = "Supervisor stopped".into();
    save("runtime.json", &state)?; event(&cfg, "shutdown", "Restored owned controls");
    cleanup?;
    Ok("Supervisor stopping.".into())
}
/// Explicit frame sample: gfxinfo is read only and never reset by butter.
pub fn frame_sample(package: &str) -> Result<Value> {
    if !android::valid_package(package) { return Err("Invalid app package".into()); }
    let text = android::command("/system/bin/dumpsys", &["gfxinfo", package, "framestats"])?;
    Ok(parse_frames(&text))
}
pub fn parse_frames(text: &str) -> Value {
    let mut columns = None; let mut durations = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        if line.starts_with("Flags,IntendedVsync,") { columns = Some(line.split(',').map(str::to_owned).collect::<Vec<_>>()); continue; }
        let Some(header) = &columns else { continue; };
        let values: Vec<_> = line.split(',').collect();
        let field = |name| -> Option<u64> { values.get(header.iter().position(|value| value == name)?).and_then(|value| value.parse().ok()) };
        if field("Flags") != Some(0) { continue; }
        if let (Some(start), Some(end)) = (field("IntendedVsync"), field("FrameCompleted")) {
            if end > start && end - start < 10_000_000_000 && seen.insert(start) { durations.push((end - start) as f64 / 1e6); }
        }
    }
    durations.sort_by(f64::total_cmp);
    let percentile = |p: usize| durations.get(durations.len().saturating_sub(1) * p / 100).copied();
    json!({"frames":durations.len(),"p50Ms":percentile(50),"p95Ms":percentile(95),"note":"Recent app-rendered frame durations; not game FPS or proof of a performance gain. Compare the same scene and temperature."})
}
pub fn launch_if_requested(boot: &str) -> Result<()> {
    { let e = engine(boot)?; recover_refresh(&e)?; }
    config()?;
    ensure_daemon()

}

pub fn guarded_apply(boot: &str, feature: Feature) -> Result<String> {
    let ctx = context();
    if ctx.temperature.is_none_or(|value| !(0..400).contains(&value)) || ctx.battery.is_none_or(|value| value <= 20) || ctx.saver != Some(false) { return Err("Advanced controls require a known battery temperature below 40°C, battery above 20%, and battery saver off".into()); }
    let charging = matches!(feature, Feature::ChargingPause | Feature::ChargingThermal | Feature::Bypass);
    if !charging && ctx.soc_millic.is_none_or(|value| value >= 75000) { return Err("Clock and GPU cooling overrides require a readable SoC temperature below 75°C".into()); }
    if (charging && ctx.online != Some(true)) || (!charging && ctx.awake != Some(true)) { return Err("Connect a charger for charging controls, or wake the screen for clock controls".into()); }
    ensure_daemon()?;
    let e = engine(boot)?;
    let other = if feature == Feature::ChargingPause { Some(Feature::Bypass) } else if feature == Feature::Bypass { Some(Feature::ChargingPause) } else { None };
    if other.as_ref().is_some_and(|other| e.session(other).is_ok_and(|session| session.is_some())) { return Err("Restore the other charging mode first".into()); }
    if feature == Feature::ChargingPause && e.session(&feature)?.is_none() {
        let raw = e.hardware.read(crate::hardware::PAUSE)?;
        if raw.split_whitespace().nth(1) != Some("0") { return Err("Charging is already paused outside butter".into()); }
    }
    let mut state = state(boot)?;
    let seconds = if matches!(feature, Feature::CpuLock | Feature::GpuLock | Feature::GpuThermal) { 60 } else { 1800 };
    state.advanced_deadlines.insert(feature.key(), uptime()? + seconds);
    save("runtime.json", &state)?;
    let result = e.apply(feature.clone(), "", false, true).and_then(|message| {
        if feature == Feature::Bypass && e.hardware.read(crate::hardware::BYPASS_ACTIVE)? != "1" { return Err("Kernel did not confirm active battery bypass".into()); }
        Ok(message)
    });
    match result {
        Ok(message) => { event(&config()?, "control", &format!("{} enabled for at most {seconds}s", feature.key())); Ok(message) }
        Err(error) => {
            let cleanup = e.apply_targets(feature.clone(), Vec::new(), true);
            let message = format!("{error}; cleanup: {}", cleanup.unwrap_or_else(|error| error.to_string()));
            event(&config().unwrap_or_default(), "error", &message);
            Err(message.into())
        }
    }
}

fn cleanup_all(e: &Engine<SystemHardware, DiskStore>, state: &mut RuntimeState) -> Result<()> {
    let mut errors = Vec::new();
    for feature in e.controls() {
        if let Err(error) = e.session(&feature).and_then(|session| if session.is_some() { e.apply_targets(feature.clone(), Vec::new(), true).map(|_| ()) } else { Ok(()) }) { errors.push(format!("{}: {error}", feature.key())); }
    }
    if let Err(error) = restore_refresh(e, state) { errors.push(error.to_string()); }
    if let Err(error) = recover_refresh(e) { errors.push(error.to_string()); }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ").into()) }
}

pub fn log_control(command: &[String], error: Option<&str>) {
    if let Ok(cfg) = config() { event(&cfg, if error.is_some() { "error" } else { "control" }, &format!("{}{}", command.join(" "), error.map_or(String::new(), |error| format!(": {error}")))); }
}

fn recover_refresh(e: &Engine<SystemHardware, DiskStore>) -> Result<()> {
    let mut errors = Vec::new();
    let mut count = 0;
    for entry in fs::read_dir(DIRECTORY)? {
        count += 1; if count > 512 { return Err("Too many state files".into()); }
        let name = entry?.file_name(); let name = name.to_string_lossy();
        if let Some(user) = name.strip_prefix("refresh-").and_then(|name| name.strip_suffix(".json")).and_then(|name| name.parse::<u32>().ok()).filter(|user| *user <= 99999) {
            if let Err(error) = e.apply_targets(Feature::Refresh(user), Vec::new(), true) { errors.push(error.to_string()); }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ").into()) }
}

pub fn supervisor_status(boot: &str) -> Value {
    let running = crate::lock::FileLock::held(&Path::new(DIRECTORY).join("daemon.lock"));
    let last = state(boot).ok().map(|state| state.sampled_at).filter(|time| *time > 0);
    json!({"running":running.as_ref().ok(),"error":running.err().map(|error| error.to_string()),"sampleAgeSeconds":last.and_then(|time| uptime().ok().map(|now| now.saturating_sub(time)))})
}
pub fn fatal(message: &str) {
    let _ = android::command("/system/bin/log", &["-p","e","-t","butter", message]);
}

pub fn compatibility_status() -> Value {
    let text = match read_bounded(Path::new("/data/adb/modules/butterd/compatibility/pif.prop"),16384) {
        Ok(text) => text, Err(error) => return json!({"error":error.to_string()}),
    };
    let fields: BTreeMap<_,_> = text.lines().filter_map(|line| line.split_once('=' )).collect();
    json!({"selectedModel":fields.get("selectedModel"),"model":fields.get("MODEL"),"fingerprint":fields.get("FINGERPRINT"),"securityPatch":fields.get("SECURITY_PATCH"),"initialApi":fields.get("DEVICE_INITIAL_SDK_INT"),"spoofBuild":fields.get("spoofBuild").map(|value| *value == "true" || *value == "1"),"spoofVendingBuild":fields.get("spoofVendingBuild").map(|value| *value == "true" || *value == "1"),"spoofProps":fields.get("spoofProps").map(|value| *value == "true" || *value == "1"),"scope":"GMS unstable and Play Store","injectionVerified":false})
}
