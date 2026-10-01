//! Linux audio playback for validated VDISC artifacts.
//!
//! The Objective 16 `Player` remains the transport/state authority. This module
//! adds a testable playback coordinator plus the real CPAL/Symphonia Linux
//! backend. Decoder output is streamed through a bounded PCM queue; complete
//! tracks are never loaded into memory.

use std::{
    collections::VecDeque,
    error::Error,
    fmt,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{
        Arc, Condvar, Mutex, TryLockError,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use symphonia::{
    core::{
        codecs::{
            CodecParameters,
            audio::{AudioDecoder, AudioDecoderOptions},
            registry::CodecRegistry,
        },
        common::Limit,
        formats::{FormatOptions, FormatReader, SeekMode, SeekTo, Track, TrackType, probe::Hint},
        io::{MediaSource, MediaSourceStream},
        meta::MetadataOptions,
        units::Time,
    },
    default,
};
use symphonia_adapter_libopus::OpusDecoder;

use crate::{
    BurnedDisc, DiscPayload, Player, PlayerAction, PlayerError, PlayerState,
    format::{FormatError, TrackEntry},
};

const PCM_BUFFER_MS: u64 = 500;

#[derive(Debug)]
pub enum PlaybackError {
    Player(PlayerError),
    Disc(FormatError),
    NoOutputDevice,
    UnsupportedOutput { sample_rate_hz: u32 },
    Device(String),
    Decode(String),
    BackendInvariant(&'static str),
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Player(error) => write!(f, "{error}"),
            Self::Disc(error) => write!(f, "VDISC payload error: {error}"),
            Self::NoOutputDevice => write!(f, "no Linux audio output device is available"),
            Self::UnsupportedOutput { sample_rate_hz } => write!(
                f,
                "no output configuration supports {sample_rate_hz} Hz PCM playback"
            ),
            Self::Device(message) => write!(f, "audio device error: {message}"),
            Self::Decode(message) => write!(f, "audio decode error: {message}"),
            Self::BackendInvariant(message) => {
                write!(f, "audio backend invariant failed: {message}")
            }
        }
    }
}

impl Error for PlaybackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Player(error) => Some(error),
            Self::Disc(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PlayerError> for PlaybackError {
    fn from(error: PlayerError) -> Self {
        Self::Player(error)
    }
}

impl From<FormatError> for PlaybackError {
    fn from(error: FormatError) -> Self {
        Self::Disc(error)
    }
}

pub type PlaybackResult<T> = std::result::Result<T, PlaybackError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackBackendEvent {
    TrackFinished,
    DeviceError(String),
    DecodeError(String),
}

/// One backend-owned playback session for the currently selected track.
///
/// Sessions are always created paused. The coordinator calls `play()` only
/// after construction succeeds, which keeps failed device setup from changing
/// the Objective 16 transport state.
pub trait PlaybackSession: Send {
    fn play(&self) -> PlaybackResult<()>;
    fn pause(&self) -> PlaybackResult<()>;
    fn position_ms(&self) -> u64;
    fn poll_event(&mut self) -> Option<PlaybackBackendEvent>;
}

/// Dependency-inversion seam for the Objective 17 playback coordinator.
///
/// The production implementation is `CpalBackend`. Tests can supply a fake
/// backend so transport behavior never depends on a real sound card.
pub trait PlaybackBackend {
    type Session: PlaybackSession;

    fn open_session(
        &mut self,
        disc: &BurnedDisc,
        track_index: usize,
        position_ms: u64,
    ) -> PlaybackResult<Self::Session>;
}

/// Player state + audio backend coordination.
///
/// Backend events are consumed by `poll()`. A normal application event loop
/// should call it regularly; `play_to_end()` provides the blocking CLI harness
/// used by Campaign 01.
#[derive(Debug)]
pub struct PlaybackPlayer<B: PlaybackBackend> {
    player: Player,
    backend: B,
    session: Option<B::Session>,
}

impl<B: PlaybackBackend> PlaybackPlayer<B> {
    pub fn with_backend(backend: B) -> Self {
        Self {
            player: Player::new(),
            backend,
            session: None,
        }
    }

    pub fn state(&self) -> PlayerState {
        self.player.state()
    }

    pub fn disc(&self) -> Option<&BurnedDisc> {
        self.player.disc()
    }

    pub fn track_count(&self) -> usize {
        self.player.track_count()
    }

    pub fn current_track_index(&self) -> Option<usize> {
        self.player.current_track_index()
    }

    pub fn current_track(&self) -> Option<&TrackEntry> {
        self.player.current_track()
    }

    pub fn position_ms(&self) -> u64 {
        match self.player.state() {
            PlayerState::Playing | PlayerState::Paused => self
                .session
                .as_ref()
                .map_or_else(|| self.player.position_ms(), PlaybackSession::position_ms),
            PlayerState::Empty | PlayerState::Stopped => self.player.position_ms(),
        }
    }

    pub fn insert(&mut self, path: impl AsRef<Path>) -> PlaybackResult<()> {
        self.poll()?;
        self.player.insert(path)?;
        Ok(())
    }

    pub fn eject(&mut self) -> PlaybackResult<()> {
        self.poll()?;
        self.player.eject()?;
        self.session.take();
        Ok(())
    }

    pub fn play(&mut self) -> PlaybackResult<()> {
        self.poll()?;

        match self.player.state() {
            PlayerState::Stopped => {
                let session = self.open_current_session()?;
                session.play()?;
                self.player.play()?;
                self.session = Some(session);
                Ok(())
            }
            PlayerState::Paused => {
                let session = self
                    .session
                    .as_ref()
                    .ok_or(PlaybackError::BackendInvariant(
                        "paused player has no audio session",
                    ))?;
                session.play()?;
                self.player.play()?;
                Ok(())
            }
            _ => {
                self.player.play()?;
                Ok(())
            }
        }
    }

    pub fn pause(&mut self) -> PlaybackResult<()> {
        self.poll()?;

        if self.player.state() == PlayerState::Playing {
            let session = self
                .session
                .as_ref()
                .ok_or(PlaybackError::BackendInvariant(
                    "playing player has no audio session",
                ))?;
            session.pause()?;
        }

        self.player.pause()?;
        Ok(())
    }

    pub fn stop(&mut self) -> PlaybackResult<()> {
        self.poll()?;
        self.player.stop()?;
        self.session.take();
        Ok(())
    }

    pub fn next_track(&mut self) -> PlaybackResult<()> {
        self.navigate(PlayerAction::Next)
    }

    pub fn previous(&mut self) -> PlaybackResult<()> {
        self.navigate(PlayerAction::Previous)
    }

    pub fn seek(&mut self, position_ms: u64) -> PlaybackResult<()> {
        self.poll()?;

        let state = self.player.state();
        let old_position = self.player.position_ms();
        self.player.seek(position_ms)?;

        let replacement = match self.open_current_session() {
            Ok(session) => session,
            Err(error) => {
                self.restore_seek(old_position);
                return Err(error);
            }
        };

        if state == PlayerState::Playing
            && let Err(error) = replacement.play()
        {
            self.restore_seek(old_position);
            return Err(error);
        }

        self.session = Some(replacement);
        Ok(())
    }

    /// Consume asynchronous decoder/device events.
    ///
    /// EOF advances through the Objective 16 `track_finished()` transition.
    /// Intermediate tracks immediately open and start the next embedded payload.
    /// Finishing the final track returns to track 1 in `Stopped`.
    ///
    /// Decoder/device failures stop playback and return a controlled error; they
    /// never panic or leave the public state as `Playing` without a session.
    pub fn poll(&mut self) -> PlaybackResult<()> {
        loop {
            let event = self.session.as_mut().and_then(PlaybackSession::poll_event);

            let Some(event) = event else {
                return Ok(());
            };

            match event {
                PlaybackBackendEvent::TrackFinished => {
                    self.session.take();
                    self.player.track_finished()?;

                    if self.player.state() == PlayerState::Playing {
                        let next = match self.open_current_session() {
                            Ok(session) => session,
                            Err(error) => {
                                self.force_stopped();
                                return Err(error);
                            }
                        };

                        if let Err(error) = next.play() {
                            self.force_stopped();
                            return Err(error);
                        }

                        self.session = Some(next);
                    }
                }
                PlaybackBackendEvent::DeviceError(message) => {
                    self.session.take();
                    self.force_stopped();
                    return Err(PlaybackError::Device(message));
                }
                PlaybackBackendEvent::DecodeError(message) => {
                    self.session.take();
                    self.force_stopped();
                    return Err(PlaybackError::Decode(message));
                }
            }
        }
    }

    /// Blocking backend harness used by `vdisc play`.
    ///
    /// This is intentionally not a product UI. It simply pumps backend events
    /// until normal final-track behavior returns the player to `Stopped`.
    pub fn play_to_end(&mut self) -> PlaybackResult<()> {
        self.play()?;

        loop {
            self.poll()?;
            if self.player.state() != PlayerState::Playing {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn navigate(&mut self, action: PlayerAction) -> PlaybackResult<()> {
        self.poll()?;

        let state = self.player.state();
        let old_position = self.player.position_ms();

        match action {
            PlayerAction::Next => self.player.next_track()?,
            PlayerAction::Previous => self.player.previous()?,
            _ => {
                return Err(PlaybackError::BackendInvariant(
                    "navigate called with a non-navigation action",
                ));
            }
        }

        if !matches!(state, PlayerState::Playing | PlayerState::Paused) {
            return Ok(());
        }

        let replacement = match self.open_current_session() {
            Ok(session) => session,
            Err(error) => {
                self.rollback_navigation(action, old_position);
                return Err(error);
            }
        };

        if state == PlayerState::Playing
            && let Err(error) = replacement.play()
        {
            self.rollback_navigation(action, old_position);
            return Err(error);
        }

        self.session = Some(replacement);
        Ok(())
    }

    fn open_current_session(&mut self) -> PlaybackResult<B::Session> {
        let index = self
            .player
            .current_track_index()
            .ok_or(PlaybackError::BackendInvariant(
                "loaded player has no selected track",
            ))?;
        let disc = self
            .player
            .disc()
            .cloned()
            .ok_or(PlaybackError::BackendInvariant("loaded player has no disc"))?;

        self.backend
            .open_session(&disc, index, self.player.position_ms())
    }

    fn rollback_navigation(&mut self, action: PlayerAction, old_position: u64) {
        let rolled_back = match action {
            PlayerAction::Next => self.player.previous(),
            PlayerAction::Previous => self.player.next_track(),
            _ => return,
        };

        if rolled_back.is_ok()
            && matches!(
                self.player.state(),
                PlayerState::Playing | PlayerState::Paused
            )
        {
            let _ = self.player.seek(old_position);
        }
    }

    fn restore_seek(&mut self, position_ms: u64) {
        let _ = self.player.seek(position_ms);
    }

    fn force_stopped(&mut self) {
        if !matches!(
            self.player.state(),
            PlayerState::Empty | PlayerState::Stopped
        ) {
            let _ = self.player.stop();
        }
    }
}

#[derive(Debug, Default)]
pub struct CpalBackend;

pub type LinuxAudioPlayer = PlaybackPlayer<CpalBackend>;

impl PlaybackPlayer<CpalBackend> {
    pub fn new() -> Self {
        Self::with_backend(CpalBackend)
    }
}

impl Default for PlaybackPlayer<CpalBackend> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StreamInfo {
    sample_rate_hz: u32,
    channels: usize,
}

#[derive(Debug)]
struct ReaderSource<R> {
    inner: R,
    len: Option<u64>,
}

impl<R: Read> Read for ReaderSource<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buffer)
    }
}

impl<R: Seek> Seek for ReaderSource<R> {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(from)
    }
}

impl<R> MediaSource for ReaderSource<R>
where
    R: Read + Seek + Send + Sync,
{
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

type OpenedDecoder = (
    Box<dyn FormatReader>,
    Box<dyn AudioDecoder>,
    Track,
    StreamInfo,
);

fn open_decoder<R>(reader: R, len: Option<u64>) -> std::result::Result<OpenedDecoder, String>
where
    R: Read + Seek + Send + Sync + 'static,
{
    let source = ReaderSource { inner: reader, len };
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let metadata = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(1024 * 1024))
        .limit_visual_bytes(Limit::Maximum(0));

    let format = default::get_probe()
        .probe(&Hint::new(), stream, FormatOptions::default(), metadata)
        .map_err(|error| error.to_string())?;

    let track = format
        .default_track(TrackType::Audio)
        .cloned()
        .ok_or_else(|| "embedded media has no audio track".to_owned())?;

    let params = match track.codec_params.clone() {
        Some(CodecParameters::Audio(params)) => params,
        _ => return Err("embedded media has no audio codec parameters".to_owned()),
    };

    let sample_rate_hz = params
        .sample_rate
        .ok_or_else(|| "embedded audio has no sample rate".to_owned())?;
    let channels = params
        .channels
        .as_ref()
        .map(|channels| channels.count())
        .ok_or_else(|| "embedded audio has no channel layout".to_owned())?;

    if channels == 0 {
        return Err("embedded audio reports zero channels".to_owned());
    }

    let mut registry = CodecRegistry::new();
    default::register_enabled_codecs(&mut registry);
    registry.register_audio_decoder::<OpusDecoder>();

    let decoder = registry
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|error| error.to_string())?;

    Ok((
        format,
        decoder,
        track,
        StreamInfo {
            sample_rate_hz,
            channels,
        },
    ))
}

