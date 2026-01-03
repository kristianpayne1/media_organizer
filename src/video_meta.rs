use anyhow::Result;
use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;
use std::{path::Path, process::Command};

use crate::time::file_mtime;

pub fn ffprobe_creation_time(path: &Path) -> Result<Option<NaiveDateTime>> {
    let output = Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "json", "-show_format"])
        .arg(path)
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let json: JsonValue = serde_json::from_slice(&output.stdout)?;
    let creation = json
        .get("format")
        .and_then(|f| f.get("tags"))
        .and_then(|t| t.get("creation_time"))
        .and_then(|v| v.as_str());

    let dt = creation
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.naive_local());

    Ok(dt)
}

pub fn video_best_datetime(path: &Path) -> Result<Option<NaiveDateTime>> {
    if let Some(dt) = ffprobe_creation_time(path)? {
        return Ok(Some(dt));
    }
    Ok(file_mtime(path))
}
