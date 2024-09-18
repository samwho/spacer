# spacer

[![Build status](https://github.com/samwho/spacer/workflows/Build/badge.svg)](https://github.com/samwho/spacer/actions)
[![Crates.io](https://img.shields.io/crates/v/spacer.svg)](https://crates.io/crates/spacer)

`spacer` is a simple CLI tool to insert spacers when command output stops.

If you're the type of person that habitually presses enter a few times in your
log tail to know where the last request ended and the new one begins, this tool
is for you!

![](/images/spacer.gif)

## Installation

With Homebrew:

```bash
brew install spacer
```

Direct from Cargo:

```bash
cargo install spacer
```

## Usage

By default, `spacer` outputs a spacer after 1 second with no output. You can
change this with the `--after` flag.

```bash
tail -f some.log | spacer --after 5
```

`--after` accepts a number of seconds, and allows floating point numbers for
extra precision.

## STDOUT and STDERR

Some commands output most of their information on STDERR, not STDOUT. `spacer`
only monitors STDOUT, so if you find a situation where `spacer` doesn't seem
to be working it could be that the program you're piping from is using STDERR.

To "fix" that, pipe both STDERR to STDOUT to spacer by using `|&` instead of 
`|` as the pipe characters:

```bash
my-command |& spacer
```
