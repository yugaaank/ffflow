# ffx

ffx is a commercial-grade, open-source (FOSS) terminal application that streamlines professional media processing with ffmpeg. It combines a focused TUI with a clean command surface for repeatable encoding and probing workflows that can be used in studios, pipelines, and internal tooling.

## What it does

- Wraps ffmpeg with a clear, typed command model (inputs, codecs, presets, extra args).
- Provides a fast, keyboard-first TUI for running jobs and viewing progress.
- Parses ffmpeg progress output and surfaces frames, time, and speed updates.
- Keeps a job history log so runs are reviewable without leaving the terminal.

## Why commercial FOSS

ffx is built to be used in production environments where reliability, observability, and predictable workflows matter. The code is open to enable transparency, audits, and community contributions, while the product is designed for commercial adoption and support.

## Usage

Run the TUI:

```bash
cargo run
```

Inside the TUI, type:

```bash
help
```

Example encode:

```bash
encode -i input.mp4 -o output.mp4 --vcodec libx264 --preset medium
```

Example probe:

```bash
probe -i input.mp4
```

Pass raw ffmpeg args:

```bash
ffmpeg -i input.mp4 -c:v libx264 -preset fast output.mp4
```

## Requirements

- Rust toolchain (edition 2024)
- ffmpeg available in `PATH`
## Project status

Early-stage, but functional. Expect rapid iteration and improvements to workflows, error handling, and UX polish.

## License

This project is open-source. Add your preferred license file if you plan to redistribute or sell it.
