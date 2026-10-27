//! Command-line options. The `hoardmap` command only hands a folder to the Mac app.

use std::path::PathBuf;

pub const USAGE: &str = "\
Usage: hoardmap [PATH] [--help]

Opens the Hoardmap app, scanning PATH when one is given.

  -h, --help      show this help
";

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// `--help` was given.
    Help,
    /// A message for stderr; the caller exits 2.
    Usage(String),
}

/// Parses the arguments after the program name: at most one folder.
pub fn parse<I>(args: I) -> Result<Option<PathBuf>, Error>
where
    I: IntoIterator<Item = String>,
{
    let mut path = None;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Err(Error::Help),
            _ if arg.starts_with('-') => {
                return Err(Error::Usage(format!("unknown option: {arg}")));
            }
            _ if path.is_some() => {
                return Err(Error::Usage(format!("unexpected argument: {arg}")));
            }
            _ => path = Some(PathBuf::from(arg)),
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Option<PathBuf>, Error> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn no_arguments_is_no_folder() {
        assert_eq!(parse_strs(&[]), Ok(None));
    }

    #[test]
    fn one_folder() {
        assert_eq!(parse_strs(&["/tmp"]), Ok(Some(PathBuf::from("/tmp"))));
    }

    #[test]
    fn help_wins() {
        assert_eq!(parse_strs(&["/tmp", "--help"]), Err(Error::Help));
        assert_eq!(parse_strs(&["-h"]), Err(Error::Help));
    }

    #[test]
    fn the_browser_options_are_gone() {
        for old in ["--port", "--no-open", "--linger"] {
            assert!(matches!(parse_strs(&[old]), Err(Error::Usage(_))), "{old}");
        }
    }

    #[test]
    fn a_second_folder_is_refused() {
        assert!(matches!(parse_strs(&["/a", "/b"]), Err(Error::Usage(_))));
    }
}
