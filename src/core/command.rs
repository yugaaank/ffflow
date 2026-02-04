#[derive(Debug, Clone)]
pub struct FfmpegCommand {
    pub inputs: Vec<String>,
    pub output: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub preset: Option<String>,
    pub extra_args: Vec<String>,
}

impl FfmpegCommand {
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        for input in &self.inputs {
            args.push("-i".to_string());
            args.push(input.clone());
        }

        if let Some(codec) = &self.video_codec {
            args.push("-c:v".to_string());
            args.push(codec.clone());
        }

        if let Some(codec) = &self.audio_codec {
            args.push("-c:a".to_string());
            args.push(codec.clone());
        }

        if let Some(preset) = &self.preset {
            args.push("-preset".to_string());
            args.push(preset.clone());
        }

        args.extend(self.extra_args.iter().cloned());
        args.push(self.output.clone());

        args
    }
}
