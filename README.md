# spacer

Spacer is a simple utility that puts visual markers in command output to help
you know what happened when. No more habitually pressing enter a few times
in your log tail to know where the last request ended and the new one begins.

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
change this if you like:

```
tail -f some.log | spacer --after 5
```
