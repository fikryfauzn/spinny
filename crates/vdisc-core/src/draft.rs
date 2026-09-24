use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{DISC_TRACK_CAPACITY, DiscAppearance, DiscSurface, DraftTrack, Result, VdiscError};

pub const DRAFT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftDisc {
    draft_version: u32,

    id: Uuid,

    title: String,

    created_at_unix: u64,

    #[serde(default)]
    tracks: Vec<DraftTrack>,

    #[serde(default)]
    appearance: DiscAppearance,
}

impl DraftDisc {
    pub fn new(title: impl Into<String>) -> Result<Self> {
        let title = title.into();

        validate_title(&title)?;

        let created_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                VdiscError::InvalidInput(format!("system clock is before Unix epoch: {error}"))
            })?
            .as_secs();

        Ok(Self {
            draft_version: DRAFT_FORMAT_VERSION,

            id: Uuid::new_v4(),

            title,

            created_at_unix,

            tracks: Vec::new(),

            appearance: DiscAppearance::default(),
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn created_at_unix(&self) -> u64 {
        self.created_at_unix
    }

    pub fn capacity(&self) -> usize {
        DISC_TRACK_CAPACITY
    }

    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    pub fn tracks(&self) -> &[DraftTrack] {
        &self.tracks
    }

    pub fn appearance(&self) -> &DiscAppearance {
        &self.appearance
    }

    pub fn is_full(&self) -> bool {
        self.track_count() >= DISC_TRACK_CAPACITY
    }

    pub(crate) fn add_track(&mut self, track: DraftTrack) -> Result<()> {
        if self.is_full() {
            return Err(VdiscError::DiscFull {
                capacity: DISC_TRACK_CAPACITY,
            });
        }

        self.tracks.push(track);

        Ok(())
    }

    pub(crate) fn remove_track_at(&mut self, position: usize) -> Result<DraftTrack> {
        let index = self.position_to_index(position)?;

        Ok(self.tracks.remove(index))
    }

    pub(crate) fn move_track(&mut self, from_position: usize, to_position: usize) -> Result<bool> {
        let from_index = self.position_to_index(from_position)?;

        let to_index = self.position_to_index(to_position)?;

        if from_index == to_index {
            return Ok(false);
        }

        let track = self.tracks.remove(from_index);

        self.tracks.insert(to_index, track);

        Ok(true)
    }

    pub(crate) fn set_disc_surface(&mut self, surface: DiscSurface) {
        self.appearance.set_surface(surface);
    }

    pub(crate) fn set_disc_label(&mut self, label: Option<String>) {
        self.appearance.set_label(label);
    }

    pub fn validate(&self) -> Result<()> {
        if self.draft_version != DRAFT_FORMAT_VERSION {
            return Err(VdiscError::InvalidInput(format!(
                "unsupported draft format version: {}",
                self.draft_version
            )));
        }

        validate_title(&self.title)?;

        if self.tracks.len() > DISC_TRACK_CAPACITY {
            return Err(VdiscError::InvalidInput(format!(
                "draft contains {} tracks but capacity is {}",
                self.tracks.len(),
                DISC_TRACK_CAPACITY
            )));
        }

        self.appearance.validate()?;

        Ok(())
    }

    fn position_to_index(&self, position: usize) -> Result<usize> {
        if position == 0 || position > self.tracks.len() {
            return Err(VdiscError::TrackPositionOutOfBounds {
                position,
                track_count: self.tracks.len(),
            });
        }

        Ok(position - 1)
    }
}

fn validate_title(title: &str) -> Result<()> {
    if title.trim().is_empty() {
        return Err(VdiscError::InvalidInput(
            "CD title cannot be empty".to_string(),
        ));
    }

    Ok(())
}
