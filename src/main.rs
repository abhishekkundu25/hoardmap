use macdirtree::cli::{self, USAGE};
use macdirtree::server;

fn main() {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(cli::Error::Help) => {
            print!("{USAGE}");
            return;
        }
        Err(cli::Error::Usage(message)) => {
            eprint!("macdirtree: {message}\n{USAGE}");
            std::process::exit(2);
        }
    };
    match server::run(options) {
        Ok(never) => match never {},
        Err(error) => {
            eprintln!("macdirtree: {error}");
            std::process::exit(1);
        }
    }
}
