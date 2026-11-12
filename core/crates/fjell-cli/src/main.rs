//! `fjell-cli`: testsele. Bruk: `fjell-cli <skann|liste|scan|list> <mappe>`.
//! Leser bare; originalfiler endres aldri. Logikken ligger i biblioteket (`cmd`, `fsource`).

use fjell_cli::{cmd, fsource::FsSource};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(c), Some(dir)) = (args.first(), args.get(1)) else {
        eprintln!(
            "fjell-cli {}\nbruk: fjell-cli <skann|liste> <mappe>",
            fjell_core::VERSION
        );
        return ExitCode::from(2);
    };
    let src = FsSource::new(dir);
    let mut out = std::io::stdout().lock();
    let r = match c.as_str() {
        "skann" | "scan" => cmd::scan(&src, &mut out),
        "liste" | "list" => cmd::list(&src, &mut out).map(|_| ()),
        _ => {
            eprintln!("ukjent kommando: {c}");
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("feil: {e}");
            ExitCode::FAILURE
        }
    }
}
