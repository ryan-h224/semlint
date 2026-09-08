mod lint;

use lint::Severity;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut lenient = false;
    let mut paths: Vec<String> = Vec::new();

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--lenient" => lenient = true,
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => paths.push(other.to_string()),
        }
    }

    if paths.is_empty() {
        paths.push("-".to_string());
    }

    let mut had_error = false;

    for path in &paths {
        let content = match read_input(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: {}", path, e);
                had_error = true;
                continue;
            }
        };

        for finding in lint::lint_str(&content, lenient) {
            if finding.severity == Severity::Error {
                had_error = true;
            }
            println!(
                "{}:{}:{}: {}: {}",
                path, finding.line, finding.column, finding.severity, finding.message
            );
        }
    }

    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn read_input(path: &str) -> io::Result<String> {
    if path == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        fs::read_to_string(path)
    }
}

fn print_usage() {
    println!("semlint - a strict SemVer linter\n");
    println!("USAGE:");
    println!("    semlint [--lenient] [FILE...]\n");
    println!("Reads one version string per line from each FILE (or stdin if no FILE");
    println!("is given, or FILE is '-'). Blank lines and lines starting with '#' are");
    println!("skipped.\n");
    println!("OPTIONS:");
    println!("    --lenient   accept common real-world deviations (v prefix, missing");
    println!("                minor/patch components) instead of treating them as errors");
    println!("    -h, --help  print this message");
}
