use clap::{Parser, Subcommand};

use crate::core::command::FfmpegCommand;

#[derive(Debug, Parser)]
#[command(name = "ffx", version, about = "Professional ffmpeg wrapper")]
pub struct SystemCli {
    /// Path to a .flw file containing commands
    #[arg(value_name = "FILE")]
    pub file: Option<std::path::PathBuf>,
}

#[derive(Debug, Parser)]
#[command(name = "ffx", version, about = "Professional ffmpeg wrapper")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Encode(EncodeArgs),
    Probe(ProbeArgs),
    Presets,
}

#[derive(Debug, Parser)]
pub struct EncodeArgs {
    #[arg(short = 'i', long = "input", required = true)]
    pub inputs: Vec<String>,
    #[arg(short = 'o', long = "output")]
    pub output: String,
    #[arg(long = "vcodec")]
    pub video_codec: Option<String>,
    #[arg(long = "acodec")]
    pub audio_codec: Option<String>,
    #[arg(long = "preset")]
    pub preset: Option<String>,
    #[arg(last = true)]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct ProbeArgs {
    #[arg(short = 'i', long = "input")]
    pub input: String,
}

pub fn encode_args_to_command(args: EncodeArgs) -> FfmpegCommand {
    FfmpegCommand {
        inputs: args.inputs,
        output: args.output,
        video_codec: args.video_codec,
        audio_codec: args.audio_codec,
        preset: args.preset,
        extra_args: args.extra_args,
    }
}

pub fn probe_args_to_command(args: ProbeArgs) -> FfmpegCommand {
    FfmpegCommand {
        inputs: vec![args.input],
        output: "-".to_string(),
        video_codec: None,
        audio_codec: None,
        preset: None,
        extra_args: vec!["-f".to_string(), "null".to_string()],
    }
}

pub fn parse_line(line: &str) -> Result<Commands, String> {
    let mut argv = Vec::new();
    argv.push("ffx".to_string());

    let tokens = shell_words::split(line).map_err(|err| err.to_string())?;
    argv.extend(tokens);

    let parsed = Cli::try_parse_from(argv).map_err(|err| err.to_string())?;
    Ok(parsed.command)
}

pub const PRESETS: [&str; 10] = [
    "ultrafast",
    "superfast",
    "veryfast",
    "faster",
    "fast",
    "medium",
    "slow",
    "slower",
    "veryslow",
    "placebo",
];
