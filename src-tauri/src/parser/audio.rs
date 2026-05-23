use std::path::Path;

use crate::{
    parser::video,
    types::{DocFormat, ParsedDoc},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    // Audio files share the exact same transcription pipeline as videos —
    // ffmpeg decodes the audio stream to 16 kHz mono PCM and whisper takes
    // it from there. Reuse the video parser to keep the "no audio track"
    // soft-error behaviour consistent.
    video::parse_with_format(path, DocFormat::Audio)
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
