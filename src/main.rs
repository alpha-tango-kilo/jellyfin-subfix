use std::{env, io, path::Path};

use camino::{Utf8Path, Utf8PathBuf};
use log::{debug, error, info, warn};
use walkdir::WalkDir;

fn main() {
    env_logger::builder().format_timestamp(None).init();
    debug!("Hello, world!");
    env::args().skip(1).for_each(|arg| {
        let path = Utf8PathBuf::from(arg);
        if path.is_dir() {
            if let Err(why) = process(&path) {
                error!("failed to process {path}: {why}");
            }
        } else {
            error!("{path} is not a folder, ignoring");
        }
    });
}

fn process(path: impl AsRef<Utf8Path>) -> anyhow::Result<()> {
    info!("discovering video files in {}", path.as_ref());
    let videos = discover_videos(path.as_ref());
    info!("videos in {}: {videos:?}", path.as_ref());
    Ok(())
}

fn discover_videos(in_dir: impl AsRef<Utf8Path>) -> Vec<Utf8PathBuf> {
    WalkDir::new(in_dir.as_ref())
        .min_depth(1)
        .max_depth(1)
        .contents_first(true)
        .into_iter()
        .filter_entry(predicates::is_video)
        .filter_map(|dir_entry| match dir_entry {
            Ok(ent) => Some(ent.path().to_owned()),
            Err(why) => {
                warn!("{why}");
                None
            },
        })
        .filter_map(|path| match Utf8PathBuf::try_from(path.clone()) {
            Ok(path) => Some(path),
            Err(_) => {
                warn!("skipped non-UTF-8 path {}", path.display());
                None
            },
        })
        .collect()
}

mod predicates {
    use walkdir::DirEntry;

    const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv"];

    pub fn is_video(dir_entry: &DirEntry) -> bool {
        dir_entry.file_type().is_file()
            && dir_entry
                .path()
                .extension()
                .map(|ext| {
                    VIDEO_EXTENSIONS
                        .iter()
                        .any(|vid_ext| ext.eq_ignore_ascii_case(vid_ext))
                })
                .unwrap_or_default()
    }
}

#[cfg(unix)]
fn symlink(
    actual_file: impl AsRef<Path>,
    link_here: impl AsRef<Path>,
) -> io::Result<()> {
    use std::os::unix::fs;
    fs::symlink(actual_file, link_here)
}

#[cfg(windows)]
fn symlink(
    actual_file: impl AsRef<Path>,
    link_here: impl AsRef<Path>,
) -> io::Result<()> {
    use std::os::windows::fs;
    assert!(std::fs::metadata(actual_file.as_ref())?.is_file());
    fs::symlink_file(actual_file, link_here)
}
