use crate::{hardware::{self, Hardware, CPU, GOODIX, TCP, TCP_AVAILABLE}, journal::{Entry, Journal, Store}, model::{selectable_governor, valid_governor, valid_algorithm, valid_policy, Feature}, Result};
use serde_json::{json, Map, Value};

pub struct Engine<H, S> { pub hardware: H, pub store: S, pub boot: String }
impl<H: Hardware, S: Store> Engine<H, S> {
    fn checked(&self, feature: &Feature, journal: &Journal) -> Result<()> {
        if journal.schema != 1 || &journal.feature != feature || journal.boot.len() != 36 || journal.entries.is_empty() || journal.entries.len() > 16 {
            return Err("Invalid session journal".into());
        }
        let mut seen = std::collections::HashSet::new();
        for entry in &journal.entries {
            if !hardware::allowed(&self.hardware, feature, &entry.path) || !seen.insert(&entry.path) { return Err("Invalid or duplicated session path".into()); }
            hardware::validate_value(feature, &entry.original)?; hardware::validate_value(feature, &entry.expected)?;
            if let Some(before) = &entry.before { hardware::validate_value(feature, before)?; }
        }
        Ok(())
    }
    pub fn session(&self, feature: &Feature) -> Result<Option<Journal>> {
        feature.validate()?;
        let Some(journal) = self.store.load(feature)? else { return Ok(None); };
        if journal.schema != 1 || &journal.feature != feature || journal.boot.len() != 36 {
            return Err("Invalid session identity".into());
        }
        if journal.boot != self.boot { return Ok(None); }
        self.checked(feature, &journal)?;
        Ok(Some(journal))
    }
    pub fn apply(&self, feature: Feature, setting: &str, restoring: bool, accepted_risk: bool) -> Result<String> {
        feature.validate()?;
        if feature.advanced() && !restoring && !accepted_risk { return Err("Advanced controls require --accept-risk".into()); }
        let saved = self.session(&feature)?;
        if restoring && saved.is_none() { return Ok("No active session to restore.".into()); }
        let mut targets = if restoring { Vec::new() } else { hardware::plan(&self.hardware, &feature, setting)? };
        let mut journal = match saved {
            Some(journal) => journal,
            None => {
                let mut entries = Vec::new();
                for (path, _) in &targets {
                    let original = hardware::value(&self.hardware, path)?;
                    entries.push(Entry { path: path.clone(), expected: original.clone(), original, before: None });
                }
                Journal { schema: 1, boot: self.boot.clone(), feature: feature.clone(), entries }
            }
        };
        self.checked(&feature, &journal)?;
        // Preflight every node, including originals loaded from older versions, before any write.
        for entry in &journal.entries {
            let current = hardware::value(&self.hardware, &entry.path)?;
            if !entry.owned(&current) { return Err(format!("Another controller changed {}; nothing overwritten", entry.path).into()); }
            self.hardware.check_write(&entry.path)?;
            if restoring {
                if feature == Feature::Tcp && !self.hardware.read(TCP_AVAILABLE)?.split_whitespace().any(|value| value == entry.original) { return Err("The original TCP algorithm is no longer available".into()); }
                // An original governor must still be offered by the live policy.
                if let Feature::Governor(policy) = &feature {
                    let available = self.hardware.read(&format!("{CPU}/{policy}/scaling_available_governors"))?;
                    if !available.split_whitespace().any(|value| value == entry.original) { return Err("The original governor is no longer supported".into()); }
                }
                targets.push((entry.path.clone(), entry.original.clone()));
            }
            if !targets.iter().any(|(path, _)| path == &entry.path) { return Err("Capabilities changed; restore the existing session first".into()); }
        }
        if targets.len() != journal.entries.len() { return Err("Capabilities changed; restore first".into()); }
        for index in 0..journal.entries.len() {
            let path = journal.entries[index].path.clone();
            let target = targets.iter().find(|(node, _)| node == &path).ok_or("Missing target")?.1.clone();
            let current = hardware::value(&self.hardware, &path)?;
            if !journal.entries[index].owned(&current) { return Err("External change detected; session retained".into()); }
            journal.entries[index].before = Some(current);
            journal.entries[index].expected = target.clone();
            self.store.save(&journal)?;
            self.hardware.write(&path, &target)?;
            let mut verified = false;
            for attempt in 0..5 {
                if hardware::value(&self.hardware, &path)? == target { verified = true; break; }
                if attempt < 4 { std::thread::sleep(std::time::Duration::from_millis(20)); }
            }
            if !verified { return Err("Readback failed; session retained for restoration".into()); }
            journal.entries[index].before = None;
            self.store.save(&journal)?;
        }
        if restoring { self.store.remove(&feature)?; }
        Ok(if restoring { "Original settings restored." } else { "Request verified. ROM services may replace it; butter does not continuously force it." }.into())
    }
    pub fn state(&self, feature: &Feature) -> Result<&'static str> {
        let Some(journal) = self.session(feature)? else { return Ok("Off"); };
        let mut pending = false;
        for entry in &journal.entries {
            let actual = hardware::value(&self.hardware, &entry.path)?;
            pending |= entry.before.is_some();
            if !entry.owned(&actual) { return Ok("Changed by another controller"); }
        }
        Ok(if pending { "Interrupted; restore needed" } else { "Request active" })
    }
    pub fn status(&self) -> Value {
        let mut output = Map::new();
        for feature in [Feature::CpuLock, Feature::GpuLock, Feature::ChargingThermal, Feature::GpuThermal, Feature::Touch] {
            let capability = (|| -> Result<Value> {
                let targets = hardware::plan(&self.hardware, &feature, if feature == Feature::Touch { "high" } else { "" })?;
                let mut entry = json!({"available": true, "nodes": targets.len(), "state": self.state(&feature)?});
                if feature == Feature::Touch {
                    entry["driverHz"] = match hardware::value(&self.hardware, GOODIX) { Ok(value) => json!(if value == "1" { 480 } else { 240 }), Err(_) => Value::Null };
                }
                Ok(entry)
            })();
            output.insert(feature.key(), capability.unwrap_or_else(|error| json!({"available": false, "state": "Not exposed", "reason": error.to_string()})));
        }
        let mut policies = Vec::new();
        if let Ok(names) = self.hardware.list(CPU) {
            for policy in names.into_iter().filter(|value| valid_policy(value)).take(16) {
                let item = (|| -> Result<Value> {
                    let governor = self.hardware.read(&format!("{CPU}/{policy}/scaling_governor"))?;
                    if !valid_governor(&governor) { return Err("Invalid governor".into()); }
                    let choices: Vec<String> = self.hardware.read(&format!("{CPU}/{policy}/scaling_available_governors"))?.split_whitespace().filter(|value| selectable_governor(value)).map(str::to_owned).collect();
                    Ok(json!({"id": policy, "governor": governor, "cores": self.hardware.read(&format!("{CPU}/{policy}/related_cpus"))?, "choices": choices,
                        "original": self.session(&Feature::Governor(policy.clone()))?.is_some()}))
                })();
                if let Ok(item) = item { policies.push(item); }
            }
        }
        let tcp = (|| -> Result<Value> {
            let current = self.hardware.read(TCP)?;
            if !valid_algorithm(&current) { return Err("Invalid TCP algorithm".into()); }
            let choices: Vec<String> = self.hardware.read(TCP_AVAILABLE)?.split_whitespace().filter(|value| valid_algorithm(value)).map(str::to_owned).collect();
            self.hardware.check_write(TCP)?;
            Ok(json!({"available":true,"current":current,"choices":choices,"original":self.session(&Feature::Tcp)?.is_some(),"state":self.state(&Feature::Tcp)?}))
        })();
        output.insert("tcp".into(), tcp.unwrap_or_else(|error| json!({"available":false,"reason":error.to_string()})));
        output.insert("cpuPolicies".into(), json!(policies)); output.insert("engine".into(), json!({"name":"butter", "language":"Rust", "version":env!("CARGO_PKG_VERSION")}));
        Value::Object(output)
    }
}
