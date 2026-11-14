// Release builds are a GUI app with no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod download;
mod game_window;
mod dropdown;
mod lang;
mod launch;
mod meta;
mod profiles;
mod rules;
mod theme;
mod ui;
mod window_shape;

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
/// exits; `--open-dropdown account`, `--state downloading|checking|running|failed`, `--open-sidebar` and
/// `--focus play|account` set up the state to capture.
#[cfg(debug_assertions)]
fn dev_options(args: &[String]) -> ui::DevOptions {
    ui::DevOptions {
        screenshot: args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(Into::into),
        open_dropdown: value_of(args, "--open-dropdown"),
        state: value_of(args, "--state"),
        focus: value_of(args, "--focus"),
        open_sidebar: args.iter().any(|a| a == "--open-sidebar"),
        screenshot_frame: value_of(args, "--screenshot-frame").and_then(|v| v.parse().ok()),
    }
}

#[cfg(debug_assertions)]
fn value_of(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

#[cfg(not(debug_assertions))]
fn dev_options(_: &[String]) -> ui::DevOptions {
    ui::DevOptions::default()
}

/// Smoke test: launches the selected profile the same way the Play button does,
/// prints timings, then waits for the game and fails if it exits with an error.
fn headless() -> ExitCode {
    let start = Instant::now();
    let game_dir = match launch::Paths::new().and_then(|p| Ok((profiles::Profiles::load(&p.root)?, p))) {
        Ok((profiles, paths)) => profiles::Profiles::dir(&paths.root, &profiles.selected),
        Err(e) => {
            eprintln!("loading profiles failed: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    println!("profile folder: {}", game_dir.display());
    let print = |s: launch::Step| println!("[{:>6} ms] {s}", start.elapsed().as_millis());
    let mut launched = match launch::launch(&game_dir, &print) {
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
        println!("game log:    {}", game_dir.join("logs").join("latest.log").display());
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
