use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Photo,
    Video,
    Dvd,
    Ignore,
}

pub fn normalize_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

pub fn classify(path: &Path) -> Kind {
    let extension = normalize_extension(path);
    match extension.as_deref() {
        Some("jpg") | Some("jpeg") | Some("png") => Kind::Photo,
        Some("mp4") | Some("avi") | Some("mov") | Some("m4v") => Kind::Video,
        Some("vob") | Some("ifo") | Some("bup") => Kind::Dvd,
        _ => Kind::Ignore,
    }
}

pub fn is_jpeg(path: &Path) -> bool {
    matches!(
        normalize_extension(path).as_deref(),
        Some("jpg") | Some("jpeg")
    )
}
