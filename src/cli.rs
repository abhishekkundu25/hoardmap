//! Command-line options: the path to scan, the port, whether to open the browser, and
//! the linger period before the server exits.

use std::path::PathBuf;
use std::time::Duration;

pub const USAGE: &str = "\
Usage: hoardmap [PATH] [--port N] [--no-open] [--linger SECS] [--help]

Scans PATH (default: your home folder) and serves the result on 127.0.0.1.

  --port N        listen on port N (default: one the OS picks)
  --no-open       do not launch the browser
  --linger SECS   exit this many seconds after the last tab closes (default 10)
  -h, --help      show this help
";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub path: PathBuf,
    /// 0 lets the OS pick.
    pub port: u16,
    pub open: bool,
    pub linger: Duration,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// `--help` was given.
    Help,
    /// A message for stderr; the caller exits 2.
    Usage(String),
}

/// Parses the arguments after the program name.
pub fn parse<I>(args: I) -> Result<Options, Error>
where
    I: IntoIterator<Item = String>,
{
    let mut path = None;
    let mut port = 0;
    let mut open = true;
    let mut linger = Duration::from_secs(10);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let mut value = |what: &str| {
            inline
                .clone()
                .or_else(|| args.next())
                .ok_or_else(|| Error::Usage(format!("{name} needs {what}")))
        };
        match name.as_str() {
            "-h" | "--help" => return Err(Error::Help),
            "--no-open" => open = false,
            "--port" => {
                let v = value("a port number")?;
                port = v
                    .parse()
                    .map_err(|_| Error::Usage(format!("invalid port: {v}")))?;
            }
            "--linger" => {
                let v = value("a number of seconds")?;
                linger = v
                    .parse::<f64>()
                    .ok()
                    .filter(|s| s.is_finite() && *s >= 0.0)
                    .map(Duration::from_secs_f64)
                    .ok_or_else(|| Error::Usage(format!("invalid linger: {v}")))?;
            }
            _ if name.starts_with('-') && name.len() > 1 => {
                return Err(Error::Usage(format!("unknown option: {name}")));
            }
            _ if path.is_some() => {
                return Err(Error::Usage(format!("unexpected argument: {arg}")));
            }
            _ => path = Some(PathBuf::from(arg)),
        }
    }
    let path = path
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(Options {
        path,
        port,
        open,
        linger,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Options, Error> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults() {
        let o = parse_strs(&[]).unwrap();
        assert_eq!(o.path, PathBuf::from(std::env::var_os("HOME").unwrap()));
        assert_eq!(o.port, 0);
        assert!(o.open);
        assert_eq!(o.linger, Duration::from_secs(10));
    }

    #[test]
    fn every_option() {
        let o = parse_strs(&["/tmp", "--port", "8080", "--no-open", "--linger=2.5"]).unwrap();
        assert_eq!(o.path, PathBuf::from("/tmp"));
        assert_eq!(o.port, 8080);
        assert!(!o.open);
        assert_eq!(o.linger, Duration::from_millis(2500));
    }

    #[test]
    fn errors() {
        assert_eq!(parse_strs(&["--help"]), Err(Error::Help));
        assert_eq!(parse_strs(&["-h"]), Err(Error::Help));
        assert!(matches!(parse_strs(&["--bogus"]), Err(Error::Usage(_))));
        assert!(matches!(parse_strs(&["--port"]), Err(Error::Usage(_))));
        assert!(matches!(parse_strs(&["--port", "x"]), Err(Error::Usage(_))));
        assert!(matches!(
            parse_strs(&["--linger", "-1"]),
            Err(Error::Usage(_))
        ));
        assert!(matches!(parse_strs(&["a", "b"]), Err(Error::Usage(_))));
    }
}
