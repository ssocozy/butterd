//! Scoped XiaomiParts touch-service handover; no package or component disabling.
use crate::{android,hardware::{self,Hardware,SystemHardware},Result};
use serde::{Deserialize,Serialize};
use serde_json::json;
pub const NODE: &str = "rom-touch:0:xiaomi-parts";
pub const COMPONENT: &str = "com.xiaomi.settings/.touchsampling.TouchSamplingService";
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot { pub running: bool, pub value: u8 }
pub fn normalize(raw: &str) -> Result<String> {
    if raw.len()>128 { return Err("Touch ownership state exceeds its limit".into()); }
    let value:Snapshot=serde_json::from_str(raw)?;
    if value.value>1 || (value.running && value.value != 0) { return Err("Invalid HTSR state".into()); }
    Ok(serde_json::to_string(&value)?)
}
pub fn running(text: &str) -> Result<bool> {
    if !text.contains("ACTIVITY MANAGER SERVICES") || text.contains("Permission Denial") { return Err("Touch service state could not be verified".into()); }
    let mut matched=false;
    for line in text.lines() {
        if line.contains("* ServiceRecord{") { matched=line.contains(COMPONENT) && line.contains(" u0 "); }
        if matched && line.trim_start().starts_with("app=ProcessRecord{") { return Ok(true); }
    }
    Ok(false)
}
fn service_running() -> Result<bool> {
    running(&android::command("/system/bin/dumpsys",&["activity","services",COMPONENT])?)
}
fn snapshot() -> Result<Snapshot> {
    let value=SystemHardware.read(hardware::HTSR)?.parse::<u8>()?;
    if value>1 { return Err("Invalid HTSR readback".into()); }
    let running=service_running()?;
    // Once the ROM owns the node, its current app policy is authoritative.
    Ok(Snapshot{running,value:if running { 0 } else { value }})
}
pub fn read() -> Result<String> { Ok(serde_json::to_string(&snapshot()?)?) }
pub fn stop_reply(result:Result<String>) -> Result<String> {
    match result {
        Err(error) if error.downcast_ref::<android::CommandFailure>().is_some_and(|failure|
            failure.binary == "/system/bin/am" && failure.code == Some(255)
            && failure.stderr.trim() == "Service stopped"
            && failure.stdout.starts_with("Stopping service:")) => {
            // AOSP runStopService returns -1 even after a successful stop.
            // The caller must still verify the service is actually absent.
            Ok("Service stopped".into())
        }
        result=>result,
    }
}
fn service(start: bool) -> Result<()> {
    let action=if start { "startservice" } else { "stopservice" };
    let result=android::command("/system/bin/am",&[action,"--user","0","-n",COMPONENT]);
    let reply=if start { result? } else { stop_reply(result)? };
    if reply.contains("Error") || reply.contains("Exception") || reply.contains("not found") { return Err(format!("Touch service handover rejected: {reply}").into()); }
    for _ in 0..10 {
        if service_running()? == start { return Ok(()); }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("Touch service did not reach the requested state".into())
}
fn apply(target: &Snapshot) -> Result<()> {
    if !target.running && service_running()? {
        service(false)?;
        // onDestroy releases HTSR; let its last write settle before our request.
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    if target.running {
        if !service_running()? { service(true)?; }
    } else { SystemHardware.write(hardware::HTSR,&target.value.to_string())?; }
    if snapshot()? != *target { return Err("Touch ownership or HTSR changed during handover".into()); }
    Ok(())
}
pub fn write(raw: &str) -> Result<()> {
    let target:Snapshot=serde_json::from_str(&normalize(raw)?)?;
    let before=snapshot()?;
    match apply(&target) {
        Ok(())=>Ok(()),
        Err(error)=> {
            // Do not take a newly restarted service away from the ROM during rollback.
            let rollback:Result<()>=if service_running()? && !before.running { Err("ROM reclaimed touch control; no rollback write made".into()) } else { apply(&before) };
            Err(format!("Touch handover failed: {error}; rollback: {}",rollback.map(|_|"complete".to_string()).unwrap_or_else(|error|error.to_string())).into())
        }
    }
}
pub fn target(hardware: &impl Hardware, setting: &str) -> Result<String> {
    normalize(&hardware.read(NODE)?)?;
    let value=match setting { "high"|"480"=>1,"240"=>0,_=>return Err("Choose 240, 480 or high".into()) };
    Ok(json!({"running":false,"value":value}).to_string())
}
