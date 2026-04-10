<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/assets/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset=".github/assets/logo-dark.svg">
  <img alt="si" src=".github/assets/logo-dark.svg" width="160">
</picture>

# space-investigator

Disk space analysis. Finds the biggest files and directories, runs in parallel, single binary.

## Background

I used to work at Rackspace, over a decade ago now. The senior engineers there kept an internal wiki of useful scripts, and over a few years it turned into this massive collection of one-liners for every situation you'd run into on a customer's server.

Some of the scripts needed extra tools the original author had installed. You'd copy a command, paste it on a box, and it'd fail because `ncdu` or `sar` or some other utility wasn't there. So you'd end up with multiple versions of the same script, slightly different depending on which tools the author preferred.

Some people focused on writing versions that only used tools from the base kickstart install so they'd work on any box. Which got you stuff like this:

```sh
FS='./';resize;clear;date;df -h $FS; echo "Largest Directories:"; \
nice -n19 find $FS -mount -type d -print0 2>/dev/null|xargs -0 du -k| \
sort -runk1|head -n20|awk -F'\t' '{printf "%8d MB\t%s\n",($1/1024),$NF}'; \
echo "Largest Files:"; nice -n 19 find $FS -mount -type f -print0 2>/dev/null| \
xargs -0 du -k | sort -rnk1| head -n20 |awk -F'\t' '{printf "%8d MB\t%s\n",($1/1024),$NF}';
```

([explainshell breakdown](https://explainshell.com/explain?cmd=FS%3D%27./%27%3Bresize%3Bclear%3Bdate%3Bdf%20-h%20%24FS%3B%20echo%20%22Largest%20Directories%3A%22%3B%20nice%20-n19%20find%20%24FS%20-mount%20-type%20d%20-print0%202%3E/dev/null%7Cxargs%20-0%20du%20-k%7Csort%20-runk1%7Chead%20-n20%7Cawk%20-F%27%5Ct%27%20%27%7Bprintf%20%22%258d%20MB%5Ct%25s%5Cn%22%2C%28%241/1024%29%2C%24NF%7D%27%3B%20echo%20%22Largest%20Files%3A%22%3B%20nice%20-n%2019%20find%20%24FS%20-mount%20-type%20f%20-print0%202%3E/dev/null%7Cxargs%20-0%20du%20-k%7Csort%20-rnk1%7Chead%20-n20%7Cawk%20-F%27%5Ct%27%20%27%7Bprintf%20%22%258d%20MB%5Ct%25s%5Cn%22%2C%28%241/1024%29%2C%24NF%7D%27))

It works. Nobody is typing that from memory though. The knowledge base was probably the most visited page on our wiki, everyone had it bookmarked just for commands like this.

The other problem: on a filesystem with thousands of small files, it'd take forever. Single-threaded `find` piped to `du`, waiting, waiting.

So this is a Rust rewrite of that snippet. Same job, but parallel, and you just type `si /path`.

## What it does

- Shows filesystem usage (like `df -h`)
- Lists the largest directories and files, sorted by size
- Walks directories in parallel with [jwalk](https://github.com/Byron/jwalk)
- Stays on the same filesystem (won't cross mount boundaries)
- JSON output (`--json`) for feeding into `jq` or other tools

## Install

Grab a binary from the [releases page](https://github.com/gremlinltd/space-investigator/releases), or build from source:

```sh
cargo install --path .
```

## Usage

The binary is called `si`:

```
si [OPTIONS] [PATH]
```

| Flag | What it does | Default |
|------|-------------|---------|
| `PATH` | Directory to scan | `.` |
| `-d`, `--dirs <N>` | How many top directories to show | `20` |
| `-f`, `--files <N>` | How many top files to show | `20` |
| `-j`, `--json` | Output JSON instead of text | off |

### Examples

```sh
# Current directory
si

# /var, top 10 of each
si /var -d 10 -f 10

# Pipe JSON to jq
si /home --json | jq '.largest_files[:5]'
```

## Author

Built by [Michael Leer (Trozz)](https://github.com/trozz) at [Gremlin](https://github.com/gremlinltd).

## License

[MIT](LICENSE)
