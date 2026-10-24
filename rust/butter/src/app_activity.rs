//! On-demand app telemetry. No automatic app termination or shell interpolation.
use crate::{android, Result};
use serde_json::Value;
pub fn arguments(args: &[String]) -> Result<Vec<&str>> {
    match args {
        [] => Ok(vec!["list"]),
        [operation, confirmation, packages @ ..] if operation == "stop" && confirmation == "--confirm" && !packages.is_empty() && packages.len() <= 16 => {
            if packages.iter().any(|package| !android::valid_package(package)) { return Err("Invalid app package".into()); }
            let mut values=vec!["stop","--confirm"];values.extend(packages.iter().map(String::as_str));Ok(values)
        }
        _ => Err("Usage: activity | activity stop --confirm PACKAGE [PACKAGE…] (up to 16)".into()),
    }
}
pub fn run(args: &[String]) -> Result<Value> {
    let operation=arguments(args)?;
    let user=android::command("/system/bin/am", &["get-current-user"])?;
    if user.parse::<u32>()? > 99999 { return Err("Invalid Android user".into()); }
    let mut command=vec!["/system/bin","dev.butterd.module.AppActivity",user.as_str()];command.extend(operation);
    let raw=android::execute("/system/bin/app_process", &command, &[("CLASSPATH","/data/adb/modules/butterd/bin/catalog.dex")],40)?;
    let value:Value=serde_json::from_str(&raw)?;
    if value["apps"].as_array().is_some_and(|rows| rows.len()<=256) || value["results"].as_array().is_some_and(|rows| rows.len()<=16) { Ok(value) }
    else { Err("Unsupported app activity response".into()) }
}
