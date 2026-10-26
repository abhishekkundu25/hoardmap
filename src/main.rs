use hoardmap::cli::{self, USAGE};
use hoardmap::server;

fn main() {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(cli::Error::Help) => {
            print!("{USAGE}");
            return;
        }
        Err(cli::Error::Usage(message)) => {
            eprint!("hoardmap: {message}\n{USAGE}");
            std::process::exit(2);
        }
    };
    match server::run(options) {
        Ok(never) => match never {},
        Err(error) => {
            eprintln!("hoardmap: {error}");
            std::process::exit(1);
        }
    }
}
