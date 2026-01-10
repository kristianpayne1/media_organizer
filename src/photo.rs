use anyhow::Result;
use chrono::NaiveDateTime;
use exif::{In, Reader, Tag, Value};
use std::{fs::File, io::BufReader, path::Path};

fn parse_exif_datetime(value: &Value) -> Option<NaiveDateTime> {
    let s = match value {
        Value::Ascii(vec) if !vec.is_empty() => String::from_utf8_lossy(&vec[0]).to_string(),
        _ => return None,
    };

    let s = s.trim();
    NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S").ok()
}

pub fn exif_capture_datetime(path: &Path) -> Result<Option<NaiveDateTime>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let exif_data = Reader::new().read_from_container(&mut reader).ok();

    if let Some(exif) = exif_data {
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
    }
    Ok(None)
}
