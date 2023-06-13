# spacer

[![Build status](https://github.com/samwho/spacer/workflows/Build/badge.svg)](https://github.com/samwho/spacer/actions)
[![Crates.io](https://img.shields.io/crates/v/spacer.svg)](https://crates.io/crates/spacer)

Spacer is a simple CLI tool to insert spacers in when command output stops.

If you're the type of person that habitually presses enter a few times in your
log tail to know where the last request ended and the new one begins, this tool
is for you!

![](/images/spacer.gif)

## Installation

With Homebrew:

```
brew tap samwho/spacer
brew install spacer
```

Direct from Cargo:

```
cargo install spacer
```

## Usage

By default, spacer outputs a spacer after 1 second with no output. You can
change this with the `--after` flag.

```
tail -f some.log | spacer --after 5
```

`--after` accepts a number of seconds, and allows floating point numbers for
extra precision.
