use anyhow::Result;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use exif::{In, Reader, Tag, Value};
use serde_json::Value as JsonValue;
use std::process::Command;
use std::time::SystemTime;
use std::{fs::File, io::BufReader, path::Path};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy)]
enum Kind {
    Photo,
    Video,
    Dvd,
    Ignore,
}

fn is_avi(path: &Path) -> bool {
    matches!(normalize_extension(path).as_deref(), Some("avi"))
}

fn file_mtime(path: &Path) -> Option<NaiveDateTime> {
    let meta = std::fs::metadata(path).ok()?;
    let modified: SystemTime = meta.modified().ok()?;
    let dt: DateTime<Local> = modified.into();
    Some(dt.naive_local())
}

fn ffprobe_creation_time(path: &Path) -> Result<Option<NaiveDateTime>> {
    let output = Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "josn", "-show_format"])
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

    if let Some(dt) = creation.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()) {
        return Ok(Some(dt.naive_local()));
    }

    Ok(None)
}

fn video_best_datetime(path: &Path) -> Result<Option<NaiveDateTime>> {
    if let Some(dt) = (!is_avi(path))
        .then(|| ffprobe_creation_time(path))
        .transpose()?
        .flatten()
    {
        return Ok(Some(dt));
    }

    Ok(file_mtime(path))
}

fn normalize_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

fn classify(path: &Path) -> Kind {
    let extension = normalize_extension(path);
    match extension.as_deref() {
        Some("jpg") | Some("jpeg") | Some("png") => Kind::Photo,
        Some("mp4") | Some("avi") | Some("mov") | Some("m4v") => Kind::Video,
        Some("vob") | Some("ifo") | Some("bup") => Kind::Dvd,
        _ => Kind::Ignore,
    }
}

fn is_jpeg(path: &Path) -> bool {
    let extension = normalize_extension(path);
    matches!(extension.as_deref(), Some("jpg") | Some("jpeg"))
}

fn parse_exif_datetime(value: &Value) -> Option<NaiveDateTime> {
    let s = match value {
        Value::Ascii(vec) if !vec.is_empty() => String::from_utf8_lossy(&vec[0]).to_string(),
        _ => return None,
    };

    let s = s.trim();
    NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S").ok()
}

fn exif_capture_datetime(path: &Path) -> Result<Option<NaiveDateTime>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let exif = Reader::new().read_from_container(&mut reader)?;

    if let Some(dt) = exif
        .get_field(Tag::DateTimeOriginal, In::PRIMARY)
        .and_then(|f| parse_exif_datetime(&f.value))
    {
        return Ok(Some(dt));
    }

    if let Some(dt) = exif
        .get_field(Tag::DateTime, In::PRIMARY)
        .and_then(|f| parse_exif_datetime(&f.value))
    {
        return Ok(Some(dt));
    }

    Ok(None)
}

fn main() -> Result<()> {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());

    let mut photos = 0u64;
    let mut photos_with_date = 0u64;
    let mut videos = 0u64;
    let mut dvds = 0u64;
    let mut ignored = 0u64;

    for entry in WalkDir::new(&root) {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("Walk error: {err}");
                continue;
            }
        };

        // check if it is a file
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        match classify(path) {
            Kind::Photo => {
                photos += 1;
                if !is_jpeg(path) {
                    continue;
                }

                match exif_capture_datetime(path) {
                    Ok(Some(dt)) => {
                        photos_with_date += 1;
                        println!("{}    {}", dt.format("%Y-%m-%d %H:%M:%S"), path.display());
                    }
                    Ok(None) => {
                        println!("(no exif date)    {}", path.display());
                    }
                    Err(err) => {
                        println!("(exif error)  {} [ {err} ]", path.display());
                    }
                }
            }
            Kind::Video => {
                videos += 1;

                match video_best_datetime(path) {
                    Ok(Some(dt)) => {
                        println!(
                            "(video) {}    {}",
                            dt.format("%Y-%m-%d %H:%M:%S"),
                            path.display()
                        );
                    }
                    Ok(None) => {
                        println!("(video) (no date)     {}", path.display());
                    }
                    Err(err) => {
                        println!("(video) (error)   {}  [ {err} ]", path.display());
                    }
                }
            }
            Kind::Dvd => dvds += 1,
            Kind::Ignore => ignored += 1,
        }
    }

    println!("Scanned: {root}");
    println!("Photos: {photos}");
    println!("With EXIF data: {photos_with_date}");
    println!("Videos: {videos}");
    println!("DVD files: {dvds}");
    println!("Ignored: {ignored}");

    Ok(())
}
