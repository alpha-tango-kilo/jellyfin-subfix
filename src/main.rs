use std::{
    env, io,
    path::{Path, PathBuf},
};

use log::{debug, error, info, warn};
use walkdir::WalkDir;

fn main() {
    env_logger::builder().format_timestamp(None).init();
    debug!("Hello, world!");
    env::args_os().skip(1).for_each(|arg| {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            if let Err(why) = process(&path) {
                error!("failed to process {}: {why}", path.display());
            }
        } else {
            error!("{} is not a folder, ignoring", path.display());
        }
    });
}

fn process(path: &Path) -> anyhow::Result<()> {
    info!("discovering video files in {}", path.display());
    let videos = discover_videos(path);
    info!("videos in {}: {videos:?}", path.display());
    Ok(())
}

fn discover_videos(in_dir: impl AsRef<Path>) -> Vec<PathBuf> {
    WalkDir::new(in_dir)
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
