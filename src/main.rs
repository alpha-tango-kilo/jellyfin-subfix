use std::{env, io, num::NonZeroU8, path::Path, str::FromStr};

use anyhow::{anyhow, bail, Context};
use camino::{Utf8Path, Utf8PathBuf};
use env_logger::Env;
use isolang::Language;
use log::{debug, error, info, trace, warn, LevelFilter};
use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use walkdir::WalkDir;

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .parse_env(Env::new().filter("SUBFIX_LOG"))
        .format_timestamp(None)
        .init();
    let mut no_args = true;
    env::args().skip(1).for_each(|arg| {
        no_args = false;
        let path = Utf8PathBuf::from(arg);
        if path.is_dir() {
            if let Err(why) = process(&path) {
                error!("failed to process {path}: {why}");
            }
        } else {
            error!("{path} is not a folder, ignoring");
        }
    });
    if no_args {
        info!("assuming current directory");
        if let Err(why) = process(Utf8Path::new(".")) {
            error!("failed to process this directory: {why}");
        }
    }
}

fn process(path: impl AsRef<Utf8Path>) -> anyhow::Result<()> {
    info!("discovering video files in {}", path.as_ref());
    let path = path.as_ref();
    env::set_current_dir(path).context("failed to move into directory")?;
    let videos = discover_videos(path);
    match videos.len() {
        0 => bail!("didn't find any videos in {}", path),
        1 => info!("found {}", &videos[0].path),
        _ => {
            info!("videos in {path}: {videos:#?}");
            if !(predicates::no_series(videos.iter())
                || predicates::all_a_series(videos.iter()))
            {
                bail!("can't mix series and movies");
            }
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
    let mut subs = discover_subtitles(path);
    if subs.is_empty() {
        info!("no subtitles found in {path}, nothing to do");
        return Ok(());
    }
    info!("subtitles in {path}: {subs:#?}");
    remove_duplicate_languages(&mut subs);
    create_symlinks(path, &videos, &subs);
    info!("done!");
    Ok(())
}

fn discover_videos(in_dir: impl AsRef<Utf8Path>) -> Vec<Video> {
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
                Ok(path) => match Video::from_path(path) {
                    Ok(video) => Some(video),
                    Err(why) => {
                        warn!(
                            "skipped path {}: {why}",
                            dir_entry.path().display()
                        );
                        None
                    },
                },
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

fn discover_subtitles(in_root_dir: impl AsRef<Utf8Path>) -> Vec<Subtitle> {
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
                Ok(path) => {
                    info!("found {path}");
                    Some(path)
                },
                Err(_) => {
                    warn!(
                        "skipped non-UTF-8 path {}",
                        dir_entry.path().display()
                    );
                    None
                },
            }
        })
        .filter_map(|path| match Subtitle::new(path.clone()) {
            Ok(sub) => Some(sub),
            Err(why) => {
                warn!("failed to process {path}, skipping: {why}");
                None
            },
        })
        .collect()
}

fn create_symlinks(
    in_root_dir: impl AsRef<Utf8Path>,
    videos: &[Video],
    subtitles: &[Subtitle],
) {
    videos
        .iter()
        .flat_map(|video| {
            subtitles.iter().map(move |subtitle| (video, subtitle))
        })
        .filter(|(video, subtitle)| video.series_info == subtitle.series_info)
        .for_each(|(video, subtitle)| {
            let subtitle_name = {
                let mut path = in_root_dir.as_ref().to_owned();
                let file_name = {
                    let mut file_name =
                        video.path.file_stem().unwrap().to_owned();
                    file_name.push('.');
                    file_name.push_str(
                        subtitle
                            .lang
                            .to_639_1()
                            .unwrap_or(subtitle.lang.to_639_3()),
                    );
                    if subtitle.lang == Language::Eng {
                        file_name.push('.');
                        file_name.push_str(jellyfin_flags::DEFAULT)
                    }
                    file_name.push('.');
                    file_name.push_str(subtitle.path.extension().unwrap());
                    file_name
                };
                path.push(file_name);
                path
            };
            info!(
                "naming {} symlink for {} to {}",
                subtitle.lang.to_name(),
                video.path.file_name().unwrap(),
                subtitle_name.file_name().unwrap(),
            );
            if let Err(why) = symlink(&subtitle.path, &subtitle_name) {
                error!(
                    "failed to create symlink {} -> {subtitle_name}: {why}",
                    &subtitle.path
                );
            }
        });
}

fn remove_duplicate_languages(subs: &mut Vec<Subtitle>) {
    let mut seen = Vec::new();
    subs.retain(|sub| {
        if seen.contains(&(sub.lang, sub.series_info)) {
            warn!(
                "skipping duplicate {} subtitle {}",
                sub.lang.to_name(),
                &sub.path
            );
            false
        } else {
            seen.push((sub.lang, sub.series_info));
            true
        }
    });
}

#[derive(Debug)]
pub struct Video {
    path: Utf8PathBuf,
    series_info: Option<SeriesInfo>,
}

