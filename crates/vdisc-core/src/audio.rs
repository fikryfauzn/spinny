use std::{
    fs::File,
    path::{Path, PathBuf},
};

use symphonia::{
    core::{
        codecs::{
            CodecParameters,
            audio::{
                AudioCodecId, AudioDecoderOptions,
                well_known::{
                    CODEC_ID_AAC, CODEC_ID_ALAC, CODEC_ID_FLAC, CODEC_ID_MP1, CODEC_ID_MP2,
                    CODEC_ID_MP3, CODEC_ID_OPUS, CODEC_ID_VORBIS,
                },
            },
            registry::CodecRegistry,
        },
        formats::{FormatOptions, TrackType, probe::Hint},
        io::MediaSourceStream,
        meta::MetadataOptions,
    },
    default,
};
use symphonia_adapter_libopus::OpusDecoder;
use uuid::Uuid;

use crate::{LocalFileSelection, Result, SourceFingerprint, VdiscError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLocalAudio {
    target_path: PathBuf,
    target_disc_id: Uuid,

    source_path: PathBuf,
    source_fingerprint: SourceFingerprint,

    container: String,
    codec: String,

    sample_rate: Option<u32>,
    channels: Option<u16>,
    duration_ms: Option<u64>,
}

impl ValidatedLocalAudio {
    pub fn validate(selection: LocalFileSelection) -> Result<Self> {
        let target_path = selection.target_path().to_path_buf();
        let target_disc_id = selection.target_disc_id();

        let source_path = selection.source_path().to_path_buf();

        let file = File::open(&source_path)?;

        let stream = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();

        if let Some(extension) = source_path
            .extension()
            .and_then(|extension| extension.to_str())
        {
            hint.with_extension(extension);
        }

        let mut format = default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|error| audio_error(&source_path, format!("media probe failed: {error}")))?;

        let track = format
            .default_track(TrackType::Audio)
            .cloned()
            .ok_or_else(|| audio_error(&source_path, "no audio track found"))?;

        let audio_params = match track.codec_params {
            Some(CodecParameters::Audio(params)) => params,

            _ => {
                return Err(audio_error(
                    &source_path,
                    "audio codec parameters are missing",
                ));
            }
        };

        let mut codecs = CodecRegistry::new();

        default::register_enabled_codecs(&mut codecs);

        codecs.register_audio_decoder::<OpusDecoder>();

        let mut decoder = codecs
            .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())
            .map_err(|error| {
                audio_error(
                    &source_path,
                    format!("unsupported or unusable codec: {error}"),
                )
            })?;

        let mut decoded_packet = false;

        while let Some(packet) = format.next_packet().map_err(|error| {
            audio_error(
                &source_path,
                format!("failed reading media packet: {error}"),
            )
        })? {
            if packet.track_id != track.id {
                continue;
            }

            decoder.decode(&packet).map_err(|error| {
                audio_error(
                    &source_path,
                    format!("audio packet could not be decoded: {error}"),
                )
            })?;

            decoded_packet = true;

            break;
        }

        if !decoded_packet {
            return Err(audio_error(
                &source_path,
                "audio track contained no decodable packets",
            ));
        }

        /*
         * Fingerprint only after the file has passed actual
         * audio validation.
         *
         * This fingerprint now represents the exact source
         * bytes that VDISC successfully validated.
         */
        let source_fingerprint = SourceFingerprint::from_file(&source_path).map_err(|error| {
            audio_error(
                &source_path,
                format!("failed fingerprinting validated audio: {error}"),
            )
        })?;

        let container = normalize_container(format.format_info().short_name);

        let codec = codec_name(audio_params.codec, &container);

        let sample_rate = audio_params.sample_rate;

        let channels = audio_params
            .channels
            .map(|channels| channels.count() as u16);

        let duration_ms = match (track.time_base, track.duration) {
            (Some(time_base), Some(duration)) => {
                let seconds = f64::from(time_base) * duration.get() as f64;

                Some((seconds * 1000.0).round() as u64)
            }

            _ => None,
        };

        Ok(Self {
            target_path,
            target_disc_id,

            source_path,
            source_fingerprint,

            container,
            codec,

            sample_rate,
            channels,
            duration_ms,
        })
    }

    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    pub fn target_disc_id(&self) -> Uuid {
        self.target_disc_id
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn source_fingerprint(&self) -> &SourceFingerprint {
        &self.source_fingerprint
    }

    pub fn container(&self) -> &str {
        &self.container
    }

    pub fn codec(&self) -> &str {
        &self.codec
    }

    pub fn sample_rate(&self) -> Option<u32> {
        self.sample_rate
    }

    pub fn channels(&self) -> Option<u16> {
        self.channels
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }
}

fn audio_error(path: &Path, reason: impl Into<String>) -> VdiscError {
    VdiscError::AudioValidation {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

fn normalize_container(container: &str) -> String {
    match container {
        "wave" => "wav".to_string(),

        other => other.to_string(),
    }
}

fn codec_name(codec: AudioCodecId, container: &str) -> String {
    if codec == CODEC_ID_FLAC {
        "flac".to_string()
    } else if codec == CODEC_ID_MP3 {
        "mp3".to_string()
    } else if codec == CODEC_ID_MP2 {
        "mp2".to_string()
    } else if codec == CODEC_ID_MP1 {
        "mp1".to_string()
    } else if codec == CODEC_ID_OPUS {
        "opus".to_string()
    } else if codec == CODEC_ID_VORBIS {
        "vorbis".to_string()
    } else if codec == CODEC_ID_AAC {
        "aac".to_string()
    } else if codec == CODEC_ID_ALAC {
        "alac".to_string()
    } else if container == "wav" {
        "pcm".to_string()
    } else {
        codec.to_string()
    }
}
