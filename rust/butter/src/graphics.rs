//! Reboot-applied renderer preference, loaded by the root manager before Zygote.
use crate::{android, hardware::read_bounded, Result};
use serde_json::{json, Value};
use std::{fs::{self, OpenOptions}, io::Write, os::unix::fs::OpenOptionsExt, path::Path};

const MODULE: &str = "/data/adb/modules/butterd";
pub const ROM_PROPERTIES: &str = "# Butterd renderer: ROM defaults.\n";
pub const VULKAN_PROPERTIES: &str = "debug.hwui.renderer=skiavk\ndebug.renderengine.backend=skiavkthreaded\n";

fn enabled_at(module: &Path) -> Result<bool> {
    let path = module.join("system.prop");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
        Ok(meta) => {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err("Invalid renderer preference file".into());
            }
            let properties = read_bounded(&path, 1024)?;
            if properties == ROM_PROPERTIES { return Ok(false); }
            if properties != VULKAN_PROPERTIES {
                return Err("Renderer properties were modified externally; preference retained".into());
            }
            Ok(true)
        }
    }
}

fn set_at(module: &Path, enabled: bool) -> Result<()> {
    let current = enabled_at(module)?;
    if current == enabled { return Ok(()); }
    let path = module.join("system.prop");
    {
        let temporary = module.join(format!("renderer-{}.tmp", std::process::id()));
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o644).open(&temporary)?;
        let result = (|| -> Result<()> {
            file.write_all(if enabled { VULKAN_PROPERTIES.as_bytes() } else { ROM_PROPERTIES.as_bytes() })?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            Ok(())
        })();
        let _ = fs::remove_file(temporary);
        result?;
    }
    fs::File::open(module)?.sync_all()?;
    Ok(())
}

pub fn set(enabled: bool) -> Result<String> {
    if enabled && !Path::new("/vendor/lib64/hw/vulkan.mali.so").is_file() {
        return Err("The Mali Vulkan driver is unavailable".into());
    }
    set_at(Path::new(MODULE), enabled)?;
    Ok("Renderer preference saved. Reboot to apply it to apps, SystemUI and SurfaceFlinger.".into())
}

pub fn status() -> Value {
    let enabled = enabled_at(Path::new(MODULE));
    let hwui = android::command("/system/bin/getprop", &["debug.hwui.renderer"]).ok();
    let composition = android::command("/system/bin/getprop", &["debug.renderengine.backend"]).ok();
    json!({
        "enabled": enabled.as_ref().copied().unwrap_or(false),
        "error": enabled.err().map(|error| error.to_string()),
        "available": Path::new("/vendor/lib64/hw/vulkan.mali.so").is_file(),
        "hwuiProperty": hwui,
        "compositionProperty": composition,
    })
}

fn composition_backend(dump: &str) -> Option<&'static str> {
    if dump.contains("RE Vulkan") || dump.lines().any(|line|line.trim()=="Vulkan device initialized: 1") { Some("Vulkan") }
    else if dump.lines().any(|line|line.trim_start().starts_with("GLES:")) { Some("OpenGL") }
    else { None }
}
fn ui_backend(dump: &str) -> Option<&'static str> {
    if dump.contains("Pipeline=Skia (Vulkan)") { Some("Vulkan") }
    else if dump.contains("Pipeline=Skia (OpenGL)") { Some("OpenGL") }
    else { None }
}
pub fn probe(now: u64) -> Value {
    let composition=android::execute("/system/bin/dumpsys", &["SurfaceFlinger"], &[], 5);
    let ui=android::execute("/system/bin/dumpsys", &["gfxinfo","com.android.systemui"], &[], 5);
    json!({"sampledAtUptime":now,
        "composition":composition.as_ref().ok().and_then(|dump|composition_backend(dump)),
        "systemUi":ui.as_ref().ok().and_then(|dump|ui_backend(dump)),
        "compositionError":composition.err().map(|error|error.to_string()),
        "systemUiError":ui.err().map(|error|error.to_string())})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renderer_selection_is_not_confused_with_running_backends() {
        assert_eq!(composition_backend("debug.renderengine.backend=skiavkthreaded"),None);
        assert_eq!(composition_backend("GLES: ARM, Mali"),Some("OpenGL"));
        assert_eq!(composition_backend(" ------------RE Vulkan (Ganesh)----------"),Some("Vulkan"));
        assert_eq!(ui_backend("Pipeline=Skia (Vulkan)"),Some("Vulkan"));
        assert_eq!(ui_backend("No process found"),None);
    }
    #[test]
    fn preference_is_reversible_and_preserves_external_changes() {
        let module = std::env::temp_dir().join(format!("butter-graphics-{}", std::process::id()));
        fs::create_dir(&module).unwrap();
        assert!(!enabled_at(&module).unwrap());
        set_at(&module, true).unwrap();
        assert!(enabled_at(&module).unwrap());
        set_at(&module, false).unwrap();
        assert_eq!(fs::read_to_string(module.join("system.prop")).unwrap(), ROM_PROPERTIES);
        fs::write(module.join("system.prop"), "unrelated=value\n").unwrap();
        assert!(set_at(&module, false).is_err());
        assert_eq!(fs::read_to_string(module.join("system.prop")).unwrap(), "unrelated=value\n");
        fs::remove_file(module.join("system.prop")).unwrap();
        std::os::unix::fs::symlink("missing", module.join("system.prop")).unwrap();
        assert!(set_at(&module, true).is_err());
        fs::remove_dir_all(module).unwrap();
    }
}
