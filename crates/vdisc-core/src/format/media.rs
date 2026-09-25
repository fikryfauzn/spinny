use super::{FormatError, FormatErrorKind as K, FormatResult, TrackEntry, zip::Entry};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
};
use symphonia::{
    core::{
        codecs::{
            CodecParameters,
            audio::{AudioCodecId, AudioDecoderOptions, well_known::*},
            registry::CodecRegistry,
        },
        common::Limit,
        formats::{FormatOptions, TrackType, probe::Hint},
        io::{MediaSource, MediaSourceStream},
        meta::MetadataOptions,
    },
    default,
};
use symphonia_adapter_libopus::OpusDecoder;

struct EntrySource {
    file: File,
    start: u64,
    len: u64,
    pos: u64,
}
impl Read for EntrySource {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let count = (self.len - self.pos).min(b.len() as u64) as usize;
        self.file.seek(SeekFrom::Start(
            self.start
                .checked_add(self.pos)
                .ok_or_else(|| io::Error::other("offset overflow"))?,
        ))?;
        let n = self.file.read(&mut b[..count])?;
        self.pos += n as u64;
        Ok(n)
    }
}
impl Seek for EntrySource {
    fn seek(&mut self, s: SeekFrom) -> io::Result<u64> {
        let pos = match s {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.pos) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.len) + i128::from(n),
        };
        if pos < 0 || pos > i128::from(self.len) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek outside embedded track",
            ));
        }
        self.pos = pos as u64;
        Ok(self.pos)
    }
}
impl MediaSource for EntrySource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

pub(crate) fn is_pcm(codec: AudioCodecId) -> bool {
    [
        CODEC_ID_PCM_S32LE,
        CODEC_ID_PCM_S32LE_PLANAR,
        CODEC_ID_PCM_S32BE,
        CODEC_ID_PCM_S32BE_PLANAR,
        CODEC_ID_PCM_S24LE,
        CODEC_ID_PCM_S24LE_PLANAR,
        CODEC_ID_PCM_S24BE,
        CODEC_ID_PCM_S24BE_PLANAR,
        CODEC_ID_PCM_S16LE,
        CODEC_ID_PCM_S16LE_PLANAR,
        CODEC_ID_PCM_S16BE,
        CODEC_ID_PCM_S16BE_PLANAR,
        CODEC_ID_PCM_S8,
        CODEC_ID_PCM_S8_PLANAR,
        CODEC_ID_PCM_U32LE,
        CODEC_ID_PCM_U32LE_PLANAR,
        CODEC_ID_PCM_U32BE,
        CODEC_ID_PCM_U32BE_PLANAR,
        CODEC_ID_PCM_U24LE,
        CODEC_ID_PCM_U24LE_PLANAR,
        CODEC_ID_PCM_U24BE,
        CODEC_ID_PCM_U24BE_PLANAR,
        CODEC_ID_PCM_U16LE,
        CODEC_ID_PCM_U16LE_PLANAR,
        CODEC_ID_PCM_U16BE,
        CODEC_ID_PCM_U16BE_PLANAR,
        CODEC_ID_PCM_U8,
        CODEC_ID_PCM_U8_PLANAR,
        CODEC_ID_PCM_F32LE,
        CODEC_ID_PCM_F32LE_PLANAR,
        CODEC_ID_PCM_F32BE,
        CODEC_ID_PCM_F32BE_PLANAR,
        CODEC_ID_PCM_F64LE,
        CODEC_ID_PCM_F64LE_PLANAR,
        CODEC_ID_PCM_F64BE,
        CODEC_ID_PCM_F64BE_PLANAR,
        CODEC_ID_PCM_ALAW,
        CODEC_ID_PCM_MULAW,
    ]
    .contains(&codec)
}

