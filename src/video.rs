use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde_json::Value as JsonValue;
use std::{path::Path, process::Command};

use crate::apply::ensure_parent_dir;

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

pub fn ffmpeg_convert_to_mp4(src: &Path, dst: &Path) -> Result<()> {
    ensure_parent_dir(dst)?;

    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            src.to_str().unwrap(),
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            dst.to_str().unwrap(),
        ])
        .status()
        .with_context(|| "failed to spawn ffmpeg")?;

    anyhow::ensure!(
        status.success(),
        "ffmpeg failed converting {}",
        src.display()
    );

    Ok(())
}
