use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{TrackMetadata, ValidatedLocalAudio};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftTrack {
    id: Uuid,

    source_path: PathBuf,

    container: String,
    codec: String,

    sample_rate: Option<u32>,
    channels: Option<u16>,
    duration_ms: Option<u64>,

    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,

    source_track_number: Option<u32>,
    source_track_total: Option<u32>,

    source_disc_number: Option<u32>,
    source_disc_total: Option<u32>,
}

impl DraftTrack {
    pub(crate) fn from_local_audio(audio: &ValidatedLocalAudio, metadata: &TrackMetadata) -> Self {
        Self {
            id: Uuid::new_v4(),

            source_path: audio.source_path().to_path_buf(),

            container: audio.container().to_string(),
            codec: audio.codec().to_string(),

            sample_rate: audio.sample_rate(),
            channels: audio.channels(),
            duration_ms: audio.duration_ms(),

            title: metadata.title().map(str::to_string),
            artist: metadata.artist().map(str::to_string),
            album: metadata.album().map(str::to_string),
            genre: metadata.genre().map(str::to_string),

            source_track_number: metadata.track_number(),
            source_track_total: metadata.track_total(),

            source_disc_number: metadata.disc_number(),
            source_disc_total: metadata.disc_total(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
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

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }

    pub fn album(&self) -> Option<&str> {
        self.album.as_deref()
    }

    pub fn genre(&self) -> Option<&str> {
        self.genre.as_deref()
    }

    pub fn source_track_number(&self) -> Option<u32> {
        self.source_track_number
    }

    pub fn source_track_total(&self) -> Option<u32> {
        self.source_track_total
    }

    pub fn source_disc_number(&self) -> Option<u32> {
        self.source_disc_number
    }

    pub fn source_disc_total(&self) -> Option<u32> {
        self.source_disc_total
    }
}
