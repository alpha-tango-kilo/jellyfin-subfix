use std::{env, io, path::Path};

use anyhow::bail;
use camino::{Utf8Path, Utf8PathBuf};
use env_logger::Env;
use log::{debug, error, info, warn, LevelFilter};
use walkdir::WalkDir;

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .parse_env(Env::new().filter("SUBFIX_LOG"))
        .format_timestamp(None)
        .init();
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
    match videos.len() {
        0 => bail!("didn't find any videos in {}", path.as_ref()),
        1 => info!("found {}", &videos[0]),
        _ => {
            info!("videos in {}: {videos:?}", path.as_ref());
            if !predicates::different_versions_same_media(videos.iter()) {
                bail!(
                    "unsure that all videos are different versions of the \
                     same thing"
                );
            }
            debug!(
                "verified all videos are different versions of the same thing"
            );
        },
    }
    let subs = discover_subtitles(path.as_ref());
    if subs.is_empty() {
        info!("no subtitles found in {}, nothing to do", path.as_ref());
        return Ok(());
    }
    info!("subtitles in {}: {subs:?}", path.as_ref());
    Ok(())
}

fn discover_videos(in_dir: impl AsRef<Utf8Path>) -> Vec<Utf8PathBuf> {
    WalkDir::new(in_dir.as_ref())
        .min_depth(1)
        .max_depth(1)
        .contents_first(true)
        .into_iter()
        .filter_map(|dir_entry| match dir_entry {
            Ok(dir_entry) => Some(dir_entry),
            Err(why) => {
                warn!("{why}");
                None
            },
        })
        .filter(predicates::is_video)
        .filter_map(|dir_entry| {
            match Utf8PathBuf::try_from(dir_entry.path().to_owned()) {
                Ok(path) => Some(path),
                Err(_) => {
                    warn!(
                        "skipped non-UTF-8 path {}",
                        dir_entry.path().display()
                    );
                    None
                },
            }
        })
        .collect()
}

fn discover_subtitles(in_root_dir: impl AsRef<Utf8Path>) -> Vec<Utf8PathBuf> {
    WalkDir::new(in_root_dir.as_ref())
        .min_depth(1)
        .sort_by_file_name()
        .follow_links(false)
        .into_iter()
        .filter_map(|dir_entry| match dir_entry {
            Ok(dir_entry) => Some(dir_entry),
            Err(why) => {
                warn!("{why}");
                None
            },
        })
        .filter(predicates::is_subtitle)
        .filter_map(|dir_entry| {
            match Utf8PathBuf::try_from(dir_entry.path().to_owned()) {
                Ok(path) => Some(path),
                Err(_) => {
                    warn!(
                        "skipped non-UTF-8 path {}",
                        dir_entry.path().display()
                    );
                    None
                },
            }
        })
        .collect()
}

mod predicates {
    use std::ffi::OsStr;

    use camino::Utf8PathBuf;
    use log::{error, info, trace};
    use once_cell::sync::Lazy;
    use regex::{Regex, RegexBuilder};
    use walkdir::DirEntry;

    const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv"];
    const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "vtt", "idx"];

    static QUALITY_SUFFIX_REGEX: Lazy<Regex> = Lazy::new(|| {
        RegexBuilder::new(r#" - ((720p)|(1080p)|(4K( HDR)?))$"#)
            .case_insensitive(true)
            .build()
            .unwrap()
    });

    fn ext_in(ext: &OsStr, group: &[&str]) -> bool {
        group
            .iter()
            .any(|acceptable| ext.eq_ignore_ascii_case(acceptable))
    }

    pub fn is_video(dir_entry: &DirEntry) -> bool {
        dir_entry.file_type().is_file()
            && dir_entry
                .path()
                .extension()
                .map(|ext| {
                    trace!("seeing if {ext:?} is a video extension");
                    ext_in(ext, VIDEO_EXTENSIONS)
                })
                .unwrap_or_default()
    }

    pub fn is_subtitle(dir_entry: &DirEntry) -> bool {
        trace!("testing {dir_entry:?}");
        dir_entry.file_type().is_file()
            && dir_entry
                .path()
                .extension()
                .map(|ext| {
                    trace!("seeing if {ext:?} is a subtitle extension");
                    ext_in(ext, SUBTITLE_EXTENSIONS)
                })
                .unwrap_or_default()
    }

    // Assumes files has 2 or more elements
    pub fn different_versions_same_media<'a>(
        mut files: impl Iterator<Item = &'a Utf8PathBuf>,
    ) -> bool {
        let first = files
            .next()
            .expect("files iter should have at least two elements");
        let first_name = first.file_stem().expect("file has no name");
        trace!("regexing {first_name:?}");
        let Some(name_prefix) = QUALITY_SUFFIX_REGEX.splitn(first_name, 2).next() else {
            error!("couldn't find quality suffix in {first}");
            return false;
        };
        info!("guessing movie/episode name is {name_prefix:?}");
        files.all(|file| {
            file.file_stem()
                .map(|name| name.starts_with(name_prefix))
                .unwrap_or_default()
        })
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
