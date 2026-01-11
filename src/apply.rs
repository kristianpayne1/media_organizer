use crate::dvd::convert_dvd_vobs_to_single_mp4;
use crate::plan::{Action, PlannedItem};
use crate::video::ffmpeg_convert_to_mp4;
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct ApplySummary {
    pub total: u64,
    pub copied: u64,
    pub converted_video: u64,
    pub converted_dvd: u64,
    pub skipped_existing: u64,
    pub skipped_dupliace: u64,
    pub failed: u64,
}

impl ApplySummary {
    pub fn new() -> Self {
        Self {
            total: 0,
            copied: 0,
            converted_video: 0,
            converted_dvd: 0,
            skipped_existing: 0,
            skipped_dupliace: 0,
            failed: 0,
        }
    }
}

pub fn ensure_parent_dir(dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    ensure_parent_dir(dst)?;
    fs::copy(src, dst).with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
    Ok(())
}
pub fn apply_items(items: &[PlannedItem]) -> Result<ApplySummary> {
    let mut ok_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open("apply_ok.log")?;
    let mut fail_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open("apply_fail.log")?;
    let mut dup_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open("apply_duplicates_skipped.log")?;

    let mut summary = ApplySummary::new();

    for item in items {
        summary.total += 1;

        if let Some(canon) = &item.duplicate_of {
            summary.skipped_dupliace += 1;
            writeln!(dup_log, "SKIP_DUP\t{}\tdup_of={}", item.src, canon)?;
            continue;
        }

        let src = PathBuf::from(&item.src);
        let dst = PathBuf::from(&item.dst);

        if dst.exists() {
            summary.skipped_existing += 1;
            continue;
        }

        let result = match item.action {
            Action::Copy => copy_file(&src, &dst),
            Action::ConvertVideo => ffmpeg_convert_to_mp4(&src, &dst),
            Action::ConvertDvd => convert_dvd_vobs_to_single_mp4(&src, &dst),
        };

        match result {
            Ok(()) => {
                match item.action {
                    Action::Copy => summary.copied += 1,
                    Action::ConvertVideo => summary.converted_video += 1,
                    Action::ConvertDvd => summary.converted_dvd += 1,
                }
                writeln!(
                    ok_log,
                    "OK\t{:?}\t{}\t->\t{}",
                    item.action, item.src, item.dst
                )?;
            }
            Err(e) => {
                summary.failed += 1;
                writeln!(
                    fail_log,
                    "FAIL\t{:?}\t{}\t->\t{}\t[{}]",
                    item.action, item.src, item.dst, e
                )?;
            }
        }
    }

    Ok(summary)
}
