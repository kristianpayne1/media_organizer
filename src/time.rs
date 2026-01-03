use chrono::{DateTime, Local, NaiveDateTime};
use std::{path::Path, time::SystemTime};

pub fn file_mtime(path: &Path) -> Option<NaiveDateTime> {
    let meta = std::fs::metadata(path).ok()?;
    let modified: SystemTime = meta.modified().ok()?;
    let dt: DateTime<Local> = modified.into();
    Some(dt.naive_local())
}
