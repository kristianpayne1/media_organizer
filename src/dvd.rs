use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn is_dvd_video_ts_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("VIDEO_TS"))
        .unwrap_or(false)
}
