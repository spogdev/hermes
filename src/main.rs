// Release builds are a GUI app with no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod download;
mod dropdown;
mod lang;
mod launch;
mod meta;
mod rules;
mod theme;
mod ui;

use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--headless") {
        return headless();
    }
    match ui::run(dev_options(&args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Debug builds only: `--screenshot <png>` saves the window after it settles and
/// exits; `--open-dropdown` starts with the account dropdown open.
#[cfg(debug_assertions)]
fn dev_options(args: &[String]) -> ui::DevOptions {
    ui::DevOptions {
        screenshot: args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(Into::into),
        open_dropdown: args.iter().any(|a| a == "--open-dropdown"),
    }
}

#[cfg(not(debug_assertions))]
fn dev_options(_: &[String]) -> ui::DevOptions {
    ui::DevOptions::default()
}

/// Smoke test: runs the same launch path as the Start button, prints timings,
/// then waits for the game and fails if it exits with an error.
fn headless() -> ExitCode {
    let start = Instant::now();
    let print = |s: launch::Step| println!("[{:>6} ms] {s}", start.elapsed().as_millis());
    let mut launched = match launch::launch(&print) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("launch failed: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "[{:>6} ms] game started (pid {}), {} files downloaded, launcher time {} ms",
        start.elapsed().as_millis(),
        launched.child.id(),
        launched.downloaded,
        launched.elapsed.as_millis()
    );
    if let Ok(paths) = launch::Paths::new() {
        println!("game output: {}", paths.game_output_log().display());
        println!("game log:    {}", paths.root.join("instance").join("logs").join("latest.log").display());
    }
    match launched.child.wait() {
        Ok(status) if status.success() => {
            println!("game exited cleanly");
            ExitCode::SUCCESS
        }
        Ok(status) => {
            eprintln!("game exited with {status}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("waiting for game failed: {e}");
            ExitCode::FAILURE
        }
    }
}
