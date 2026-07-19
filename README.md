# ffflow

`ffflow` is a CLI and TUI for building structured, repeatable ffmpeg
workflows. Instead of retyping long ffmpeg invocations or wrangling shell
scripts, you describe a pipeline once in a `.flw` file (or on the command
line) and `ffflow` runs it with a live, ratatui-driven progress view — one
track per job, real-time encoding progress, and a summary at the end.

It wraps `ffmpeg` (it shells out to the binary on your `PATH`); it does not
reimplement encoders.

## Why

Complex media commands are easy to get wrong and hard to reuse. `ffflow` gives
the commands a name and a shape: presets for codecs, a batch format for
multi-file jobs, and a TUI so you can watch several encodes at once instead of
reading ffmpeg's stderr.

## Architecture

```
ffflow/
├── main.rs        entry; parse args, load .flw, hand off to tui::run
├── cli.rs         clap parser: top-level FILE + Encode/Probe/Presets
├── repl.rs        line parser (parse_line) for .flw batch files
├── tui.rs         ratatui UI + event loop
├── core/
│   ├── command.rs   FfmpegCommand (inputs, output, codecs, preset, extra)
│   ├── batch.rs     parse_flw_file -> Vec<FfmpegCommand>
│   ├── runner.rs    spawns ffmpeg, streams progress
│   ├── progress.rs  parses ffmpeg's progress lines into a model
│   ├── job.rs       per-job state machine
│   ├── metadata.rs  probe output (via -f null)
│   ├── formatter.rs render commands/presets as text
│   ├── summary.rs   end-of-run report
│   └── event.rs / error.rs / mod.rs
└── util/fs.rs     path helpers
```

- **Command model** — `core/command.rs` defines `FfmpegCommand` with multiple
  inputs, an output, optional video/audio codecs, a preset, and trailing
  `extra_args`. `cli.rs` converts the `Encode`/`Probe` args into this model.
- **Batch files** — `core/batch.rs::parse_flw_file` reads a `.flw` file into a
  list of commands; `cli.rs::parse_line` tokenizes each line with
  `shell_words` and re-parses it through the same clap `Cli`, so batch syntax
  is identical to CLI syntax.
- **Execution + progress** — `core/runner.rs` spawns `ffmpeg` per job and
  `core/progress.rs` interprets ffmpeg's `out=…` / `frame=…` / `fps=…` progress
  output into per-job state that the TUI renders. `core/metadata.rs` uses
  `ffmpeg -f null` to probe a file.

## Installation

Requires a Rust toolchain and a working `ffmpeg` on your `PATH`.

```bash
cargo build --release
# binary at target/release/ffflow
```

## Usage

Encode with explicit inputs/output and a preset:

```bash
ffflow encode -i input.mov -o out.mp4 --vcodec libx264 --preset veryfast
ffflow encode -i a.mov -i b.mov -o merged.mp4 --extra-args "-filter_complex concat"
```

Probe a file (runs `ffmpeg -f null`):

```bash
ffflow probe -i input.mov
```

List built-in presets:

```bash
ffflow presets
```

Batch mode via a `.flw` file — each line is a normal `ffflow` command:

```bash
ffflow pipeline.flw
```

```text
# pipeline.flw
encode -i clip1.mov -o clip1.mp4 --preset fast
encode -i clip2.mov -o clip2.mp4 --preset fast
encode -i clip1.mp4 -i clip2.mp4 -o final.mp4 --extra-args "-c copy"
```

Running `ffflow` (or `ffflow <file>`) opens the TUI, which shows one progress
track per job and a summary when all jobs finish.

## License

MIT
