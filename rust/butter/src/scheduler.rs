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
struct Session { boot: String, mode: Mode, blocked: bool, entries: BTreeMap<u32, Entry> }
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
        Mode::Performance | Mode::Aggressive | Mode::Boost => value.group = "/foreground".into(),
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
    if mode == Mode::Balanced && session.entries.is_empty() && session.mode == mode && !session.blocked { return Ok(()); }
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
    fn original() -> Observed { Observed { start:"123".into(), minimum:328, maximum:1024, group:"/system-background".into() } }
    #[test]
    fn presets_and_restore_preserve_external_votes_and_ceiling() {
        let base=original();
        let saver=target(&base,Mode::Battery);
        assert_eq!(saver.minimum,110); assert_eq!(saver.group,base.group);
        let performance=target(&base,Mode::Performance);
        assert_eq!(performance.minimum,328); assert_eq!(performance.group,"/foreground");
        let entry=Entry{original:base.clone(),expected:performance.clone()};
        assert_eq!(restore_owned(&entry,&performance),base);
        let external=Observed{minimum:200,maximum:800,..performance};
        let restored=restore_owned(&entry,&external);
        assert_eq!(restored.minimum,200);assert_eq!(restored.maximum,800);assert_eq!(restored.group,base.group);
        assert_eq!(target(&Observed{minimum:64,..base},Mode::Battery).minimum,64);
    }
}
