# Jellyfin subtitle finder

Goes hunting for subtitle files and creates symlinks to facilitate Jellyfin in discovering them

## Usage

```
subfix [MOVIE_DIR]...
```

## How it works

The directory you give as an argument is searched for video files (only in that directory, not descending into child directories).
If multiple video files are found, then it is checked that they're the same movie but a different version (see `test_dir/dual` for what I mean), following [Jellyfin's naming convention](https://jellyfin.org/docs/general/server/media/movies/#multiple-versions-of-a-movie).
Subtitles are then searched for in the given directory (child directories are explored), prefix numbers are stripped (expected format being `123_Language.ext`), and the language is then checked to see if is recognised for being converted to a [ISO 639-1](https://www.iso.org/standard/22109.html) (2 letter) code.
Currently, the first subtitle found for each language will then be symlinked to the given directory using [Jellyfin's preferred subtitle naming convention](https://jellyfin.org/docs/general/server/media/external-files#naming).
This results in one symlink per language, per version of the movie

For the most part, anything that's considered an error just results in that thing being skipped, as opposed to the program completely bombing out.
The logs should be pretty communicative about what's happening

Also supports series, where the series information should be specified in the file name before the quality suffix (see `test_dir/series`)

## Future plans

Supporting multiple subtitles of the same language, and flagging whether the subtitle track should be made default, marked as forced / foreign / hearing impaired.
Probably going to pull in [`bat`](https://github.com/sharkdp/bat) to show previews of subtitle files and get the user to select the flags etc.

### Not planned

Making the tool more flexible.
It fits my workflow/organisation, so I don't intend to change it.
If you want to contribute something that makes the tool more accommodating and the logic is backwards compatible, I'm happy to review a PR.
Otherwise, please maintain your own fork of the tool - I made this more as a personal tool than a community project
