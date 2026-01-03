use anyhow::Result;
use walkdir::WalkDir;

mod classify;
mod dvd;
mod photo_exif;
mod time;
mod video_meta;

use classify::{Kind, classify, is_jpeg};

fn main() -> Result<()> {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());

    let mut photos = 0u64;
    let mut photos_with_date = 0u64;
    let mut videos = 0u64;

    for entry in WalkDir::new(&root) {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("Walk error: {err}");
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        match classify(path) {
            Kind::Photo => {
                photos += 1;

                if is_jpeg(path) {
                    match photo_exif::exif_capture_datetime(path) {
                        Ok(Some(dt)) => {
                            photos_with_date += 1;
                            println!(
                                "(photo) {}    {}",
                                dt.format("%Y-%m-%d %H:%M:%S"),
                                path.display()
                            );
                        }
                        Ok(None) => println!("(photo) (no exif date)    {}", path.display()),
                        Err(err) => println!("(photo) (exif error) {} [ {err} ]", path.display()),
                    }
                }
            }
            Kind::Video => {
                videos += 1;
                match video_meta::video_best_datetime(path) {
                    Ok(Some(dt)) => println!(
                        "(video) {}    {}",
                        dt.format("%Y-%m-%d %H:%M:%S"),
                        path.display()
                    ),
                    Ok(None) => println!("(video) (no date)    {}", path.display()),
                    Err(err) => println!("(video) (error) {} [ {err} ]", path.display()),
                }
            }
            _ => {}
        }
    }

    println!("Scanned: {root}");
    println!("Photos: {photos}");
    println!("With EXIF data: {photos_with_date}");
    println!("Videos: {videos}");

    Ok(())
}
