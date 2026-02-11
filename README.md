# ffflow (Foss Flow)

**ffflow** is a powerful, open-source terminal user interface (TUI) for FFmpeg. It is designed to streamline professional media workflows by providing a clean, interactive surface for running complex encoding, probing, and filtering jobs.

Built with **Rust**, it emphasizes reliability, observability, and ease of use for both casual users and media engineers.

![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Build Status](https://img.shields.io/badge/build-passing-brightgreen)

## ✨ Key Features

- **Interactive TUI**: Real-time progress monitoring with speed, frame count, and time estimates.
- **Batch Processing**: Run sequential jobs defined in simple `.flw` text files.
- **smart Wrappers**: Simplified commands (`encode`, `probe`) for common tasks with preset support.
- **FFmpeg Passthrough**: Full support for raw `ffmpeg` commands, including complex filtergraphs and multiline commands.
- **Interactive Prompts**: Handles FFmpeg interactive queries (like "Overwrite? [y/N]") directly in the UI.
- **Session History**: Scrollable history of all commands and outputs within the session.

## 🚀 Installation

Ensure you have [Rust](https://www.rust-lang.org/tools/install) and `ffmpeg` installed.

```bash
git clone https://github.com/yugaaank/ffflow
cd ffflow
cargo install --path .
```

## 📖 Usage

Start the TUI:
```bash
ffflow
```

### Basic Commands
Inside the TUI, you can type commands just like in a shell:

- **Encode Wrapper**:
  ```bash
  encode -i input.mp4 -o output.mp4 --vcodec libx264 --preset fast
  ```
- **Probe Wrapper**:
  ```bash
  probe -i input.mp4
  ```
- **Raw FFmpeg**:
  ```bash
  ffmpeg -i input.mp4 -c:v libx265 -crf 28 output.mp4
  ```

### Batch Processing (`.flw` files)
Create a workflow file (e.g., `jobs.flw`) to run multiple commands in sequence.

**Example `jobs.flw`:**
```bash
# Convert to simple MP4
ffmpeg -i raw.mov -c:v libx264 -c:a aac output.mp4

# Extract Audio
ffmpeg -i raw.mov -vn -c:a libmp3lame audio.mp3

# Complex Filtergraph (Multiline supported with \)
ffmpeg -i input.mp4 \
       -vf "scale=1280:720,format=gray" \
       -c:v libx264 bw_720p.mp4
```

**Run it:**
From the terminal:
```bash
ffflow jobs.flw
```
Or interactively inside `ffflow`:
```bash
batch jobs.flw
```

## ⚡ Interactive Input
`ffflow` intelligently detects when FFmpeg asks for confirmation (e.g., file overwrite) and allows you to respond with `y` or `n` directly from the TUI, preventing jobs from hanging in the background.

## ⚠️ Known Limitations
- **Shell Features**: Piping (`|`), redirection (`>`), and globbing (`*.mp4`) are not supported.
- **Complex Prompts**: Only standard overwrite prompts are currently interactive. Password prompts may hang.

## 🤝 Contributing
We welcome contributions! This project is FOSS and we believe in community-driven development.
1. Fork the repo.
2. Create feature branch (`git checkout -b feature/amazing-feature`).
3. Commit changes (`git commit -m 'Add amazing feature'`).
4. Push to branch (`git push origin feature/amazing-feature`).
5. Open a Pull Request.

## 📜 License
Distributed under the MIT License. See `LICENSE` for more information.
