//! Independent foreground-app and compositor scheduling with crash-safe ownership.
use crate::{android, presets::Mode, runtime, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
const HELPER: &str = "/data/adb/modules/butterd/bin/butter-scheduler";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observed { start: String, minimum: u32, maximum: u32, group: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry { original: Observed, expected: Observed }
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct GroupEntry { original: u32, expected: u32, #[serde(default)] original_max: Option<u32>, #[serde(default)] expected_max: Option<u32> }
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Session { top_app_mode: Mode, top_app_blocked: bool, top_app: Option<GroupEntry>, boot: String, mode: Mode, blocked: bool, entries: BTreeMap<u32, Entry> }
fn read(tid: u32) -> Result<Observed> {
    let text = android::command(HELPER, &["get", &tid.to_string()])?;
    let fields: Vec<_> = text.split_whitespace().collect();
    if fields.len() != 4 { return Err("Unsupported compositor scheduler reply".into()); }
    let value = Observed { start: fields[0].into(), minimum: fields[1].parse()?, maximum: fields[2].parse()?, group: fields[3].into() };
    validate(&value)?; Ok(value)
}
fn validate(value: &Observed) -> Result<()> {
    if value.start.is_empty() || value.start.len()>20 || !value.start.bytes().all(|byte|byte.is_ascii_digit())
        || value.minimum>value.maximum || value.maximum>1024
        || !matches!(value.group.as_str(), "/"|"/foreground"|"/system-background"|"/top-app"|"/background") {
        return Err("Invalid compositor baseline".into());
    }
    Ok(())
}
fn write(tid: u32, value: &Observed) -> Result<()> {
    validate(value)?;
    android::command(HELPER, &["set", &tid.to_string(), &value.start, &value.minimum.to_string(), &value.group])?;
    if read(tid)? != *value { return Err("Compositor scheduler readback changed".into()); }
    Ok(())
}
fn target(original: &Observed, mode: Mode) -> Observed {
    let mut value = original.clone();
    match mode {
        Mode::Battery => value.minimum = value.minimum.min(110),
        Mode::Performance | Mode::Aggressive | Mode::Boost => {
            value.group = "/foreground".into();
            let floor = if mode == Mode::Aggressive { 768 } else { 512 };
            value.minimum = value.minimum.max(floor).min(value.maximum);
        },
        Mode::Balanced => (),
    }
    value
}
fn restore_owned(entry: &Entry, current: &Observed) -> Observed {
    let mut value = current.clone();
    if current.minimum == entry.expected.minimum || current.minimum == entry.original.minimum {
        value.minimum = entry.original.minimum.min(current.maximum);
    }
    if current.group == entry.expected.group || current.group == entry.original.group {
        value.group = entry.original.group.clone();
    }
    value
}
const TOP_MIN: &str = "/dev/cpuctl/top-app/cpu.uclamp.min";
const TOP_MAX: &str = "/dev/cpuctl/top-app/cpu.uclamp.max";
fn percent(raw: &str) -> Result<u32> {
    let raw = raw.trim();
    if raw == "max" { return Ok(10000); }
    let (whole, fraction) = raw.split_once('.').unwrap_or((raw, ""));
    if whole.is_empty() || !whole.bytes().all(|byte|byte.is_ascii_digit()) || fraction.len()>2 || !fraction.bytes().all(|byte|byte.is_ascii_digit()) {
        return Err("Unsupported cgroup clamp format".into());
    }
    let value = whole.parse::<u32>()?.checked_mul(100).and_then(|value|value.checked_add(match fraction.len() { 0=>0, 1=>fraction.parse::<u32>().ok().unwrap_or(0)*10, _=>fraction.parse().ok().unwrap_or(0) })).ok_or("Cgroup clamp overflow")?;
    if value>10000 { return Err("Invalid cgroup clamp percentage".into()); }
    Ok(value)
}
fn group_target(original: u32, maximum: u32, mode: Mode) -> u32 {
    let floor = if mode == Mode::Aggressive { 7500 } else { 5000 };
    original.max(floor.min(maximum))
}
fn group_restore(entry: &GroupEntry, current: u32) -> u32 {
    if current == entry.expected { entry.original } else { current }
}
fn group_value(path: &str) -> Result<u32> { percent(&crate::hardware::read_bounded(Path::new(path),128)?) }
fn group_write(path: &str, value: u32) -> Result<()> {
    if value>10000 { return Err("Invalid cgroup clamp target".into()); }
    fs::write(path,format!("{}.{:02}",value/100,value%100))?;
    if group_value(path)? != value { return Err("Top-app clamp readback changed".into()); }
    Ok(())
}
fn save(session: &Session) -> Result<()> { runtime::save("scheduler.json", session) }
fn start_time(tid: u32) -> Result<String> {
    let stat = crate::hardware::read_bounded(Path::new(&format!("/proc/{tid}/stat")), 16384)?;
    let (_, fields) = stat.rsplit_once(')').ok_or("Missing thread identity")?;
    Ok(fields.split_whitespace().nth(19).ok_or("Missing thread start time")?.into())
}
fn restore_threads(session: &mut Session) -> Result<()> {
    let mut errors = Vec::new();
    for (tid, entry) in session.entries.clone() {
        if !Path::new(&format!("/proc/{tid}")).exists() { session.entries.remove(&tid); continue; }
        if start_time(tid).is_ok_and(|start| start != entry.original.start) { session.entries.remove(&tid); continue; }
        match read(tid) {
            Ok(current) if current.start != entry.original.start => { session.entries.remove(&tid); },
            Ok(current) => {
                let value = restore_owned(&entry, &current);
                if value != current {
                    if let Err(error) = write(tid, &value) { errors.push(error.to_string()); continue; }
                }
                session.entries.remove(&tid);
            },
            Err(error) => errors.push(error.to_string()),
        }
    }
    save(session)?;
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ").into()) }
}
fn group_limits(original: u32, maximum: u32, mode: Mode, stage: u8) -> (u32, u32) {
    if mode == Mode::Battery { let ceiling=maximum*3/4; return (original.min(ceiling),ceiling); }
    let floor = group_target(original, maximum, mode);
    let minimum = match stage { 0=>floor, 1=>original.max(floor/2), 2=>original.max(floor/4), _=>original };
    (minimum, maximum)
}
fn restore_group(session: &mut Session) -> Result<()> {
    if let Some(entry) = &session.top_app {
        let current = group_value(TOP_MIN)?;
        let maximum = group_value(TOP_MAX)?;
        let minimum_target = group_restore(entry, current);
        let maximum_target = if entry.expected_max == Some(maximum) { entry.original_max.unwrap_or(maximum) } else { maximum };
        apply_group(current, maximum, minimum_target, maximum_target)?;
        session.top_app = None; save(session)?;
    }
    Ok(())
}
fn apply_group(minimum: u32, maximum: u32, target_min: u32, target_max: u32) -> Result<()> {
    if target_max > maximum { group_write(TOP_MAX, target_max)?; }
    if minimum != target_min { group_write(TOP_MIN, target_min)?; }
    if target_max < maximum { group_write(TOP_MAX, target_max)?; }
    Ok(())
}
fn reconcile_group(session: &mut Session, mode: Mode, stage: u8) -> Result<Value> {
    if session.top_app_mode != mode || mode == Mode::Balanced {
        restore_group(session)?;
        if session.top_app_mode != mode || session.top_app_blocked {
            session.top_app_mode = mode; session.top_app_blocked = false; save(session)?;
        }
    }
    if mode == Mode::Balanced { return Ok(json!({"state":"ROM defaults"})); }
    if !Path::new(TOP_MIN).is_file() || !Path::new(TOP_MAX).is_file() {
        return Ok(json!({"state":"Not exposed"}));
    }
    if session.top_app_blocked { return Err("ROM changed top-app clamps; switch presets to retry".into()); }
    let minimum = group_value(TOP_MIN)?;
    let maximum = group_value(TOP_MAX)?;
    if let Some(entry) = &session.top_app {
        if (minimum != entry.expected && minimum != entry.original)
            || entry.expected_max.zip(entry.original_max).is_some_and(|(expected,original)|maximum!=expected && maximum!=original) {
            return Err("ROM changed top-app clamps; override paused".into());
        }
    }
    let original = session.top_app.as_ref().map(|entry|entry.original).unwrap_or(minimum);
    let original_max = session.top_app.as_ref().and_then(|entry|entry.original_max).unwrap_or(maximum);
    let (expected, expected_max) = group_limits(original, original_max, mode, stage);
    if session.top_app.as_ref().is_none_or(|entry|entry.expected!=expected || entry.expected_max!=Some(expected_max)) {
        session.top_app = Some(GroupEntry{original,expected,original_max:Some(original_max),expected_max:Some(expected_max)});
        save(session)?;
    }
    apply_group(minimum,maximum,expected,expected_max)?;
    Ok(json!({"state":"Request active","minimumPercent":group_value(TOP_MIN)? as f64/100.0,"maximumPercent":group_value(TOP_MAX)? as f64/100.0}))
}
fn tids() -> Result<Vec<u32>> {
    let pid: u32 = android::command("/system/bin/pidof", &["surfaceflinger"])?.parse()?;
    let mut result = vec![pid];
    for item in fs::read_dir(format!("/proc/{pid}/task"))? {
        let item = item?;
        if fs::read_to_string(item.path().join("comm")).unwrap_or_default().trim()=="RenderEngine" {
            if let Some(tid)=item.file_name().to_str().and_then(|name|name.parse::<u32>().ok()) { result.push(tid); }
        }
    }
    result.sort_unstable();result.dedup();
    if result.len()>8 { return Err("Unsupported compositor thread count".into()); }
    Ok(result)
}
fn reconcile_threads(session: &mut Session, mode: Mode, rom_managed: bool) -> Result<Value> {
    if session.mode != mode || mode == Mode::Balanced || rom_managed {
        if !session.entries.is_empty() { restore_threads(session)?; }
        if session.mode != mode || session.blocked { session.mode = mode; session.blocked = false; save(session)?; }
    }
    if rom_managed {
        let mut observed=Vec::new();
        for tid in tids()? { observed.push(json!({"tid":tid,"actual":read(tid)?})); }
        return Ok(json!({"state":"Managed by HyperOS","threads":observed}));
    }
    if mode == Mode::Balanced { return Ok(json!({"state":"ROM defaults"})); }
    if session.blocked { return Err("Compositor scheduling paused after an external change; switch presets to retry".into()); }
    let tids=tids()?;
    for tid in session.entries.keys().copied().collect::<Vec<_>>() {
        if tids.contains(&tid) && read(tid)?.start != session.entries[&tid].original.start { session.entries.remove(&tid); }
    }
    if session.entries.keys().any(|tid|!tids.contains(tid)) { restore_threads(session)?; }
    for tid in tids {
        let current = read(tid)?;
        let original = session.entries.get(&tid).map(|entry|entry.original.clone()).unwrap_or_else(||current.clone());
        if let Some(entry) = session.entries.get(&tid) {
            if current != entry.expected && current != entry.original { return Err("ROM changed compositor scheduling; override paused".into()); }
        }
        let expected = target(&original, mode);
        if session.entries.contains_key(&tid) && current == expected { continue; }
        session.entries.insert(tid, Entry { original, expected: expected.clone() });
        save(session)?; // Record ownership before either kernel write.
        if current != expected { write(tid, &expected)?; }
    }
    Ok(json!({"state":"Request active"}))
}
pub fn reconcile(boot: &str, mode: Mode) -> Result<Value> { reconcile_cooled(boot,mode,0) }
pub fn reconcile_cooled(boot: &str, mode: Mode, stage: u8) -> Result<Value> {
    if stage>3 { return Err("Invalid scheduler cooling stage".into()); }
    let mut session: Session = runtime::load("scheduler.json")?;
    if session.boot != boot { session = Session { boot: boot.into(), ..Default::default() }; }
    if session.entries.keys().any(|tid| !(1..=4194304).contains(tid)) || session.entries.len()>8 { return Err("Invalid compositor journal size".into()); }
    for entry in session.entries.values() {
        validate(&entry.original)?; validate(&entry.expected)?;
        if entry.original.start != entry.expected.start { return Err("Invalid compositor journal identity".into()); }
    }
    if session.top_app.as_ref().is_some_and(|entry| entry.original>10000 || entry.expected>10000 || entry.original_max.is_some_and(|value|value>10000) || entry.expected_max.is_some_and(|value|value>10000)) {
        return Err("Invalid top-app clamp journal".into());
    }
    // Each controller owns its own failure state and restores only its own writes.
    let top_app = match reconcile_group(&mut session, mode, stage) {
        Ok(value)=>value,
        Err(error)=>{
            session.top_app_blocked=true; save(&session)?;
            let cleanup=restore_group(&mut session).err().map(|error|error.to_string());
            json!({"state":"Paused","error":error.to_string(),"cleanupError":cleanup})
        }
    };
    let rom_managed=crate::vendor::dynamic_policy(&crate::hardware::SystemHardware);
    let compositor = match reconcile_threads(&mut session,mode,rom_managed) {
        Ok(value)=>value,
        Err(error)=>{
            session.blocked=true; save(&session)?;
            let cleanup=restore_threads(&mut session).err().map(|error|error.to_string());
            json!({"state":"Paused","error":error.to_string(),"cleanupError":cleanup})
        }
    };
    if mode==Mode::Balanced && (!session.entries.is_empty() || session.top_app.is_some()) {
        return Err("Some scheduler requests could not be restored; journal retained".into());
    }
    Ok(json!({"mode":mode,"topApp":top_app,"compositor":compositor}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn group_clamps_preserve_stock_minimums_caps_and_external_changes() {
        assert_eq!(percent("40.00\n").unwrap(),4000);
        assert_eq!(percent("75.5").unwrap(),7550);
        assert_eq!(percent("max").unwrap(),10000);
        for raw in ["-1", "101", "40.001", "NaN", "", ".5"] { assert!(percent(raw).is_err()); }
        assert_eq!(group_target(4000,10000,Mode::Performance),5000);
        assert_eq!(group_target(4000,10000,Mode::Aggressive),7500);
        assert_eq!(group_target(8000,10000,Mode::Performance),8000);
        assert_eq!(group_target(4000,6000,Mode::Aggressive),6000);
        assert_eq!(group_target(4000,3000,Mode::Aggressive),4000);
        let entry=GroupEntry{original:4000,expected:7500,original_max:Some(10000),expected_max:Some(10000)};
        assert_eq!(group_restore(&entry,7500),4000);
        assert_eq!(group_restore(&entry,6000),6000);
    }
    #[test]
    fn cooling_and_saver_do_not_compound_or_raise_existing_limits() {
        assert_eq!(group_limits(0,10000,Mode::Performance,0),(5000,10000));
        assert_eq!(group_limits(0,10000,Mode::Performance,1),(2500,10000));
        assert_eq!(group_limits(0,10000,Mode::Aggressive,2),(1875,10000));
        assert_eq!(group_limits(0,6000,Mode::Battery,0),(0,4500));
        assert_eq!(group_limits(8000,6000,Mode::Battery,0),(4500,4500));
        let old:Session=serde_json::from_str(r#"{"mode":"performance","top_app":{"original":4000,"expected":5000}}"#).unwrap();
        assert_eq!(old.top_app.unwrap().original_max,None);
        assert_eq!(old.top_app_mode,Mode::Balanced);
    }
    fn original() -> Observed { Observed { start:"123".into(), minimum:328, maximum:1024, group:"/system-background".into() } }
    #[test]
    fn presets_and_restore_preserve_external_votes_and_ceiling() {
        let base=original();
        let saver=target(&base,Mode::Battery);
        assert_eq!(saver.minimum,110); assert_eq!(saver.group,base.group);
        let performance=target(&base,Mode::Performance);
        assert_eq!(performance.minimum,512); assert_eq!(performance.group,"/foreground");
        assert_eq!(target(&base,Mode::Aggressive).minimum,768);
        assert_eq!(target(&Observed{minimum:900,..base.clone()},Mode::Performance).minimum,900);
        assert_eq!(target(&Observed{maximum:600,..base.clone()},Mode::Aggressive).minimum,600);
        let entry=Entry{original:base.clone(),expected:performance.clone()};
        assert_eq!(restore_owned(&entry,&performance),base);
        let external=Observed{minimum:200,maximum:800,..performance};
        let restored=restore_owned(&entry,&external);
        assert_eq!(restored.minimum,200);assert_eq!(restored.maximum,800);assert_eq!(restored.group,base.group);
        assert_eq!(target(&Observed{minimum:64,..base},Mode::Battery).minimum,64);
    }
}