fn probe_audio_info(payload: DiscPayload) -> PlaybackResult<StreamInfo> {
    let len = payload.len();
    let (_, _, _, info) = open_decoder(payload, Some(len)).map_err(PlaybackError::Decode)?;
    Ok(info)
}

fn decode_audio<R, Emit>(
    reader: R,
    len: Option<u64>,
    start_ms: u64,
    expected: StreamInfo,
    output_channels: usize,
    mut emit: Emit,
) -> std::result::Result<(), String>
where
    R: Read + Seek + Send + Sync + 'static,
    Emit: FnMut(&[f32]) -> std::result::Result<(), String>,
{
    let (mut format, mut decoder, track, actual_info) = open_decoder(reader, len)?;

    if actual_info != expected {
        return Err("audio parameters changed between probe and decode".to_owned());
    }

    let mut skip_frames = 0u64;
    if start_ms != 0 {
        let time_base = track
            .time_base
            .ok_or_else(|| "embedded media has no seek time base".to_owned())?;
        let required_time = Time::from_millis_u64(start_ms);
        let required_ts = time_base
            .calc_timestamp(required_time)
            .ok_or_else(|| "seek target cannot be represented by media time base".to_owned())?;

        let seeked = format
            .seek(
                SeekMode::Accurate,
                SeekTo::Timestamp {
                    ts: required_ts,
                    track_id: track.id,
                },
            )
            .map_err(|error| error.to_string())?;
        decoder.reset();

        let actual_time = time_base
            .calc_time(seeked.actual_ts)
            .ok_or_else(|| "seek result cannot be converted to time".to_owned())?;
        let delta_ns = required_time
            .as_nanos()
            .saturating_sub(actual_time.as_nanos())
            .max(0) as u128;
        let frames =
            delta_ns.saturating_mul(u128::from(expected.sample_rate_hz)) / 1_000_000_000u128;
        skip_frames = frames.min(u128::from(u64::MAX)) as u64;
    }

    let mut decoded_interleaved = Vec::<f32>::new();
    let mut remixed = Vec::<f32>::new();

    while let Some(packet) = format.next_packet().map_err(|error| error.to_string())? {
        if packet.track_id != track.id {
            continue;
        }

        let decoded = decoder.decode(&packet).map_err(|error| error.to_string())?;

        let spec = decoded.spec();
        let channels = spec.channels().count();
        if spec.rate() != expected.sample_rate_hz || channels != expected.channels {
            return Err("audio parameters changed during decode".to_owned());
        }

        decoded_interleaved.clear();
        decoded.copy_to_vec_interleaved(&mut decoded_interleaved);

        let frame_count = decoded_interleaved.len() / expected.channels;
        let skipped = usize::try_from(skip_frames.min(frame_count as u64))
            .map_err(|_| "seek frame count does not fit platform usize".to_owned())?;
        skip_frames -= skipped as u64;

        if skipped == frame_count {
            continue;
        }

        let start = skipped
            .checked_mul(expected.channels)
            .ok_or_else(|| "decoded sample offset overflow".to_owned())?;

        remix_channels(
            &decoded_interleaved[start..],
            expected.channels,
            output_channels,
            &mut remixed,
        )?;

        if !remixed.is_empty() {
            emit(&remixed)?;
        }
    }

    let _ = decoder.finalize();
    Ok(())
}

