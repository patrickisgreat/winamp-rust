//! Command-line parsing: a handful of maintenance modes plus the normal
//! "open these files" start.

use std::path::PathBuf;

pub const USAGE: &str = "winamp [--install-theme NAME] [--export-default-skin DIR] [--screenshot DIR] [--selftest] [FILE|DIR|PLAYLIST|URL ...]";

const DEFAULT_EXPORT_DIR: &str = "base-skin";
const DEFAULT_THEME: &str = "DMT";
const DEFAULT_SCREENSHOT_DIR: &str = "screenshots";

/// What the user asked the binary to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliCommand {
    /// Write the built-in skin's bitmaps into a folder.
    ExportDefaultSkin(PathBuf),
    /// Install a procedural theme as a `.wsz` and select it.
    InstallTheme(String),
    /// Render the windows headlessly into `dir` after loading `inputs`.
    Screenshot {
        dir: PathBuf,
        inputs: Vec<String>,
    },
    /// Run the scripted UI self-test on `inputs`.
    SelfTest {
        inputs: Vec<String>,
    },
    Help,
    /// Start the player, queueing `inputs` (files, folders, playlists, URLs).
    Run {
        inputs: Vec<String>,
    },
}

/// Interpret the arguments after the program name. The first recognised
/// mode flag wins; anything that looks like a flag is dropped from inputs.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> CliCommand {
    let args: Vec<String> = args.into_iter().collect();
    let flag_position = |flag: &str| args.iter().position(|arg| arg == flag);
    let value_after = |index: usize, default: &str| {
        args.get(index + 1)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    };
    let inputs_except = |skip: &[usize]| -> Vec<String> {
        args.iter()
            .enumerate()
            .filter(|(index, arg)| !skip.contains(index) && !arg.starts_with("--"))
            .map(|(_, arg)| arg.clone())
            .collect()
    };

    if let Some(index) = flag_position("--export-default-skin") {
        return CliCommand::ExportDefaultSkin(PathBuf::from(value_after(
            index,
            DEFAULT_EXPORT_DIR,
        )));
    }
    if let Some(index) = flag_position("--install-theme") {
        return CliCommand::InstallTheme(value_after(index, DEFAULT_THEME));
    }
    if let Some(index) = flag_position("--screenshot") {
        let dir = PathBuf::from(value_after(index, DEFAULT_SCREENSHOT_DIR));
        return CliCommand::Screenshot {
            dir,
            inputs: inputs_except(&[index, index + 1]),
        };
    }
    if let Some(index) = flag_position("--selftest") {
        return CliCommand::SelfTest {
            inputs: inputs_except(&[index]),
        };
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return CliCommand::Help;
    }
    CliCommand::Run {
        inputs: inputs_except(&[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> CliCommand {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn plain_arguments_start_the_player() {
        assert_eq!(parse(&[]), CliCommand::Run { inputs: vec![] });
        assert_eq!(
            parse(&["song.mp3", "--unknown-flag", "http://radio"]),
            CliCommand::Run {
                inputs: vec!["song.mp3".into(), "http://radio".into()]
            }
        );
    }

    #[test]
    fn mode_flags_take_optional_values_with_defaults() {
        assert_eq!(
            parse(&["--export-default-skin"]),
            CliCommand::ExportDefaultSkin("base-skin".into())
        );
        assert_eq!(
            parse(&["--export-default-skin", "out"]),
            CliCommand::ExportDefaultSkin("out".into())
        );
        assert_eq!(
            parse(&["--install-theme"]),
            CliCommand::InstallTheme("DMT".into())
        );
        assert_eq!(
            parse(&["--install-theme", "Liminal"]),
            CliCommand::InstallTheme("Liminal".into())
        );
        assert_eq!(parse(&["--help"]), CliCommand::Help);
        assert_eq!(parse(&["x.mp3", "-h"]), CliCommand::Help);
    }

    #[test]
    fn screenshot_and_selftest_keep_the_remaining_inputs() {
        assert_eq!(
            parse(&["a.mp3", "--screenshot", "shots", "b.mp3"]),
            CliCommand::Screenshot {
                dir: "shots".into(),
                inputs: vec!["a.mp3".into(), "b.mp3".into()]
            }
        );
        assert_eq!(
            parse(&["--screenshot"]),
            CliCommand::Screenshot {
                dir: "screenshots".into(),
                inputs: vec![]
            }
        );
        assert_eq!(
            parse(&["--selftest", "a.wav", "--verbose", "b.wav"]),
            CliCommand::SelfTest {
                inputs: vec!["a.wav".into(), "b.wav".into()]
            }
        );
    }

    #[test]
    fn earlier_listed_modes_win_when_several_flags_are_given() {
        assert_eq!(
            parse(&["--selftest", "--install-theme", "DMT"]),
            CliCommand::InstallTheme("DMT".into())
        );
    }
}
