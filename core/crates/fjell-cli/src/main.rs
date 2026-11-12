//! `fjell-cli`: testsele. Bruk: `fjell-cli <skann|liste> <mappe>` (mock) og
//! `fjell-cli <index|trash|undo|purge> <mappe> --db <fil> [--yes] [--fs-temp] [id...]` (ekte kjerne).
//! Originalfiler endres aldri; purge sletter bare i en midlertidig kopi (`--fs-temp`).
//! Logikken ligger i biblioteket (`cmd`, `flow`, `fsource`).

use fjell_cli::flow::{self, FlowError};
use fjell_cli::{cmd, fsource::FsSource};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "bruk: fjell-cli <skann|liste> <mappe>\n      fjell-cli <index|trash|undo|purge> <mappe> --db <fil> [--yes] [--fs-temp] [id...]\n      fjell-cli perf [--n 10000] [--rows 50000] [--rounds 500] [--dir <mappe>]";

fn flow_cmd(c: &str, args: &[String]) -> Result<(), FlowError> {
    let (mut db, mut yes, mut tmp) = (None, false, false);
    let (mut dir, mut ids) = (None, Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--db" => db = it.next().map(PathBuf::from),
            "--yes" => yes = true,
            "--fs-temp" => tmp = true,
            s if dir.is_none() => dir = Some(PathBuf::from(s)),
            s => ids.push(
                s.parse::<u64>()
                    .map_err(|_| FlowError::Usage(format!("ugyldig id: {s}")))?,
            ),
        }
    }
    let (Some(dir), Some(db)) = (dir, db) else {
        return Err(FlowError::Usage(USAGE.into()));
    };
    let mut out = std::io::stdout().lock();
    match c {
        "index" => flow::index(&dir, &db, &mut out),
        "trash" => flow::trash(&dir, &db, yes, &mut out),
        "undo" => flow::undo(&dir, &db, &mut out),
        _ => flow::purge(&dir, &db, &ids, tmp, yes, &mut out),
    }
}

fn perf_cmd(args: &[String]) -> ExitCode {
    let mut o = fjell_cli::perf::PerfOptions::standard(&std::env::temp_dir().join("fjell-perf"));
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let num = |v: Option<&String>| v.and_then(|s| s.parse::<usize>().ok());
        match a.as_str() {
            "--n" => o.n = num(it.next()).unwrap_or(o.n),
            "--rows" => o.rows = num(it.next()).unwrap_or(o.rows),
            "--rounds" => o.rounds = num(it.next()).unwrap_or(o.rounds),
            "--dir" => o.dir = it.next().map(PathBuf::from).unwrap_or(o.dir),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match fjell_cli::perf::run(&o, &mut std::io::stdout().lock()) {
        Ok(r) if r.failures.is_empty() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("feil: {e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(c) = args.first() else {
        eprintln!("fjell-cli {}\n{USAGE}", fjell_core::VERSION);
        return ExitCode::from(2);
    };
    if c == "crash-child" {
        // Intern: barneprosess for krasjtesten (tests/crash.rs).
        let (Some(mode), Some(db)) = (args.get(1), args.get(2)) else {
            return ExitCode::from(2);
        };
        let n = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(5000);
        return match fjell_cli::crashtest::child(mode, std::path::Path::new(db), n) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("feil: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if c == "perf" {
        return perf_cmd(&args[1..]);
    }
    if matches!(c.as_str(), "index" | "trash" | "undo" | "purge") {
        return match flow_cmd(c, &args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("feil: {e}");
                ExitCode::from(match e {
                    FlowError::Usage(_) => 2,
                    FlowError::NeedsConfirm(_) => 3,
                    FlowError::Failed(_) => 1,
                })
            }
        };
    }
    let Some(dir) = args.get(1) else {
        eprintln!("{USAGE}");
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
