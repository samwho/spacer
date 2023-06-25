use anyhow::{Context, Result};
use clap::Parser;
use lazy_static::lazy_static;
use log::debug;
use owo_colors::{self, OwoColorize, Stream};
use std::{
    io::{stdin, stdout, BufRead, Write},
    sync::{Arc, Mutex, RwLock},
    thread::{scope, sleep},
};
use terminal_size::{Height, Width};
use time::{
    format_description::OwnedFormatItem, macros::format_description, Instant, OffsetDateTime,
};

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Minimum number of seconds that have to pass before a spacer is printed
    #[arg(long, short, default_value = "1.0")]
    after: f64,

    /// Which character to use as a spacer
    #[arg(long, short, default_value = "━")]
    dash: char,

    /// Number of newlines to print before and after spacer lines
    #[arg(long, short, default_value = "0")]
    padding: usize,

    /// Force the output to not be colorized
    #[arg(long, group = "color-overrides", default_value = "false")]
    no_color: bool,

    /// Force the output to be colorized, even if the output is not a TTY
    #[arg(long, group = "color-overrides", default_value = "false")]
    force_color: bool,
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

fn print_spacer(output: impl Write, args: &Args, last_spacer: &Instant) -> Result<()> {
    let (width, _) = terminal_size::terminal_size().unwrap_or((Width(80), Height(24)));
    debug!("terminal width: {:?}", width);

    let mut dashes: usize = width.0.into();

    if args.padding > 0 {
        writeln!(output, "{}", "\n".repeat(args.padding));
    }

    let now = OffsetDateTime::now_local().unwrap_or(OffsetDateTime::now_utc());
    let date_str = now.format(&DATE_FORMAT)?;
    write!(
        output,
        "{} ",
        date_str.if_supports_color(Stream::Stdout, |t| t.green())
    );
    dashes -= date_str.len() + 1;

    let time_str = now.format(&TIME_FORMAT)?;
    write!(
        output,
        "{} ",
        time_str.if_supports_color(Stream::Stdout, |t| t.yellow())
    );
    dashes -= time_str.len() + 1;

    let elapsed_seconds = last_spacer.elapsed().as_seconds_f64();
    if elapsed_seconds > 0.1 {
        let elapsed = format_elapsed(elapsed_seconds);
        write!(
            output,
            "{} ",
            elapsed.if_supports_color(Stream::Stdout, |t| t.blue())
        );
        dashes -= elapsed.len() + 1;
    }

    writeln!(
        output,
        "{}",
        args.dash
            .to_string()
            .repeat(dashes)
            .as_str()
            .if_supports_color(Stream::Stdout, |t| t.dimmed())
    );
    writeln!(output);

    if args.padding > 0 {
        writeln!(output, "{}", "\n".repeat(args.padding));
    }

    Ok(())
}

fn run(input: impl BufRead, output: impl Write, args: Args) -> Result<()> {
    if args.force_color && args.no_color {
        eprintln!("--force-color and --no-color are mutually exclusive");
        std::process::exit(1);
    }

    if args.no_color {
        owo_colors::set_override(false);
    }

    if args.force_color {
        owo_colors::set_override(true);
    }

    let last_line = Arc::new(RwLock::new(Instant::now()));
    let last_spacer = Arc::new(RwLock::new(Instant::now()));

    let finished = Arc::new(RwLock::new(false));
    let c_last_spacer = last_spacer.clone();
    let c_last_line = last_line.clone();
    let c_args = args.clone();
    let c_finished = finished.clone();
    let c_output = Arc::new(Mutex::new(output));

    scope(|s| {
        s.spawn(move || loop {
            if *c_finished.read().unwrap() {
                debug!("thread received finish signal, exiting");
                break;
            }

            let last_line = c_last_line.read().unwrap();
            let last_spacer = c_last_spacer.read().unwrap();
            if *last_spacer >= *last_line {
                drop(last_line);
                drop(last_spacer);

                debug!("last spacer is newer than last line, sleeping");

                // We sleep here because we know that we're going to sleep for
                // a bare minimum of the --after interval.
                sleep(std::time::Duration::from_millis(
                    (c_args.after * 1000.0) as u64,
                ));
                continue;
            }

            let elapsed_since_line = last_line.elapsed().as_seconds_f64();
            drop(last_line);

            if elapsed_since_line >= c_args.after {
                debug!("last line is older than --after, printing spacer");

                let mut output = c_output.lock().unwrap();
                print_spacer(&mut output, &c_args, &last_spacer).unwrap();
                drop(output);

                let mut last_spacer = c_last_spacer.write().unwrap();
                last_spacer.clone_from(&Instant::now());
                drop(last_spacer);

                // We sleep here because we know that we're going to sleep for
                // a bare minimum of the --after interval.
                sleep(std::time::Duration::from_millis(
                    (c_args.after * 1000.0) as u64,
                ));
            } else {
                let sleep_for = c_args.after - elapsed_since_line;
                debug!(
                    "last line is newer than --after, sleeping for {:.2}s",
                    sleep_for
                );

                // We sleep for as long as it takes to get to the number of
                // seconds --after the last line was printed.
                sleep(std::time::Duration::from_millis(
                    (sleep_for * 1000.0) as u64,
                ));
            }
        });

        for line in input.lines() {
            let line = line.context("Failed to read line")?;
            let mut last_line = last_line.write().unwrap();
            last_line.clone_from(&Instant::now());
            drop(last_line);
            writeln!(c_output.lock().unwrap(), "{}", line)?;
        }

        debug!("signalling thread to finish");
        let mut finished = finished.write().unwrap();
        *finished = true;
        drop(finished);

        Ok(())
    })
}

fn main() -> Result<()> {
    human_panic::setup_panic!();
    env_logger::init();

    let args = Args::parse();
    debug!("args: {:?}", args);

    run(&mut stdin().lock(), &mut stdout(), args)
}
