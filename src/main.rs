use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use chrono_tz::Tz;
use clap::Parser;
use human_panic::setup_panic;
use log::debug;
use owo_colors::{self, OwoColorize, Stream};
use std::time::Instant;
use std::{
    io::{stdin, stdout, BufRead, Write},
    ops::DerefMut,
    sync::{Arc, Mutex, RwLock},
    thread::{scope, sleep},
};
use terminal_size::{Height, Width};

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

    /// Put the timestamp on the right side of the spacer.
    #[arg(long, default_value = "false")]
    right: bool,

    /// Print timestamp in an arbitrary timezone (in IANA format, e.g. Europe/London).
    #[arg(long)]
    timezone: Option<String>,
}

struct TestStats {
    wakeups: usize,
}

impl TestStats {
    // This is only used in tests.
    #[allow(dead_code)]
    fn new() -> Self {
        Self { wakeups: 0 }
    }
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

fn print_spacer(mut output: impl Write, args: &Args, last_spacer: &Instant) -> Result<()> {
    let (width, _) = terminal_size::terminal_size().unwrap_or((Width(80), Height(24)));
    debug!("terminal width: {:?}", width);

    let mut dashes: i32 = width.0.into();

    if args.padding > 0 {
        writeln!(output, "{}", "\n".repeat(args.padding - 1))?;
    }

    let datetime_strings = match args.timezone.clone() {
        None => {
            let now: DateTime<Local> = Local::now();
            (
                now.format("%Y-%m-%d").to_string(),
                now.format("%H:%M:%S").to_string(),
            )
        }
        Some(timezone_str) => match timezone_str.parse::<Tz>() {
            Ok(timezone) => {
                let now: DateTime<Tz> = Local::now().with_timezone(&timezone);
                (
                    now.format("%Y-%m-%d").to_string(),
                    now.format("%H:%M:%S %Z").to_string(),
                )
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                debug!("could not parse supplied timezone name, using local time");
                let now: DateTime<Local> = Local::now();
                (
                    now.format("%Y-%m-%d").to_string(),
                    now.format("%H:%M:%S").to_string(),
                )
            }
        },
    };

    let date_str = datetime_strings.0;
    let mut buf = Vec::new();
    write!(
        buf,
        "{} ",
        date_str.if_supports_color(Stream::Stdout, |t| t.green())
    )?;

    dashes -= (date_str.len() + 1) as i32;

    let time_str = datetime_strings.1;
    write!(
        buf,
        "{} ",
        time_str.if_supports_color(Stream::Stdout, |t| t.yellow())
    )?;
    dashes -= (time_str.len() + 1) as i32;

    let elapsed_seconds = last_spacer.elapsed().as_secs_f64();
    if elapsed_seconds > 0.1 {
        let elapsed = format_elapsed(elapsed_seconds);
        write!(
            buf,
            "{} ",
            elapsed.if_supports_color(Stream::Stdout, |t| t.blue())
        )?;
        dashes -= (elapsed.len() + 1) as i32;
    }

    buf.pop(); // Remove trailing space
    let info = String::from_utf8(buf)?;

    if dashes < 0 {
        dashes = 0;
    }

    if args.right {
        write!(
            output,
            "{} ",
            args.dash
                .to_string()
                .repeat(dashes as usize)
                .as_str()
                .if_supports_color(Stream::Stdout, |t| t.dimmed())
        )?;
        writeln!(output, "{}", info)?;
    } else {
        write!(output, "{} ", info)?;
        writeln!(
            output,
            "{}",
            args.dash
                .to_string()
                .repeat(dashes as usize)
                .as_str()
                .if_supports_color(Stream::Stdout, |t| t.dimmed())
        )?;
    }

    if args.padding > 0 {
        writeln!(output, "{}", "\n".repeat(args.padding - 1))?;
    }

    Ok(())
}

fn run(
    input: impl BufRead,
    output: impl Write + Send,
    args: Args,
    mut test_stats: Option<&mut TestStats>,
) -> Result<()> {
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

    scope(|s| {
        let last_line = Arc::new(RwLock::new(Instant::now()));
        let last_spacer = Arc::new(RwLock::new(Instant::now()));
        let output = Arc::new(Mutex::new(output));

        let finished = Arc::new(RwLock::new(false));
        let c_last_spacer = last_spacer;
        let c_last_line = last_line.clone();
        let c_args = args.clone();
        let c_finished = finished.clone();
        let c_output = output.clone();

        s.spawn(move || loop {
            if let Some(test_stats) = &mut test_stats {
                test_stats.wakeups += 1;
            }

            if *c_finished.read().unwrap() {
                debug!("thread received finish signal, exiting");
                break;
            }

            debug!("begin loop");

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

            let elapsed_since_line = last_line.elapsed().as_secs_f64();
            drop(last_line);

            if elapsed_since_line >= c_args.after {
                debug!("last line is older than --after, printing spacer");

                let mut output = c_output.lock().unwrap();
                print_spacer(output.deref_mut(), &c_args, &last_spacer).unwrap();
                drop(last_spacer);
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
                // When calculating how long to sleep for, we want to make sure
                // that we sleep for at least 10ms, so that we don't spin too
                // much.
                let sleep_for = f64::max(0.01, c_args.after - elapsed_since_line);
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
            let mut out = output.lock().unwrap();
            writeln!(out, "{}", line)?;
            drop(out);
        }

        debug!("signalling thread to finish");
        let mut finished = finished.write().unwrap();
        *finished = true;
        drop(finished);

        Ok(())
    })
}

fn main() -> Result<()> {
    setup_panic!();
    env_logger::init();

    let args = Args::parse();
    debug!("args: {:?}", args);

    run(stdin().lock(), stdout(), args, None)
}

#[cfg(test)]
mod tests {
    use self::Op::*;
    use self::Out::*;
    use super::*;
    use std::io::{BufReader, Read};
    use std::thread::sleep;
    use std::time::Duration;
    use test_case::test_case;

    enum Op {
        Sleep(u64),
        Write(&'static str),
        WriteLn(&'static str),
    }

    enum Out {
        Line(&'static str),
        Spacer,
        RightSpacer,
        SpacerWithLondonTimezone,
        RightSpacerWithLondonTimezone,
    }

    struct TimedInput {
        ops: Vec<Op>,
        index: usize,
    }

    impl TimedInput {
        fn new(ops: Vec<Op>) -> Self {
            Self { ops, index: 0 }
        }
    }

    impl Read for TimedInput {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            loop {
                if self.index >= self.ops.len() {
                    return Ok(0);
                }

                let op = &self.ops[self.index];
                self.index += 1;
                match op {
                    Op::Sleep(duration) => {
                        sleep(Duration::from_millis(*duration));
                    }
                    Op::Write(string) => {
                        let bytes = string.as_bytes();
                        buf[..bytes.len()].clone_from_slice(bytes);
                        return Ok(bytes.len());
                    }
                    Op::WriteLn(string) => {
                        let str = format!("{}\n", string);
                        let bytes = str.as_bytes();
                        buf[..bytes.len()].clone_from_slice(bytes);
                        return Ok(bytes.len());
                    }
                }
            }
        }
    }

    fn test_args() -> Args {
        Args {
            after: 0.1,
            dash: '-',
            padding: 0,
            no_color: true,
            force_color: false,
            right: false,
            timezone: None,
        }
    }

    #[test_case(vec![], vec![], test_args() ; "no output")]
    #[test_case(vec![Sleep(300)], vec![], test_args() ; "no output, after sleep")]
    #[test_case(
        vec![WriteLn("foo"), Sleep(300)],
        vec![Line("foo"), Spacer],
        test_args()
        ; "single line"
    )]
    #[test_case(
        vec![WriteLn("foo"), Sleep(300), WriteLn("bar"), WriteLn("baz"), Sleep(300)],
        vec![Line("foo"), Spacer, Line("bar"), Line("baz"), Spacer],
        test_args()
        ; "multiple lines"
    )]
    #[test_case(
        vec![WriteLn("foo"), WriteLn("bar"), WriteLn("baz")],
        vec![Line("foo"), Line("bar"), Line("baz")],
        test_args()
        ; "multiple lines, no sleeps"
    )]
    #[test_case(
        vec![Write("foo"), Write("bar"), Sleep(300), WriteLn("baz")],
        vec![Line("foobarbaz")],
        test_args()
        ; "single line, sleep in the middle"
    )]
    #[test_case(
        vec![WriteLn("foo"), Sleep(300)],
        vec![Line("foo"), RightSpacer],
        Args {
            after: 0.1,
            dash: '-',
            padding: 0,
            no_color: true,
            force_color: false,
            right: true,
            timezone: None,
        }
        ; "single line, right spacer"
    )]
    #[test_case(
        vec![WriteLn("foo"), Sleep(300)],
        vec![Line("foo"), Line(""), Spacer, Line("")],
        Args {
            after: 0.1,
            dash: '-',
            padding: 1,
            no_color: true,
            force_color: false,
            right: false,
            timezone: None,
        }
        ; "padding = 1"
    )]
    #[test_case(
        vec![WriteLn("foo"), Sleep(300)],
        vec![Line("foo"), Line(""), Line(""), Spacer, Line(""), Line("")],
        Args {
            after: 0.1,
            dash: '-',
            padding: 2,
            no_color: true,
            force_color: false,
            right: false,
            timezone: None,
        }
        ; "padding = 2"
    )]
    #[test_case(
        vec![WriteLn("foo"), Sleep(300)],
        vec![Line("foo"), SpacerWithLondonTimezone],
        Args {
            after: 0.1,
            dash: '-',
            padding: 0,
            no_color: true,
            force_color: false,
            right: false,
            timezone: Some("Europe/London".to_string()),
        }
        ; "with timezone"
    )]
    #[test_case(
        vec![WriteLn("foo"), Sleep(300)],
        vec![Line("foo"), RightSpacerWithLondonTimezone],
        Args {
            after: 0.1,
            dash: '-',
            padding: 0,
            no_color: true,
            force_color: false,
            right: true,
            timezone: Some("Europe/London".to_string()),
        }
        ; "right spacer with timezone"
    )]
    fn test_output(ops: Vec<Op>, out: Vec<Out>, args: Args) -> Result<()> {
        let mut total_sleep_ms = 0;
        for op in ops.iter() {
            if let Sleep(duration) = op {
                total_sleep_ms += duration;
            }
        }

        let expected_wakeups = 2 + (total_sleep_ms as f64 / (args.after * 1000.0)).ceil() as usize;

        let input = BufReader::new(TimedInput::new(ops));
        let mut output = Vec::new();

        let mut stats = super::TestStats::new();
        run(input, &mut output, args, Some(&mut stats))?;

        let output = String::from_utf8(output)?;
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(
            lines.len(),
            out.len(),
            "wrong number of lines, expected {} got {:?}",
            out.len(),
            lines
        );
        for (line, out) in lines.iter().zip(out.iter()) {
            match out {
                Line(expected) => assert_eq!(line, expected),
                Spacer => assert!(line.ends_with("----")),
                RightSpacer => assert!(line.starts_with("----")),
                SpacerWithLondonTimezone => {
                    assert!(line.contains("GMT") || line.contains("BST"));
                    assert!(line.ends_with("----"));
                }
                RightSpacerWithLondonTimezone => {
                    assert!(line.contains("GMT") || line.contains("BST"));
                    assert!(line.starts_with("----"));
                }
            }
        }

        assert!(
            // Allow some wiggle room in the expected wakeups.
            stats.wakeups <= (expected_wakeups * 2),
            "too many wakeups, expected {} got {}",
            expected_wakeups,
            stats.wakeups
        );
        Ok(())
    }
}
