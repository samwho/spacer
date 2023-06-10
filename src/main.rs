use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use lazy_static::lazy_static;
use std::io::{stdin, BufRead};
use time::{
    format_description::OwnedFormatItem, macros::format_description, Instant, OffsetDateTime,
};

/// Spacer is a command line utility that inserts spacers between lines of
/// input. It is useful for visually separating the output of commands.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Minimum amount of seconds that can pass without a spacer.
    #[arg(long)]
    every: Option<f64>,

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

// Define our custom error type
struct Spacer {
    args: Args,
    last_spacer: Instant,
}

impl Spacer {
    fn new(args: Args) -> Result<Self> {
        let spacer = Self {
            args,
            last_spacer: Instant::now(),
        };
        Ok(spacer)
    }

    fn format_elapsed(&self, seconds: f64) -> String {
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

    fn print_spacer(&mut self) -> Result<()> {
        let (width, _) = term_size::dimensions().context("Failed to get terminal size")?;
        let mut dashes = width;

        let now = OffsetDateTime::now_local()?;
        if self.args.date {
            let date_str = now.format(&DATE_FORMAT)?;
            print!("{} ", date_str.green());
            dashes -= date_str.len() + 1;
        }

        if self.args.time {
            let time_str = now.format(&TIME_FORMAT)?;
            print!("{} ", time_str.yellow());
            dashes -= time_str.len() + 1;
        }

        if self.args.delta {
            let elapsed_seconds = self.last_spacer.elapsed().as_seconds_f64();
            if elapsed_seconds > 0.1 {
                let elapsed = self.format_elapsed(elapsed_seconds);
                print!("{} ", elapsed.blue());
                dashes -= elapsed.len() + 1;
            }
        }

        print!("{}", "━".repeat(dashes).as_str().dimmed());
        println!();
        self.last_spacer = Instant::now();
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        self.print_spacer()?;
        for line in stdin().lock().lines() {
            let line = line.context("Failed to read line")?;
            let elapsed = self.last_spacer.elapsed().as_seconds_f64();
            if let Some(every) = self.args.every {
                if elapsed > every.into() {
                    self.print_spacer()?;
                }
            }
            println!("{}", line);
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let mut app = Spacer::new(Args::parse())?;
    app.run()
}