fn remix_channels(
    input: &[f32],
    input_channels: usize,
    output_channels: usize,
    output: &mut Vec<f32>,
) -> std::result::Result<(), String> {
    if input_channels == 0 || output_channels == 0 {
        return Err("cannot remix zero-channel audio".to_owned());
    }
    if !input.len().is_multiple_of(input_channels) {
        return Err("decoded PCM is not frame-aligned".to_owned());
    }

    let frames = input.len() / input_channels;
    output.clear();
    output
        .try_reserve(frames.saturating_mul(output_channels))
        .map_err(|_| "could not allocate PCM remix buffer".to_owned())?;

    if input_channels == output_channels {
        output.extend_from_slice(input);
        return Ok(());
    }

    for frame in input.chunks_exact(input_channels) {
        if input_channels == 1 {
            output.extend(std::iter::repeat_n(frame[0], output_channels));
        } else if output_channels == 1 {
            let sum: f32 = frame.iter().copied().sum();
            output.push(sum / input_channels as f32);
        } else {
            for channel in 0..output_channels {
                output.push(frame.get(channel).copied().unwrap_or(0.0));
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct PcmState {
    samples: VecDeque<f32>,
    capacity: usize,
    producer_done: bool,
    decode_error: Option<String>,
}

#[derive(Debug)]
struct SharedPcm {
    state: Mutex<PcmState>,
    space_available: Condvar,
    stop: AtomicBool,
    completion_sent: AtomicBool,
    played_frames: AtomicU64,
}

impl SharedPcm {
    fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(PcmState {
                samples: VecDeque::with_capacity(capacity),
                capacity,
                producer_done: false,
                decode_error: None,
            }),
            space_available: Condvar::new(),
            stop: AtomicBool::new(false),
            completion_sent: AtomicBool::new(false),
            played_frames: AtomicU64::new(0),
        }
    }

    fn push(&self, samples: &[f32]) -> std::result::Result<(), String> {
        let mut offset = 0;

        while offset < samples.len() {
            if self.stop.load(Ordering::Acquire) {
                return Ok(());
            }

            let mut state = self
                .state
                .lock()
                .map_err(|_| "PCM queue lock was poisoned".to_owned())?;

            while state.samples.len() == state.capacity && !self.stop.load(Ordering::Acquire) {
                state = self
                    .space_available
                    .wait(state)
                    .map_err(|_| "PCM queue lock was poisoned".to_owned())?;
            }

            if self.stop.load(Ordering::Acquire) {
                return Ok(());
            }

            let free = state.capacity.saturating_sub(state.samples.len());
            let count = free.min(samples.len() - offset);
            state.samples.extend(&samples[offset..offset + count]);
            offset += count;
        }

        Ok(())
    }

    fn finish(&self, error: Option<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.producer_done = true;
            state.decode_error = error;
        }
        self.space_available.notify_all();
    }
}

pub struct CpalSession {
    stream: cpal::Stream,
    shared: Arc<SharedPcm>,
    events: Receiver<PlaybackBackendEvent>,
    decoder_thread: Option<JoinHandle<()>>,
    base_position_ms: u64,
    sample_rate_hz: u32,
}

impl Drop for CpalSession {
    fn drop(&mut self) {
        let _ = self.stream.pause();
        self.shared.stop.store(true, Ordering::Release);
        self.shared.space_available.notify_all();

        if let Some(thread) = self.decoder_thread.take() {
            let _ = thread.join();
        }
    }
}

impl PlaybackSession for CpalSession {
    fn play(&self) -> PlaybackResult<()> {
        self.stream
            .play()
            .map_err(|error| PlaybackError::Device(error.to_string()))
    }

    fn pause(&self) -> PlaybackResult<()> {
        self.stream
            .pause()
            .map_err(|error| PlaybackError::Device(error.to_string()))
    }

    fn position_ms(&self) -> u64 {
        let frames = self.shared.played_frames.load(Ordering::Acquire);
        self.base_position_ms.saturating_add(
            frames
                .saturating_mul(1000)
                .checked_div(u64::from(self.sample_rate_hz))
                .unwrap_or(0),
        )
    }

    fn poll_event(&mut self) -> Option<PlaybackBackendEvent> {
        self.events.try_recv().ok()
    }
}

impl PlaybackBackend for CpalBackend {
    type Session = CpalSession;

    fn open_session(
        &mut self,
        disc: &BurnedDisc,
        track_index: usize,
        position_ms: u64,
    ) -> PlaybackResult<Self::Session> {
        let probe_payload =
            disc.track_payload(track_index)?
                .ok_or(PlaybackError::BackendInvariant(
                    "selected VDISC track has no payload",
                ))?;
        let info = probe_audio_info(probe_payload)?;

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(PlaybackError::NoOutputDevice)?;

        let supported = device
            .supported_output_configs()
            .map_err(|error| PlaybackError::Device(error.to_string()))?
            .filter(|range| range.contains_rate(info.sample_rate_hz))
            .max_by(|left, right| left.cmp_default_heuristics(right))
            .ok_or(PlaybackError::UnsupportedOutput {
                sample_rate_hz: info.sample_rate_hz,
            })?
            .with_sample_rate(info.sample_rate_hz);

        let output_channels = usize::from(supported.channels());
        if output_channels == 0 {
            return Err(PlaybackError::UnsupportedOutput {
                sample_rate_hz: info.sample_rate_hz,
            });
        }

        let samples_per_ms =
            u64::from(info.sample_rate_hz).saturating_mul(output_channels as u64) / 1000;
        let requested_capacity = samples_per_ms
            .saturating_mul(PCM_BUFFER_MS)
            .max(output_channels as u64 * 1024);
        let capacity = usize::try_from(requested_capacity)
            .map_err(|_| PlaybackError::BackendInvariant("PCM queue capacity overflow"))?;

        let shared = Arc::new(SharedPcm::new(capacity));
        let (event_tx, event_rx) = mpsc::channel();

        let config = supported.config();
        let sample_format = supported.sample_format();
        let stream = build_output_stream(
            &device,
            config,
            sample_format,
            Arc::clone(&shared),
            event_tx.clone(),
            output_channels,
        )?;

        let decode_payload =
            disc.track_payload(track_index)?
                .ok_or(PlaybackError::BackendInvariant(
                    "selected VDISC track has no payload",
                ))?;
        let decode_len = decode_payload.len();
        let decode_shared = Arc::clone(&shared);

        let decoder_thread = thread::Builder::new()
            .name("vdisc-decode".to_owned())
            .spawn(move || {
                let result = decode_audio(
                    decode_payload,
                    Some(decode_len),
                    position_ms,
                    info,
                    output_channels,
                    |samples| decode_shared.push(samples),
                );

                decode_shared.finish(result.err());
            })
            .map_err(|error| PlaybackError::Decode(error.to_string()))?;

        Ok(CpalSession {
            stream,
            shared,
            events: event_rx,
            decoder_thread: Some(decoder_thread),
            base_position_ms: position_ms,
            sample_rate_hz: info.sample_rate_hz,
        })
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
    shared: Arc<SharedPcm>,
    events: Sender<PlaybackBackendEvent>,
    output_channels: usize,
) -> PlaybackResult<cpal::Stream> {
    match sample_format {
        SampleFormat::I8 => {
            build_typed_stream::<i8>(device, config, shared, events, output_channels)
        }
        SampleFormat::I16 => {
            build_typed_stream::<i16>(device, config, shared, events, output_channels)
        }
        SampleFormat::I24 => {
            build_typed_stream::<cpal::I24>(device, config, shared, events, output_channels)
        }
        SampleFormat::I32 => {
            build_typed_stream::<i32>(device, config, shared, events, output_channels)
        }
        SampleFormat::I64 => {
            build_typed_stream::<i64>(device, config, shared, events, output_channels)
        }
        SampleFormat::U8 => {
            build_typed_stream::<u8>(device, config, shared, events, output_channels)
        }
        SampleFormat::U16 => {
            build_typed_stream::<u16>(device, config, shared, events, output_channels)
        }
        SampleFormat::U24 => {
            build_typed_stream::<cpal::U24>(device, config, shared, events, output_channels)
        }
        SampleFormat::U32 => {
            build_typed_stream::<u32>(device, config, shared, events, output_channels)
        }
        SampleFormat::U64 => {
            build_typed_stream::<u64>(device, config, shared, events, output_channels)
        }
        SampleFormat::F32 => {
            build_typed_stream::<f32>(device, config, shared, events, output_channels)
        }
        SampleFormat::F64 => {
            build_typed_stream::<f64>(device, config, shared, events, output_channels)
        }
        SampleFormat::DsdU8 | SampleFormat::DsdU16 | SampleFormat::DsdU32 => Err(
            PlaybackError::Device("default output exposes DSD rather than PCM".to_owned()),
        ),
        _ => Err(PlaybackError::Device(
            "unsupported future CPAL sample format".to_owned(),
        )),
    }
}

fn build_typed_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    shared: Arc<SharedPcm>,
    events: Sender<PlaybackBackendEvent>,
    output_channels: usize,
) -> PlaybackResult<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let callback_shared = Arc::clone(&shared);
    let callback_events = events.clone();

    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                fill_output(output, &callback_shared, &callback_events, output_channels);
            },
            move |error| {
                let _ = events.send(PlaybackBackendEvent::DeviceError(error.to_string()));
            },
            None,
        )
        .map_err(|error| PlaybackError::Device(error.to_string()))
}

