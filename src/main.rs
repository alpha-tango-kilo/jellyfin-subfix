use std::{
    env, io,
    path::{Path, PathBuf},
};

use log::{debug, error};

fn main() {
    env_logger::builder().format_timestamp(None).init();
    debug!("Hello, world!");
    env::args_os().for_each(|arg| {
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
    todo!()
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
