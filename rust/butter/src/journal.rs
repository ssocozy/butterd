use crate::{
    hardware::read_bounded,
    model::{valid_policy, Feature},
    Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: String,
    pub original: String,
    pub expected: String,
    pub before: Option<String>,
}
impl Entry {
    pub fn owned(&self, actual: &str) -> bool {
        actual == self.expected || self.before.as_deref() == Some(actual)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub schema: u32,
    pub boot: String,
    pub feature: Feature,
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub cpu_limits: std::collections::BTreeMap<String,String>,
    #[serde(default)]
    pub observed_limits: std::collections::BTreeMap<String,String>,
}
pub trait Store {
    fn load(&self, feature: &Feature) -> Result<Option<Journal>>;
    fn save(&self, journal: &Journal) -> Result<()>;
    fn remove(&self, feature: &Feature) -> Result<()>;
}
pub struct DiskStore {
    directory: PathBuf,
    _lock: Option<crate::lock::FileLock>,
}
impl DiskStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            _lock: None,
        }
    }
    pub fn locked(directory: impl Into<PathBuf>) -> Result<Self> {
        let mut store = Self::new(directory);
        fs::create_dir_all(&store.directory)?;
        let metadata = fs::symlink_metadata(&store.directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("Invalid session directory".into());
        }
        fs::set_permissions(&store.directory, fs::Permissions::from_mode(0o700))?;
        let path = store.directory.join("controls.lock");
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Invalid session lock".into());
        }
        let lock = crate::lock::FileLock::acquire(&path)
            .map_err(|error| format!("Cannot lock {}: {error} (OS error {:?})", path.display(), error.raw_os_error()))?
            .ok_or(crate::lock::Busy)?;
        store._lock = Some(lock);
        Ok(store)
    }
    /// Interactive requests tolerate a short supervisor transaction without failing a tap.
    pub fn locked_wait(directory: impl Into<PathBuf>, timeout: std::time::Duration) -> Result<Self> {
        let directory = directory.into();
        let started = std::time::Instant::now();
        loop {
            match Self::locked(directory.clone()) {
                Ok(store) => return Ok(store),
                Err(error) if error.downcast_ref::<crate::lock::Busy>().is_some() && started.elapsed() < timeout => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(error) => return Err(error),
            }
        }
    }
    fn path(&self, feature: &Feature) -> Result<PathBuf> {
        feature.validate()?;
        Ok(self.directory.join(format!("{}.json", feature.key())))
    }
    fn legacy(&self, feature: &Feature) -> Result<PathBuf> {
        feature.validate()?;
        Ok(match feature {
            Feature::Governor(policy) => self.directory.join(policy),
            _ => self.directory.join(format!("{}.properties", feature.key())),
        })
    }
}
fn regular_text(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Invalid journal file".into());
    }
    read_bounded(path, 32768)
}
// Reads the previous Java/shell format without evaluating content or accepting arbitrary fields.
pub fn decode_legacy(feature: &Feature, text: &str) -> Result<Journal> {
    feature.validate()?;
    if text.len() > 32768 {
        return Err("Legacy journal exceeds its limit".into());
    }
    if let Feature::Governor(policy) = feature {
        if !valid_policy(policy) {
            return Err("Invalid CPU policy".into());
        }
        let fields: Vec<_> = text.split_whitespace().collect();
        if fields.len() != 3 {
            return Err("Invalid legacy governor journal".into());
        }
        return Ok(Journal {
            cpu_limits: Default::default(),
            observed_limits: Default::default(),
            schema: 1,
            boot: fields[0].into(),
            feature: feature.clone(),
            entries: vec![Entry {
                path: format!("/sys/devices/system/cpu/cpufreq/{policy}/scaling_governor"),
                original: fields[1].into(),
                expected: fields[2].into(),
                before: None,
            }],
        });
    }
    let mut values = HashMap::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
    {
        let (key, value) = line.split_once('=').ok_or("Invalid legacy journal field")?;
        if key.contains('\\') || value.contains('\\') || values.insert(key, value).is_some() {
            return Err("Ambiguous legacy journal".into());
        }
    }
    let get = |key: &str| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| format!("Missing legacy field: {key}"))
    };
    let count: usize = get("count")?.parse()?;
    if !(1..=16).contains(&count) {
        return Err("Invalid legacy entry count".into());
    }
    let mut entries = Vec::new();
    for index in 0..count {
        entries.push(Entry {
            path: get(&format!("path.{index}"))?.into(),
            original: get(&format!("original.{index}"))?.into(),
            expected: get(&format!("expected.{index}"))?.into(),
            before: match values.get(format!("pending.{index}").as_str()).copied() {
                Some("true") => Some(get(&format!("before.{index}"))?.into()),
                Some("false") | None => None,
                _ => return Err("Invalid legacy pending state".into()),
            },
        });
    }
    Ok(Journal {
        cpu_limits: Default::default(),
            observed_limits: Default::default(),
        schema: 1,
        boot: get("boot")?.into(),
        feature: feature.clone(),
        entries,
    })
}
impl Store for DiskStore {
    fn load(&self, feature: &Feature) -> Result<Option<Journal>> {
        let path = self.path(feature)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => return Ok(Some(serde_json::from_str(&regular_text(&path)?)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let legacy = self.legacy(feature)?;
        match fs::symlink_metadata(&legacy) {
            Ok(_) => Ok(Some(decode_legacy(feature, &regular_text(&legacy)?)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    fn save(&self, journal: &Journal) -> Result<()> {
        if self._lock.is_none() {
            return Err("Session writes require an exclusive lock".into());
        }
        let destination = self.path(&journal.feature)?;
        let bytes = serde_json::to_vec(journal)?;
        if bytes.len() > 32768 {
            return Err("Journal exceeds its limit".into());
        }
        // The process holds the exclusive directory lock. create_new refuses stale/symlink targets.
        let temporary = self
            .directory
            .join(format!("journal-{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        let result = (|| -> Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &destination)?;
            File::open(&self.directory)?.sync_all()?;
            Ok(())
        })();
        if temporary.exists() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
    fn remove(&self, feature: &Feature) -> Result<()> {
        if self._lock.is_none() {
            return Err("Session writes require an exclusive lock".into());
        }
        for path in [self.path(feature)?, self.legacy(feature)?] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        File::open(&self.directory)?.sync_all()?;
        Ok(())
    }
}