fn probe_audio(file: &File, start: u64, len: u64, expected: &TrackEntry) -> FormatResult<()> {
    let fail = |s: String| FormatError::new(K::UnsupportedMedia, s).at(&expected.path);
    let source = EntrySource {
        file: file.try_clone()?,
        start,
        len,
        pos: 0,
    };
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let metadata = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(1024 * 1024))
        .limit_visual_bytes(Limit::Maximum(0));
    let mut format = default::get_probe()
        .probe(&Hint::new(), stream, FormatOptions::default(), metadata)
        .map_err(|e| fail(e.to_string()))?;
    let track = format
        .default_track(TrackType::Audio)
        .cloned()
        .ok_or_else(|| fail("no audio track".into()))?;
    let params = match track.codec_params {
        Some(CodecParameters::Audio(p)) => p,
        _ => return Err(fail("no audio parameters".into())),
    };
    let container = format.format_info().short_name;
    let pair = if params.codec == CODEC_ID_FLAC && container == "flac" {
        ("flac", "flac")
    } else if params.codec == CODEC_ID_MP3 && ["mpa", "mp3"].contains(&container) {
        ("mp3", "mp3")
    } else if params.codec == CODEC_ID_OPUS && container == "ogg" {
        ("ogg", "opus")
    } else if is_pcm(params.codec) && ["wave", "wav"].contains(&container) {
        ("wav", "pcm")
    } else {
        return Err(fail("unsupported actual container/codec".into()));
    };
    if pair != (expected.container.as_str(), expected.codec.as_str()) {
        return Err(fail("manifest does not describe actual audio".into()));
    }
    if let (Some(declared), Some(actual)) = (expected.sample_rate_hz, params.sample_rate)
        && declared != actual
    {
        return Err(fail("sample rate mismatch".into()));
    }
    if let (Some(declared), Some(actual)) = (expected.channels, params.channels.as_ref())
        && usize::from(declared) != actual.count()
    {
        return Err(fail("channel count mismatch".into()));
    }
    let mut registry = CodecRegistry::new();
    default::register_enabled_codecs(&mut registry);
    registry.register_audio_decoder::<OpusDecoder>();
    let mut decoder = registry
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| fail(e.to_string()))?;
    while let Some(packet) = format.next_packet().map_err(|e| fail(e.to_string()))? {
        if packet.track_id != track.id {
            continue;
        }
        decoder.decode(&packet).map_err(|e| fail(e.to_string()))?;
        return Ok(());
    }
    Err(fail("no decodable audio packet".into()))
}

/// Isolate allocations performed by demuxers and decoders; entry length is never
/// used as an allocation size. The original open inode remains held by the parent.
pub(crate) fn validate_audio(
    file: &File,
    entry: &Entry,
    expected: &TrackEntry,
) -> FormatResult<()> {
    #[cfg(target_os = "linux")]
    {
        use std::{
            io::Write,
            os::fd::AsRawFd,
            process::{Command, Stdio},
        };
        let source = format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd());
        let mut child=Command::new(super::artwork::worker_path("vdisc-media-check","VDISC_MEDIA_WORKER")?)
            .args([source,entry.data.to_string(),entry.size.to_string()])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
            .map_err(|e|FormatError::new(K::ResourceLimit,format!("media worker unavailable: {e}; build/install vdisc-media-check beside the application")))?;
        let bytes = serde_json::to_vec(expected)
            .map_err(|e| FormatError::new(K::InvalidSchema, e.to_string()))?;
        let written = child
            .stdin
            .take()
            .ok_or_else(|| FormatError::new(K::Io, "worker stdin missing"))?
            .write_all(&bytes);
        let mut response = Vec::new();
        let read = child
            .stdout
            .take()
            .ok_or_else(|| FormatError::new(K::Io, "worker stdout missing"))?
            .take(4096)
            .read_to_end(&mut response);
        let status = child.wait()?;
        if !status.success() {
            let kind = if status.code() == Some(2) {
                K::UnsupportedMedia
            } else {
                K::ResourceLimit
            };
            let message = if response.is_empty() {
                "media worker rejected audio or exceeded its resource budget".to_owned()
            } else {
                String::from_utf8_lossy(&response).into_owned()
            };
            return Err(FormatError::new(kind, message).at(&entry.name));
        }
        written?;
        read?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (file, entry, expected);
        Err(FormatError::new(
            K::ResourceLimit,
            "bounded media worker requires Linux",
        ))
    }
}

/// Internal protocol for the dedicated, resource-limited media process.
#[doc(hidden)]
pub fn media_worker_main() -> i32 {
    use std::io::Write;
    if !super::artwork::worker_limits() {
        return 3;
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return 3;
    }
    let Some(start) = args[1].to_str().and_then(|v| v.parse::<u64>().ok()) else {
        return 3;
    };
    let Some(len) = args[2].to_str().and_then(|v| v.parse::<u64>().ok()) else {
        return 3;
    };
    let Ok(file) = File::open(&args[0]) else {
        return 3;
    };
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take(super::MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > super::MAX_MANIFEST_BYTES
    {
        return 3;
    }
    let Ok(expected) = serde_json::from_slice::<TrackEntry>(&bytes) else {
        return 3;
    };
    match probe_audio(&file, start, len, &expected) {
        Ok(()) => 0,
        Err(error) => {
            let message = error.to_string();
            let bytes = message.as_bytes();
            let _ = std::io::stdout().write_all(&bytes[..bytes.len().min(4096)]);
            2
        }
    }
}
