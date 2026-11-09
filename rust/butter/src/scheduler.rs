//! Preset-scoped compositor scheduling with thread identities and crash-safe ownership.
use crate::{android, presets::Mode, runtime, Result};
use serde::{Deserialize, Serialize};
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
struct GroupEntry { original: u32, expected: u32 }
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Session { top_app: Option<GroupEntry>, boot: String, mode: Mode, blocked: bool, entries: BTreeMap<u32, Entry> }
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
fn group_write(value: u32) -> Result<()> {
    if value>10000 { return Err("Invalid cgroup clamp target".into()); }
    fs::write(TOP_MIN,format!("{}.{:02}",value/100,value%100))?;
    if group_value(TOP_MIN)? != value { return Err("Top-app clamp readback changed".into()); }
    Ok(())
}
fn save(session: &Session) -> Result<()> { runtime::save("scheduler.json", session) }
fn start_time(tid: u32) -> Result<String> {
    let stat = crate::hardware::read_bounded(Path::new(&format!("/proc/{tid}/stat")), 16384)?;
    let (_, fields) = stat.rsplit_once(')').ok_or("Missing thread identity")?;
    Ok(fields.split_whitespace().nth(19).ok_or("Missing thread start time")?.into())
}
fn restore(session: &mut Session) -> Result<()> {
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
    if let Some(entry) = &session.top_app {
        let result = (|| -> Result<()> {
            let current = group_value(TOP_MIN)?;
            let value = group_restore(entry, current);
            if value != current { group_write(value)?; }
            Ok(())
        })();
        match result { Ok(()) => session.top_app = None, Err(error) => errors.push(error.to_string()) }
    }
    save(session)?;
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ").into()) }
}
pub fn reconcile(boot: &str, mode: Mode) -> Result<()> {
    let mut session: Session = runtime::load("scheduler.json")?;
    if session.boot != boot { session = Session { boot: boot.into(), ..Default::default() }; }
    if session.entries.keys().any(|tid| !(1..=4194304).contains(tid)) || session.entries.len()>8 { return Err("Invalid compositor journal size".into()); }
    for entry in session.entries.values() {
        validate(&entry.original)?; validate(&entry.expected)?;
        if entry.original.start != entry.expected.start { return Err("Invalid compositor journal identity".into()); }
    }
    if session.top_app.as_ref().is_some_and(|entry|entry.original>10000 || entry.expected>10000) { return Err("Invalid top-app clamp journal".into()); }
    if mode == Mode::Balanced && session.top_app.is_none() && session.entries.is_empty() && session.mode == mode && !session.blocked { return Ok(()); }
    if session.mode != mode || mode == Mode::Balanced {
        restore(&mut session)?;
        session.mode = mode; session.blocked = false; save(&session)?;
    }
    if mode == Mode::Balanced { return Ok(()); }
    if session.blocked { return Err("Compositor scheduling paused after an external change; switch presets to retry".into()); }
    let result = (|| -> Result<()> {
        let pid = android::command("/system/bin/pidof", &["surfaceflinger"])?;
        let pid: u32 = pid.parse()?;
        let mut tids = vec![pid];
        for item in fs::read_dir(format!("/proc/{pid}/task"))? {
            let item = item?;
            let comm = fs::read_to_string(item.path().join("comm")).unwrap_or_default();
            if comm.trim() == "RenderEngine" {
                if let Some(tid) = item.file_name().to_str().and_then(|name| name.parse::<u32>().ok()) { tids.push(tid); }
            }
        }
        tids.sort_unstable(); tids.dedup();
        if tids.len()>8 { return Err("Unsupported compositor thread count".into()); }
        // Retire dead/reused thread IDs before capturing a restarted compositor.
        for tid in session.entries.keys().copied().collect::<Vec<_>>() {
            if !tids.contains(&tid) { continue; }
            if read(tid)?.start != session.entries[&tid].original.start { session.entries.remove(&tid); }
        }
        if session.entries.keys().any(|tid|!tids.contains(tid)) { restore(&mut session)?; }
        if matches!(mode,Mode::Performance|Mode::Aggressive|Mode::Boost) && Path::new(TOP_MIN).is_file() && Path::new(TOP_MAX).is_file() {
            let current = group_value(TOP_MIN)?;
            let original = session.top_app.as_ref().map(|entry|entry.original).unwrap_or(current);
            if session.top_app.as_ref().is_some_and(|entry|current!=entry.expected && current!=entry.original) {
                return Err("ROM changed the top-app clamp; override paused".into());
            }
            let expected = group_target(original, group_value(TOP_MAX)?, mode);
            if session.top_app.is_none() || current!=expected {
                session.top_app=Some(GroupEntry{original,expected});save(&session)?;
                if current!=expected {group_write(expected)?;}
            }
        }
        for tid in tids {
            let current = read(tid)?;
            let original = session.entries.get(&tid).map(|entry|entry.original.clone()).unwrap_or_else(||current.clone());
            if let Some(entry) = session.entries.get(&tid) {
                if current != entry.expected && current != entry.original { return Err("ROM changed compositor scheduling; override paused".into()); }
            }
            let expected = target(&original, mode);
            if session.entries.contains_key(&tid) && current == expected { continue; }
            session.entries.insert(tid, Entry { original, expected: expected.clone() });
            save(&session)?; // Record ownership before either kernel write.
            if current != expected { write(tid, &expected)?; }
        }
        Ok(())
    })();
    if result.is_err() { session.blocked = true; let _ = save(&session); let _ = restore(&mut session); }
    result
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
        let entry=GroupEntry{original:4000,expected:7500};
        assert_eq!(group_restore(&entry,7500),4000);
        assert_eq!(group_restore(&entry,6000),6000);
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
