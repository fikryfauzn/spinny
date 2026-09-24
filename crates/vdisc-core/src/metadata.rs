use std::{borrow::Cow, path::Path};

use lofty::{file::TaggedFileExt, probe::Probe, tag::Accessor};

use crate::{Result, ValidatedLocalAudio, VdiscError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,

    track_number: Option<u32>,
    track_total: Option<u32>,

    disc_number: Option<u32>,
    disc_total: Option<u32>,
}

impl TrackMetadata {
    pub fn extract(audio: &ValidatedLocalAudio) -> Result<Self> {
        let path = audio.source_path();

        let probe = Probe::open(path).map_err(|error| {
            metadata_error(path, format!("failed opening metadata source: {error}"))
        })?;

        /*
         * Do not rely only on extension.
         *
         * Objective 4 explicitly allows an extensionless
         * local file.
         */
        let probe = probe.guess_file_type().map_err(|error| {
            metadata_error(path, format!("failed determining metadata format: {error}"))
        })?;

        let tagged_file = probe
            .read()
            .map_err(|error| metadata_error(path, format!("failed reading metadata: {error}")))?;

        /*
         * Prefer the format's primary tag.
         *
         * If it does not exist, accept the first tag that
         * Lofty found.
         */
        let Some(tag) = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag())
        else {
            return Ok(Self::default());
        };

        Ok(Self {
            title: clean_text(tag.title()),
            artist: clean_text(tag.artist()),
            album: clean_text(tag.album()),
            genre: clean_text(tag.genre()),

            track_number: tag.track(),
            track_total: tag.track_total(),

            /*
             * Lofty's accessor terminology is "disk".
             * Our VDISC domain uses "disc".
             */
            disc_number: tag.disk(),
            disc_total: tag.disk_total(),
        })
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

    pub fn track_number(&self) -> Option<u32> {
        self.track_number
    }

    pub fn track_total(&self) -> Option<u32> {
        self.track_total
    }

    pub fn disc_number(&self) -> Option<u32> {
        self.disc_number
    }

    pub fn disc_total(&self) -> Option<u32> {
        self.disc_total
    }
}

fn clean_text(value: Option<Cow<'_, str>>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn metadata_error(path: &Path, reason: impl Into<String>) -> VdiscError {
    VdiscError::MetadataRead {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}
