//! Where audio an engine handed back (Kokoro, Supertonic, ElevenLabs,
//! Replicate) is played.
//!
//! `afplay` has no device option: it plays on the system output, whatever
//! Settings says. So with no speaker chosen it stays the player, and with one
//! chosen the clip is decoded here and played by [`super::pcm_player`], which
//! opens that device (LAC-9111).

use tauri_plugin_voice_transcription::DeviceChoice;

/// Which player a rendered clip goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Nothing chosen: follow the system output.
    SystemOutput,
    /// The speaker picked in Settings. If it is not connected the player
    /// falls back to the system output itself.
    Chosen(DeviceChoice),
}

/// The player for a clip, given the speaker stored in Settings.
pub fn route(chosen: Option<DeviceChoice>) -> Route {
    chosen.map_or(Route::SystemOutput, Route::Chosen)
}

/// The clip at `path` (whose contents are `bytes`) as mono samples and their
/// rate.
///
/// WAV is read here. Anything else (MP3, AAC), and any WAV `hound` refuses,
/// goes through `afconvert`, the decoder macOS ships, which identifies the
/// format from the contents rather than the file name.
pub async fn decode(path: &std::path::Path, bytes: &[u8]) -> Result<(Vec<f32>, u32), String> {
    use super::speech_level::decode_wav_mono;

    if let Some(decoded) = decode_wav_mono(bytes) {
        return Ok(decoded);
    }

    let wav = tempfile::Builder::new()
        .prefix("tts_pcm_")
        .suffix(".wav")
        .tempfile()
        .map_err(|e| format!("could not create a file to decode into: {e}"))?;
    let output = tokio::process::Command::new("/usr/bin/afconvert")
        .args(["-f", "WAVE", "-d", "LEI16"])
        .arg(path)
        .arg(wav.path())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("could not run afconvert: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "afconvert exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let converted = tokio::fs::read(wav.path())
        .await
        .map_err(|e| format!("could not read the decoded audio: {e}"))?;
    decode_wav_mono(&converted).ok_or_else(|| "the decoded audio was not readable".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn airpods() -> DeviceChoice {
        DeviceChoice {
            uid: Some("uid-airpods".to_string()),
            name: "AirPods Pro".to_string(),
        }
    }

    #[test]
    fn with_no_speaker_chosen_the_system_output_plays() {
        assert_eq!(route(None), Route::SystemOutput);
    }

    /// LAC-9111: every rendered engine went to `afplay`, which cannot be
    /// told a device, so a chosen speaker was ignored.
    #[test]
    fn a_chosen_speaker_is_not_played_on_the_system_output() {
        assert_eq!(route(Some(airpods())), Route::Chosen(airpods()));
    }

    /// 100 ms of a tone as 16-bit mono WAV, what Kokoro hands back.
    fn wav_clip() -> Vec<u8> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 24_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut bytes = std::io::Cursor::new(Vec::new());
        let written =
            hound::WavWriter::new(&mut bytes, spec).and_then(|mut writer| -> hound::Result<()> {
                for i in 0..2_400 {
                    writer.write_sample(((i as f32 * 0.1).sin() * 8_000.0) as i16)?;
                }
                writer.finalize()
            });
        assert!(written.is_ok(), "{written:?}");
        bytes.into_inner()
    }

    #[tokio::test]
    async fn a_wav_clip_decodes_at_its_own_rate() {
        // The path is never read: WAV is decoded from the bytes.
        let decoded = decode(std::path::Path::new("/nonexistent"), &wav_clip()).await;
        let decoded = decoded.map(|(samples, rate)| (samples.len(), rate));
        assert_eq!(decoded, Ok((2_400, 24_000)));
    }

    /// The cloud engines hand back compressed audio, and the temp file is
    /// named `.m4a` whatever is in it. AIFF stands in for "not WAV".
    #[tokio::test]
    async fn a_clip_that_is_not_wav_decodes_whatever_its_file_is_called() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let source = dir.path().join("source.wav");
        let clip = dir.path().join("clip.m4a");
        std::fs::write(&source, wav_clip()).map_err(|e| e.to_string())?;
        let made = std::process::Command::new("/usr/bin/afconvert")
            .args(["-f", "AIFF", "-d", "BEI16"])
            .arg(&source)
            .arg(&clip)
            .status()
            .map_err(|e| e.to_string())?;
        assert!(made.success());
        let bytes = std::fs::read(&clip).map_err(|e| e.to_string())?;
        assert!(super::super::speech_level::decode_wav_mono(&bytes).is_none());

        let (samples, rate) = decode(&clip, &bytes).await?;
        assert_eq!((samples.len(), rate), (2_400, 24_000));
        Ok(())
    }

    /// What ElevenLabs hands back, in a file named `.m4a` like every clip.
    /// The fixture is 100 ms of tone; MP3 pads both ends.
    #[tokio::test]
    async fn an_mp3_clip_decodes() -> Result<(), String> {
        let bytes: &[u8] = include_bytes!("testdata/tone.mp3");
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let clip = dir.path().join("clip.m4a");
        std::fs::write(&clip, bytes).map_err(|e| e.to_string())?;

        let (samples, rate) = decode(&clip, bytes).await?;
        assert_eq!(rate, 24_000);
        assert!(
            (2_400..=4_800).contains(&samples.len()),
            "{}",
            samples.len()
        );
        assert!(samples.iter().any(|s| s.abs() > 0.05), "decoded silence");
        Ok(())
    }

    #[tokio::test]
    async fn an_aac_clip_decodes() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let source = dir.path().join("source.wav");
        let clip = dir.path().join("clip.m4a");
        std::fs::write(&source, wav_clip()).map_err(|e| e.to_string())?;
        let made = std::process::Command::new("/usr/bin/afconvert")
            .args(["-f", "m4af", "-d", "aac"])
            .arg(&source)
            .arg(&clip)
            .status()
            .map_err(|e| e.to_string())?;
        assert!(made.success());
        let bytes = std::fs::read(&clip).map_err(|e| e.to_string())?;

        let (samples, rate) = decode(&clip, &bytes).await?;
        assert_eq!(rate, 24_000);
        assert!(
            (2_400..=4_800).contains(&samples.len()),
            "{}",
            samples.len()
        );
        assert!(samples.iter().any(|s| s.abs() > 0.05), "decoded silence");
        Ok(())
    }

    #[tokio::test]
    async fn bytes_that_are_not_audio_are_an_error() -> Result<(), String> {
        let file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        std::fs::write(file.path(), b"not audio").map_err(|e| e.to_string())?;
        assert!(decode(file.path(), b"not audio").await.is_err());
        Ok(())
    }
}
