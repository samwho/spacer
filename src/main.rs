use anyhow::{Context, Result};
use clap::Parser;
use lazy_static::lazy_static;
use owo_colors::OwoColorize;
use std::{
    io::{stdin, BufRead},
    sync::{Arc, RwLock},
    thread::{sleep, spawn},
};
use time::{
    format_description::OwnedFormatItem, macros::format_description, Instant, OffsetDateTime,
};

/// Spacer is a command line utility that inserts spacers between lines of
/// input. It is useful for visually separating the output of commands.
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Minimum amount of seconds that can pass without a spacer.
    #[arg(long)]
    every: Option<f64>,

    /// Maximum amount of seconds that can pass without a spacer.
    #[arg(long, default_value = "1")]
    after: Option<f64>,

    /// Inserts the date on each spacer.
    #[arg(long, default_value = "false")]
    date: bool,

    /// Inserts the time on each spacer.
    #[arg(long, default_value = "true")]
    time: bool,

    /// Inserts a delta of how long has passed between spacers on each spacer.
    #[arg(long, default_value = "true")]
    delta: bool,
}

lazy_static! {
    static ref DATE_FORMAT: OwnedFormatItem = format_description!("[year]-[month]-[day]").into();
    static ref TIME_FORMAT: OwnedFormatItem =
        format_description!("[hour]:[minute]:[second]").into();
}

fn format_elapsed(seconds: f64) -> String {
    let minutes = seconds / 60.0;
    let hours = minutes / 60.0;
    let days = hours / 24.0;
    let weeks = days / 7.0;
    let months = days / 30.0;
    let years = days / 365.0;

    if years >= 1.0 {
        format!("{:.1}y", years)
    } else if months >= 1.0 {
        format!("{:.1}mo", months)
    } else if weeks >= 1.0 {
        format!("{:.1}w", weeks)
    } else if days >= 1.0 {
        format!("{:.1}d", days)
    } else if hours >= 1.0 {
        format!("{:.1}h", hours)
    } else if minutes >= 1.0 {
        format!("{:.1}m", minutes)
    } else {
        format!("{:.1}s", seconds)
    }
}

fn print_spacer(args: &Args, last_spacer: &Instant) -> Result<()> {
    let (width, _) = terminal_size::terminal_size().context("Failed to get terminal size")?;
    let mut dashes: usize = width.0.into();

    let now = OffsetDateTime::now_local()?;
    if args.date {
        let date_str = now.format(&DATE_FORMAT)?;
        print!("{} ", date_str.green());
        dashes -= date_str.len() + 1;
    }

    if args.time {
        let time_str = now.format(&TIME_FORMAT)?;
        print!("{} ", time_str.yellow());
        dashes -= time_str.len() + 1;
    }

    if args.delta {
        let elapsed_seconds = last_spacer.elapsed().as_seconds_f64();
        if elapsed_seconds > 0.1 {
            let elapsed = format_elapsed(elapsed_seconds);
            print!("{} ", elapsed.blue());
            dashes -= elapsed.len() + 1;
        }
    }

    print!("{}", "━".repeat(dashes).as_str().dimmed());
    println!();
    Ok(())
}

// Define our custom error type
struct Spacer {
    args: Args,
    last_spacer: Arc<RwLock<Instant>>,
    last_line: Arc<RwLock<Instant>>,
}

impl Spacer {
    fn new(args: Args) -> Result<Self> {
        let spacer = Self {
            args,
            last_line: Arc::new(RwLock::new(Instant::now())),
            last_spacer: Arc::new(RwLock::new(Instant::now())),
        };

        Ok(spacer)
    }

    fn run(&mut self) -> Result<()> {
        let finished = Arc::new(RwLock::new(false));
        let c_last_spacer = self.last_spacer.clone();
        let c_last_line = self.last_line.clone();
        let c_args = self.args.clone();
        let c_finished = finished.clone();
        let thread = spawn(move || loop {
            if *c_finished.read().unwrap() {
                break;
            }
            sleep(std::time::Duration::from_millis(100));

            let last_line = c_last_line.read().unwrap();
            let last_spacer = c_last_spacer.read().unwrap();
            if *last_spacer >= *last_line {
                drop(last_line);
                drop(last_spacer);
                continue;
            }

            let elapsed_since_line = last_line.elapsed().as_seconds_f64();
            drop(last_line);

            let elapsed_since_spacer = last_spacer.elapsed().as_seconds_f64();

            if let Some(after) = c_args.after {
                if elapsed_since_line > after {
                    print_spacer(&c_args, &last_spacer).unwrap();
                    drop(last_spacer);
                    let mut last_spacer = c_last_spacer.write().unwrap();
                    last_spacer.clone_from(&Instant::now());
                    drop(last_spacer);
                    continue;
                }
            }

            if let Some(every) = c_args.every {
                if elapsed_since_spacer > every {
                    drop(last_spacer);
                    let mut last_spacer = c_last_spacer.write().unwrap();
                    print_spacer(&c_args, &last_spacer).unwrap();
                    last_spacer.clone_from(&Instant::now());
                    drop(last_spacer);
                    continue;
                }
            }
        });

        for line in stdin().lock().lines() {
            let line = line.context("Failed to read line")?;
            let mut last_line = self.last_line.write().unwrap();
            last_line.clone_from(&Instant::now());
            drop(last_line);
            println!("{}", line);
        }

        let mut finished = finished.write().unwrap();
        *finished = true;
        drop(finished);
        thread.join().unwrap();

        Ok(())
    }
}

fn main() -> Result<()> {
    let mut app = Spacer::new(Args::parse())?;
    app.run()
}
