use anyhow::{Ok, Result, ensure};
use std::path::{Path, PathBuf};

fn write_ffconcat_file(paths: &[PathBuf]) -> anyhow::Result<PathBuf> {
    use std::io::Write;

    let list_path = std::env::temp_dir().join(format!("concat_{}.ffconcat", std::process::id()));
    let mut f = std::fs::File::create(&list_path)?;

    for p in paths {
        let escaped = p.to_string_lossy().replace('\'', "'\\''");
        writeln!(f, "file '{}'", escaped)?;
    }

    Ok(list_path)
}

pub fn is_inside_video_ts(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| s.eq_ignore_ascii_case("VIDEO_TS"))
            .unwrap_or(false)
    })
}

pub fn dvd_root_from_video_ts_dir(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    if name.eq_ignore_ascii_case("VIDEO_TS") {
        return path.parent().map(|p| p.to_path_buf());
    }
    None
}

pub fn dvd_all_content_vobs(dvd_root: &Path) -> Result<Vec<PathBuf>> {
    let video_ts = dvd_root.join("VIDEO_TS");
    if !video_ts.is_dir() {
        return Ok(vec![]);
    }

    let mut vobs: Vec<PathBuf> = Vec::new();

    for entry in std::fs::read_dir(&video_ts)? {
        let entry = entry?;
        let path = entry.path();

        let is_vob = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("vob"))
            .unwrap_or(false);

        if !is_vob {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_uppercase();

        if name == "VIDEO_TS.VOB" {
            continue;
        }

        if name.starts_with("VTS_") && name.get(7..9) == Some("_0") {
            continue;
        }

        vobs.push(path);
    }

    vobs.sort();
    Ok(vobs)
}

pub fn convert_dvd_vobs_to_single_mp4(dvd_root: &Path, dst_mp4: &Path) -> Result<()> {
    let vobs = dvd_all_content_vobs(dvd_root)?;
    ensure!(!vobs.is_empty(), "no VOBs found for {}", dvd_root.display());

    // temp dir
    let work_dir = std::env::temp_dir().join(format!("dvd_parts_{}", std::process::id()));
    std::fs::create_dir_all(&work_dir)?;

    let mut ts_parts: Vec<PathBuf> = Vec::new();

    for (i, vob) in vobs.iter().enumerate() {
        let ts_path = work_dir.join(format!("part-{:03}.ts", i + 1));
        ts_parts.push(ts_path.clone());

        if ts_path.exists() {
            continue;
        }

        let status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-hide_banner",
                "-nostats",
                "-loglevel",
                "warning",
                "-fflags",
                "+genpts+igndts+discardcorrupt",
                "-err_detect",
                "ignore_err",
                "-i",
                vob.to_str().unwrap(),
                "-map",
                "0:v:0",
                "-map",
                "0:a?",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                "-f",
                "mpegts",
                ts_path.to_str().unwrap(),
            ])
            .status()?;

        ensure!(status.success(), "ffmpeg failed on VOB {}", vob.display());
    }

    if let Some(parent) = dst_mp4.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let list_path = write_ffconcat_file(&ts_parts)?;

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-nostats",
            "-loglevel",
            "warning",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            list_path.to_str().unwrap(),
            "-c",
            "copy",
            "-bsf:a",
            "aac_adtstoasc",
            "-movflags",
            "+faststart",
            dst_mp4.to_str().unwrap(),
        ])
        .status()?;

    ensure!(
        status.success(),
        "ffmpeg concat failed for DVD {}",
        dvd_root.display()
    );

    Ok(())
}