impl Video {
    fn from_path(path: Utf8PathBuf) -> anyhow::Result<Self> {
        let series_info = match SERIES_INFO_REGEX.find(path.as_str()) {
            Some(series_info) => {
                info!("found series info in {path}");
                series_info.as_str().parse::<SeriesInfo>()?.into()
            },
            None => None,
        };
        Ok(Video { path, series_info })
    }

    fn part_of_series(&self) -> bool {
        self.series_info.is_some()
    }
}

impl AsRef<Utf8Path> for Video {
    fn as_ref(&self) -> &Utf8Path {
        self.path.as_ref()
    }
}

static SERIES_INFO_REGEX: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"S\d{2}E\d{2}"#)
        .case_insensitive(true)
        .build()
        .unwrap()
});

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct SeriesInfo {
    season: NonZeroU8,
    episode: NonZeroU8,
}

impl FromStr for SeriesInfo {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 6 || !SERIES_INFO_REGEX.is_match(s) {
            bail!("doesn't match pattern S01E01");
        }
        let season = s[1..3].parse().context("couldn't parse season")?;
        let episode = s[4..6].parse().context("couldn't parse episode")?;
        Ok(SeriesInfo { season, episode })
    }
}

#[derive(Debug)]
struct Subtitle {
    path: Utf8PathBuf,
    lang: Language,
    series_info: Option<SeriesInfo>,
}

static NUMBER_PREFIX_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\d+_"#).unwrap());

impl Subtitle {
    fn new(path: Utf8PathBuf) -> anyhow::Result<Self> {
        let file_name =
            path.file_stem().expect("subtitle should have file name");
        trace!("regexing {file_name:?}");
        let language = NUMBER_PREFIX_REGEX.splitn(file_name, 2).last().unwrap();
        info!("guessing language is {language:?}");
        let lang = Language::from_name(language)
            .ok_or_else(|| anyhow!("couldn't find language {:?}", language))?;

        let series_info = match SERIES_INFO_REGEX.find(path.as_str()) {
            Some(series_info) => {
                info!("found series info in {path}");
                series_info.as_str().parse::<SeriesInfo>()?.into()
            },
            None => None,
        };

        Ok(Self {
            path,
            lang,
            series_info,
        })
    }
}

mod predicates {
    use std::ffi::OsStr;

    use camino::Utf8Path;
    use log::{error, info, trace};
    use once_cell::sync::Lazy;
    use regex::{Regex, RegexBuilder};
    use walkdir::DirEntry;

    use crate::Video;

    const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi"];
    const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "vtt", "idx", "ass", "dts"];

    static SEASON_AND_QUALITY_SUFFIX_REGEX: Lazy<Regex> = Lazy::new(|| {
        RegexBuilder::new(r#"( S\d{2}E\d{2})? - ((720p)|(1080p)|(4K( HDR)?))$"#)
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

    pub fn all_a_series<'a>(
        videos: impl IntoIterator<Item = &'a Video>,
    ) -> bool {
        videos.into_iter().all(|vid| vid.part_of_series())
    }

    pub fn no_series<'a>(videos: impl IntoIterator<Item = &'a Video>) -> bool {
        videos.into_iter().all(|vid| !vid.part_of_series())
    }

    // Assumes files has 2 or more elements
    pub fn different_versions_same_media(
        files: impl IntoIterator<Item = impl AsRef<Utf8Path>>,
    ) -> bool {
        let mut files = files.into_iter();
        let first = files
            .next()
            .expect("files iter should have at least two elements");
        let first = first.as_ref();
        let first_name = first.file_stem().expect("file has no name");
        trace!("regexing {first_name:?}");
        let Some(name_prefix) =
            SEASON_AND_QUALITY_SUFFIX_REGEX.splitn(first_name, 2).next()
        else {
            error!("couldn't find quality suffix in {first}");
            return false;
        };
        info!("guessing movie/episode name is {name_prefix:?}");
        files.all(|file| {
            file.as_ref()
                .file_stem()
                .map(|name| name.starts_with(name_prefix))
                .unwrap_or_default()
        })
    }
}

#[allow(unused)]
mod jellyfin_flags {
    pub const DEFAULT: &str = "default";
    pub const FORCED: &str = "forced";
    pub const HEARING_IMPAIRED: &str = "cc";
}

// Nothing is symlinked except in release builds
#[cfg(unix)]
fn symlink(
    actual_file: impl AsRef<Path>,
    link_here: impl AsRef<Path>,
) -> io::Result<()> {
    use std::os::unix::fs;
    match cfg!(debug_assertions) {
        false => fs::symlink(actual_file, link_here),
        true => Ok(()),
    }
}

// Nothing is symlinked except in release builds
#[cfg(windows)]
fn symlink(
    actual_file: impl AsRef<Path>,
    link_here: impl AsRef<Path>,
) -> io::Result<()> {
    use std::os::windows::fs;
    assert!(std::fs::metadata(actual_file.as_ref())?.is_file());
    match cfg!(debug_assertions) {
        false => fs::symlink_file(actual_file, link_here),
        true => Ok(()),
    }
}