fn fill_output<T>(
    output: &mut [T],
    shared: &SharedPcm,
    events: &Sender<PlaybackBackendEvent>,
    output_channels: usize,
) where
    T: SizedSample + FromSample<f32>,
{
    let mut written = 0usize;
    let mut completion = None;

    match shared.state.try_lock() {
        Ok(mut state) => {
            while written < output.len() {
                let Some(sample) = state.samples.pop_front() else {
                    break;
                };
                output[written] = T::from_sample(sample);
                written += 1;
            }

            if state.samples.is_empty() && state.producer_done {
                completion = Some(state.decode_error.clone().map_or(
                    PlaybackBackendEvent::TrackFinished,
                    PlaybackBackendEvent::DecodeError,
                ));
            }

            drop(state);
            shared.space_available.notify_one();
        }
        Err(TryLockError::WouldBlock) => {}
        Err(TryLockError::Poisoned(_)) => {
            completion = Some(PlaybackBackendEvent::DecodeError(
                "PCM queue lock was poisoned".to_owned(),
            ));
        }
    }

    for sample in &mut output[written..] {
        *sample = T::from_sample(0.0);
    }

    if let Some(frames) = written.checked_div(output_channels) {
        shared
            .played_frames
            .fetch_add(frames as u64, Ordering::AcqRel);
    }

    if let Some(event) = completion
        && !shared.completion_sent.swap(true, Ordering::AcqRel)
    {
        let _ = events.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn required_audio_formats_decode_completely_without_an_audio_device() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/audio");

        for extension in ["flac", "mp3", "opus", "wav"] {
            let path = root.join(format!("valid.{extension}"));
            let file = File::open(&path).unwrap();
            let len = file.metadata().unwrap().len();
            let (_, _, _, info) = open_decoder(file, Some(len)).unwrap();

            let file = File::open(&path).unwrap();
            let len = file.metadata().unwrap().len();
            let mut emitted_samples = 0usize;
            decode_audio(file, Some(len), 0, info, 2, |samples| {
                emitted_samples += samples.len();
                Ok(())
            })
            .unwrap();

            assert!(emitted_samples > 0, "{extension}");
        }
    }

    #[test]
    fn channel_remix_has_defined_mono_stereo_behavior() {
        let mut output = Vec::new();

        remix_channels(&[0.25, -0.5], 1, 2, &mut output).unwrap();
        assert_eq!(output, vec![0.25, 0.25, -0.5, -0.5]);

        remix_channels(&[0.5, -0.5, 1.0, 0.0], 2, 1, &mut output).unwrap();
        assert_eq!(output, vec![0.0, 0.5]);
    }
}
