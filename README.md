# ffflow

A CLI and TUI for ffmpeg workflows. Define your encodes once in a `.flw` file, run them, and watch everything progress in real time.

No more retyping long ffmpeg commands or maintaining shell scripts for batch jobs. `ffflow` wraps the ffmpeg binary on your PATH -- it doesn't reimplement any encoders.

## Install

```bash
cargo install --git https://github.com/yugaaank/ffflow
```

Or build from source:

```bash
git clone https://github.com/yugaaank/ffflow
cd ffflow
cargo build --release
```

You need a Rust toolchain and `ffmpeg` on your PATH.

## Usage

### Single encode

```bash
ffflow encode -i input.mov -o out.mp4 --vcodec libx264 --preset veryfast
```

### Multiple inputs

```bash
ffflow encode -i a.mov -i b.mov -o merged.mp4 --extra-args "-filter_complex concat"
```

### Probe a file

```bash
ffflow probe -i input.mov
```

### Batch mode

Write a `.flw` file where each line is an `ffflow encode` command:

```
encode -i clip1.mov -o clip1.mp4 --preset fast
encode -i clip2.mov -o clip2.mp4 --preset fast
encode -i clip1.mp4 -i clip2.mp4 -o final.mp4 --extra-args "-c copy"
```

Then run it:

```bash
ffflow pipeline.flw
```

This opens the TUI with a live progress track per job. A summary shows when all jobs finish.

### Presets

Built-in x264 presets: `ultrafast`, `superfast`, `veryfast`, `faster`, `fast`, `medium`, `slow`, `slower`, `veryslow`, `placebo`.

```bash
ffflow presets
```

## License

MIT
