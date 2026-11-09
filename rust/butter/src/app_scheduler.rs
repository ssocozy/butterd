//! Foreground application thread clamps; never moves tasks or writes shared cgroups.
use crate::{android, presets::Mode, runtime, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, path::Path};
const HELPER:&str="/data/adb/modules/butterd/bin/butter-scheduler";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observed { start:String, minimum:u32, maximum:u32 }
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry { pid:u32, original:Observed, expected:Observed }
#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Session { boot:String, mode:Mode, entries:BTreeMap<u32,Entry> }
fn validate(value:&Observed)->Result<()> {
    if value.start.is_empty() || value.start.len()>20 || !value.start.bytes().all(|b|b.is_ascii_digit())
        || value.minimum>value.maximum || value.maximum>1024 { return Err("Invalid app thread baseline".into()); }
    Ok(())
}
fn snapshot(pid:u32)->Result<BTreeMap<u32,Observed>> {
    let raw=android::command(HELPER,&["app-get",&pid.to_string()])?;
    parse(&raw)
}
fn parse(raw:&str)->Result<BTreeMap<u32,Observed>> {
    let mut result=BTreeMap::new();
    for line in raw.lines().filter(|line|!line.trim().is_empty()) {
        let f:Vec<_>=line.split_whitespace().collect();
        if f.len()!=4 { return Err("Unsupported app scheduler reply".into()); }
        let tid=f[0].parse::<u32>()?;
        let value=Observed{start:f[1].into(),minimum:f[2].parse()?,maximum:f[3].parse()?};
        validate(&value)?;
        if !(1..=4194304).contains(&tid) || result.insert(tid,value).is_some() || result.len()>512 {
            return Err("Invalid app thread snapshot".into());
        }
    }
    Ok(result)
}
fn save(session:&Session)->Result<()> { runtime::save("app-scheduler.json",session) }
fn apply(pid:u32, changes:&[(u32,Observed,u32,u32)])->Result<()> {
    if changes.is_empty() { return Ok(()); }
    let mut args=vec!["app-set".to_string(),pid.to_string()];
    for (tid,current,target,maximum) in changes {
        args.extend([tid.to_string(),current.start.clone(),current.minimum.to_string(),target.to_string(),current.maximum.to_string(),maximum.to_string()]);
    }
    android::command(HELPER,&args.iter().map(String::as_str).collect::<Vec<_>>())?;
    let actual=snapshot(pid)?;
    for (tid,before,target,maximum) in changes {
        if actual.get(tid).is_some_and(|now|now.start==before.start && (now.minimum!=*target || now.maximum!=*maximum)) {
            return Err("App thread clamp readback changed".into());
        }
    }
    Ok(())
}
fn restore_target(entry:&Entry,current:&Observed)->(u32,u32) {
    if current.start!=entry.original.start { return (current.minimum,current.maximum); }
    let maximum=if current.maximum==entry.expected.maximum { entry.original.maximum } else { current.maximum };
    let minimum=if current.minimum==entry.expected.minimum { entry.original.minimum.min(maximum) } else {current.minimum};
    if minimum>maximum { (current.minimum,current.maximum) } else {(minimum,maximum)}
}
fn restore(session:&mut Session, keep:&BTreeSet<u32>)->Result<()> {
    let pids:BTreeSet<_>=session.entries.values().filter(|entry|!keep.contains(&entry.pid)).map(|entry|entry.pid).collect();
    for pid in pids {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            session.entries.retain(|_,entry|entry.pid!=pid); save(session)?; continue;
        }
        let current=snapshot(pid)?;
        let changes:Vec<_>=session.entries.iter().filter(|(_,entry)|entry.pid==pid).filter_map(|(tid,entry)| {
            let now=current.get(tid)?; let target=restore_target(entry,now);
            (target!=(now.minimum,now.maximum)).then(||(*tid,now.clone(),target.0,target.1))
        }).collect();
        apply(pid,&changes)?;
        session.entries.retain(|_,entry|entry.pid!=pid); save(session)?;
    }
    Ok(())
}
// New pthreads inherit their creator's clamp requests. When all matching owned
// parents have the same baseline, retain that baseline instead of recording our
// inherited request as an original value that would survive release.
fn baseline(current:&Observed, pid:u32, entries:&BTreeMap<u32,Entry>)->Observed {
    let inherited:BTreeSet<_>=entries.values().filter(|entry|entry.pid==pid
        && entry.expected.minimum==current.minimum && entry.expected.maximum==current.maximum
        && entry.original.start.parse::<u64>().ok().zip(current.start.parse::<u64>().ok()).is_some_and(|(old,new)|new>=old))
        .map(|entry|(entry.original.minimum,entry.original.maximum)).collect();
    if inherited.len()==1 {
        let (minimum,maximum)=*inherited.first().expect("one baseline");
        Observed{minimum,maximum,..current.clone()}
    } else {current.clone()}
}
fn target(original:&Observed, current:&Observed, mode:Mode, stage:u8)->(u32,u32) {
    if mode==Mode::Battery { let ceiling=original.maximum*3/4; return (original.minimum.min(ceiling),ceiling); }
    let floor=match mode { Mode::Aggressive=>768, Mode::Performance|Mode::Boost=>512, _=>0 };
    let floor=match stage {0=>floor,1=>floor/2,2=>floor/4,_=>0};
    (original.minimum.max(floor).min(current.maximum),current.maximum)
}
pub fn reconcile(boot:&str,mode:Mode,stage:u8,app:Option<&str>)->Result<Value> {
    if stage>3 { return Err("Invalid app cooling stage".into()); }
    let mut session:Session=runtime::load("app-scheduler.json")?;
    if session.boot!=boot { session=Session{boot:boot.into(),..Default::default()}; }
    if session.entries.len()>512 { return Err("App scheduler journal exceeds limit".into()); }
    for (tid,entry) in &session.entries {
        validate(&entry.original)?;validate(&entry.expected)?;
        if !(1..=4194304).contains(tid) || !(1..=4194304).contains(&entry.pid) || entry.original.start!=entry.expected.start {
            return Err("Invalid app scheduler journal identity".into());
        }
    }
    if session.mode!=mode {
        restore(&mut session,&BTreeSet::new())?; session.mode=mode; save(&session)?;
    }
    let app=app.filter(|app|android::valid_package(app)).filter(|_|matches!(mode,Mode::Performance|Mode::Aggressive|Mode::Boost|Mode::Battery));
    let mut pids=BTreeSet::new();
    if let Some(app)=app {
        match android::command("/system/bin/pidof",&[app]) {
            Ok(raw)=>for pid in raw.split_whitespace() {
                let pid=pid.parse::<u32>()?;
                if !(1..=4194304).contains(&pid) { return Err("Invalid foreground process list".into()); }
                let status=std::fs::read_to_string(format!("/proc/{pid}/status"))?;
                let uid=status.lines().find_map(|line|line.strip_prefix("Uid:")).and_then(|line|line.split_whitespace().next()).ok_or("Missing app UID")?.parse::<u32>()?;
                if !(10000..=19999).contains(&(uid%100000)) { continue; }
                if !pids.insert(pid) || pids.len()>8 { return Err("Invalid foreground process list".into()); }
            },
            Err(error) if error.downcast_ref::<android::CommandFailure>().is_some_and(|error|error.code==Some(1) && error.stdout.is_empty() && error.stderr.is_empty())=>(),
            Err(error)=>return Err(error),
        }
    }
    // Release old foreground threads before acquiring the new foreground process.
    restore(&mut session,&pids)?;
    let mut count=0;
    for pid in pids {
        let current=snapshot(pid)?;
        session.entries.retain(|tid,entry|entry.pid!=pid || current.get(tid).is_some_and(|now|now.start==entry.original.start));
        let inherited=session.entries.clone();
        let mut changes=Vec::new();
        for (tid,now) in current {
            let mut original=session.entries.get(&tid).map(|entry|entry.original.clone()).unwrap_or_else(||baseline(&now,pid,&inherited));
            if let Some(entry)=session.entries.get(&tid) {
                // Adopt external fields independently; retain the original of a
                // minimum we still own when only the ROM maximum changed.
                if now.minimum!=entry.expected.minimum { original.minimum=now.minimum; }
                if now.maximum!=entry.expected.maximum { original.maximum=now.maximum; }
                original.minimum=original.minimum.min(original.maximum);
            }
            let (minimum,maximum)=target(&original,&now,mode,stage);
            session.entries.insert(tid,Entry{pid,original,expected:Observed{minimum,maximum,..now.clone()}});
            if minimum!=now.minimum || maximum!=now.maximum { changes.push((tid,now,minimum,maximum)); }
            count+=1;
        }
        if session.entries.len()>512 { return Err("Foreground thread count exceeds limit".into()); }
        save(&session)?; // Persist ownership before the batch can partially succeed.
        apply(pid,&changes)?;
    }
    save(&session)?;
    Ok(json!({"state":if count>0 {"Per-thread requests active"}else{"ROM defaults"},"backend":"Foreground thread uclamp","app":app,"threads":count,"coolingStage":stage}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base()->Observed { Observed{start:"123".into(),minimum:0,maximum:1024} }
    #[test]
    fn task_requests_survive_group_min_reset_and_respect_caps_and_cooling() {
        let base=base();
        assert_eq!(target(&base,&base,Mode::Aggressive,0),(768,1024));
        assert_eq!(target(&base,&base,Mode::Performance,0),(512,1024));
        assert_eq!(target(&base,&base,Mode::Aggressive,1),(384,1024));
        assert_eq!(target(&base,&base,Mode::Aggressive,2),(192,1024));
        assert_eq!(target(&base,&Observed{maximum:300,..base.clone()},Mode::Aggressive,0),(300,300));
        assert_eq!(target(&Observed{minimum:900,..base.clone()},&base,Mode::Aggressive,0),(900,1024));
    }
    #[test]
    fn inherited_clamps_restore_and_saver_caps_do_not_compound() {
        let original=base();let expected=Observed{minimum:768,..original.clone()};
        let entries=BTreeMap::from([(10,Entry{pid:10,original:original.clone(),expected:expected.clone()})]);
        let child=Observed{start:"124".into(),..expected};
        assert_eq!(baseline(&child,10,&entries),Observed{start:"124".into(),..original.clone()});
        assert_eq!(baseline(&child,11,&entries),child);
        assert_eq!(target(&original,&original,Mode::Battery,0),(0,768));
        assert_eq!(target(&original,&Observed{maximum:768,..original.clone()},Mode::Battery,0),(0,768));
        let saver=Entry{pid:10,original:original.clone(),expected:Observed{maximum:768,..original.clone()}};
        assert_eq!(restore_target(&saver,&saver.expected),(0,1024));
        assert_eq!(restore_target(&saver,&Observed{maximum:600,..original}),(0,600));
    }
    #[test]
    fn release_preserves_external_votes_caps_and_reused_ids() {
        let base=base();let expected=Observed{minimum:768,..base.clone()};
        let entry=Entry{pid:10,original:base.clone(),expected:expected.clone()};
        assert_eq!(restore_target(&entry,&expected),(0,1024));
        assert_eq!(restore_target(&entry,&Observed{minimum:96,..expected.clone()}),(96,1024));
        assert_eq!(restore_target(&entry,&Observed{start:"124".into(),..expected}),(768,1024));
        assert!(parse("1 123 768 1024\n2 124 512 1024").is_ok());
        for raw in ["1 123 768 512","1 123 1 1024\n1 124 1 1024","0 123 0 1024","1 x 0 1024"] { assert!(parse(raw).is_err()); }
    }
}
