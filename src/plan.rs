use crate::classify::{Kind, classify, normalize_extension};
use crate::time::{DateSource, best_datetime_for_dvd, best_datetime_for_file, format_dt};
use crate::{deduplicate, dvd};
use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Action {
    Copy,
    ConvertVideo,
    ConvertDvd,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MediaKind {
    Photo,
    Video,
    Dvd,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlannedItem {
    pub kind: MediaKind,
    pub action: Action,
    pub src: String,
    pub dst: String,
    pub best_dt: Option<String>,
    pub date_source: DateSource,
    pub size_bytes: Option<u64>,
    pub content_hash: Option<String>,
    pub duplicate_of: Option<String>,
}

#[derive(Debug)]
pub struct PlanSummary {
    pub planned: u64,
    pub photos: u64,
    pub videos: u64,
    pub dvds: u64,
    pub missing_date: u64,
    pub need_convert_video: u64,
    pub need_convert_dvd: u64,
    pub duplicate_photos: u64,
    pub duplicate_videos: u64,
}

impl PlanSummary {
    pub fn new() -> Self {
        Self {
            planned: 0,
            photos: 0,
            videos: 0,
            dvds: 0,
            missing_date: 0,
            need_convert_video: 0,
            need_convert_dvd: 0,
            duplicate_photos: 0,
            duplicate_videos: 0,
        }
    }
}

fn safe_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string()
}

fn plan_dst(
    out_root: &Path,
    kind: MediaKind,
    src: &Path,
    best_dt: Option<NaiveDateTime>,
) -> PathBuf {
    let base = match kind {
        MediaKind::Photo => out_root.join("Photos"),
        MediaKind::Video => out_root.join("Videos"),
        MediaKind::Dvd => out_root.join("DVDs"),
    };

    let (year, ym, ymd) = if let Some(dt) = best_dt {
        let d = dt.date();
        (
            d.format("%Y").to_string(),
            d.format("%Y-%m").to_string(),
            d.format("%Y-%m-%d").to_string(),
        )
    } else {
        (
            "UnknownDate".into(),
            "UnknownDate".into(),
            "UnknownDate".into(),
        )
    };

    let dir = if year == "UnknownDate" {
        base.join("UnknownDate")
    } else {
        base.join(year).join(ym).join(ymd)
    };

    let ext = match kind {
        MediaKind::Photo => normalize_extension(src).unwrap_or_else(|| "jpg".into()),
        MediaKind::Video | MediaKind::Dvd => "mp4".into(),
    };

    let name = match kind {
        MediaKind::Dvd => src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("DVD")
            .to_string(),
        _ => safe_stem(src),
    };

    dir.join(format!("{name}.{ext}"))
}

fn action_for_video(path: &Path) -> Action {
    match normalize_extension(path).as_deref() {
        Some("avi") => Action::ConvertVideo,
        _ => Action::Copy,
    }
}

fn mark_input_duplicates(planned: &mut [PlannedItem], summary: &mut PlanSummary) -> Result<()> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for item in planned.iter() {
        match item.kind {
            MediaKind::Photo | MediaKind::Video => {
                paths.push(PathBuf::from(&item.src));
            }
            _ => {}
        }
    }

    let dup_groups = deduplicate::find_exact_duplicates(&paths)?;

    let mut duplicate_of: HashMap<String, String> = HashMap::new();
    let mut hash_of: HashMap<String, String> = HashMap::new();

    for (h, mut group) in dup_groups {
        group.sort();
        let canonical = group[0].clone();
        hash_of.insert(canonical.to_string_lossy().to_string(), h.clone());

        for p in group.into_iter().skip(1) {
            duplicate_of.insert(
                p.to_string_lossy().to_string(),
                canonical.to_string_lossy().to_string(),
            );
            hash_of.insert(p.to_string_lossy().to_string(), h.clone());
        }
    }

    for item in planned.iter_mut() {
        if !matches!(item.kind, MediaKind::Photo | MediaKind::Video) {
            continue;
        }

        let p = PathBuf::from(&item.src);
        item.size_bytes = std::fs::metadata(&p).ok().map(|m| m.len());

        if let Some(h) = hash_of.get(&item.src) {
            item.content_hash = Some(h.clone());
        }

        if let Some(canon) = duplicate_of.get(&item.src) {
            item.duplicate_of = Some(canon.clone());
            match item.kind {
                MediaKind::Photo => summary.duplicate_photos += 1,
                MediaKind::Video => summary.duplicate_videos += 1,
                _ => {}
            }
        }
    }

    Ok(())
}

pub fn build_plan(root: &Path, out_root: &Path) -> Result<(Vec<PlannedItem>, PlanSummary)> {
    let mut planned: Vec<PlannedItem> = Vec::new();
    let mut summary = PlanSummary::new();

    let mut dvd_roots: HashSet<PathBuf> = HashSet::new();

    for entry in WalkDir::new(root) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        if entry.file_type().is_dir() {
            if let Some(dvd_root) = dvd::dvd_root_from_video_ts_dir(path) {
                dvd_roots.insert(dvd_root);
            }
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        if dvd::is_inside_video_ts(path) {
            continue;
        }

        match classify(path) {
            Kind::Photo => {
                summary.photos += 1;

                let (dt, source) = best_datetime_for_file(path)?;
                if dt.is_none() {
                    summary.missing_date += 1;
                }

                let dst = plan_dst(out_root, MediaKind::Photo, path, dt);
                planned.push(PlannedItem {
                    kind: MediaKind::Photo,
                    action: Action::Copy,
                    src: path.to_string_lossy().to_string(),
                    dst: dst.to_string_lossy().to_string(),
                    best_dt: dt.map(format_dt),
                    date_source: source,
                    size_bytes: None,
                    content_hash: None,
                    duplicate_of: None,
                });

                summary.planned += 1;
            }
            Kind::Video => {
                summary.videos += 1;

                let (dt, source) = best_datetime_for_file(path)?;
                if dt.is_none() {
                    summary.missing_date += 1;
                }

                let action = action_for_video(path);
                if matches!(action, Action::ConvertVideo) {
                    summary.need_convert_video += 1;
                }

                let dst = plan_dst(out_root, MediaKind::Video, path, dt);
                planned.push(PlannedItem {
                    kind: MediaKind::Video,
                    action,
                    src: path.to_string_lossy().to_string(),
                    dst: dst.to_string_lossy().to_string(),
                    best_dt: dt.map(format_dt),
                    date_source: source,
                    size_bytes: None,
                    content_hash: None,
                    duplicate_of: None,
                });

                summary.planned += 1;
            }
            _ => {}
        }
    }

    mark_input_duplicates(&mut planned, &mut summary)?;

    for dvd_root in dvd_roots {
        summary.dvds += 1;

        let (dt, source) = best_datetime_for_dvd(&dvd_root);
        if dt.is_none() {
            summary.missing_date += 1;
        }

        let _vobs = dvd::dvd_main_title_vobs(&dvd_root)?;

        let dst = plan_dst(out_root, MediaKind::Dvd, &dvd_root, dt);
        planned.push(PlannedItem {
            kind: MediaKind::Dvd,
            action: Action::ConvertDvd,
            src: dvd_root.to_string_lossy().to_string(),
            dst: dst.to_string_lossy().to_string(),
            best_dt: dt.map(format_dt),
            date_source: source,
            size_bytes: None,
            content_hash: None,
            duplicate_of: None,
        });

        summary.need_convert_dvd += 1;
        summary.planned += 1;
    }

    Ok((planned, summary))
}
