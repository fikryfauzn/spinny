use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    AddTrackRequest, DISC_TRACK_CAPACITY, SourceFingerprint, TrackMetadata, TrackSourceKind,
    ValidatedLocalAudio, customization::inspect_disc_image, load_draft,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightIssueCode {
    DraftInvalid,

    DiscTitleInvalid,

    TrackCountInvalid,

    DuplicateTrackId,

    SourceFingerprintMissing,

    SourceMissing,

    SourceNotRegular,

    SourceUnreadable,

    SourceChanged,

    AudioInvalid,

    MetadataInvalid,

    ArtworkInvalid,

    OutputExtensionInvalid,

    OutputParentMissing,

    OutputParentNotDirectory,

    OutputParentReadOnly,

    OutputAlreadyExists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightIssue {
    code: PreflightIssueCode,

    message: String,

    track_position: Option<usize>,

    path: Option<PathBuf>,
}

impl PreflightIssue {
    fn new(code: PreflightIssueCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            track_position: None,
            path: None,
        }
    }

    fn for_track(
        code: PreflightIssueCode,
        track_position: usize,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            track_position: Some(track_position),
            path: Some(path.into()),
        }
    }

    fn for_path(
        code: PreflightIssueCode,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            track_position: None,
            path: Some(path.into()),
        }
    }

    pub fn code(&self) -> PreflightIssueCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn track_position(&self) -> Option<usize> {
        self.track_position
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    draft_path: PathBuf,

    output_path: PathBuf,

    issues: Vec<PreflightIssue>,
}

impl PreflightReport {
    fn new(draft_path: PathBuf, output_path: PathBuf) -> Self {
        Self {
            draft_path,
            output_path,
            issues: Vec::new(),
        }
    }

    fn push(&mut self, issue: PreflightIssue) {
        self.issues.push(issue);
    }

    pub fn is_ready(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn draft_path(&self) -> &Path {
        &self.draft_path
    }

    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub fn issues(&self) -> &[PreflightIssue] {
        &self.issues
    }

    pub fn has_issue(&self, code: PreflightIssueCode) -> bool {
        self.issues.iter().any(|issue| issue.code == code)
    }
}

pub fn run_preflight(
    draft_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> PreflightReport {
    let draft_path = draft_path.as_ref().to_path_buf();

    let output_path = output_path.as_ref().to_path_buf();

    let mut report = PreflightReport::new(draft_path.clone(), output_path.clone());

    validate_output_destination(&output_path, &mut report);

    let draft = match load_draft(&draft_path) {
        Ok(draft) => draft,

        Err(error) => {
            report.push(PreflightIssue::for_path(
                PreflightIssueCode::DraftInvalid,
                &draft_path,
                format!("draft could not be loaded and validated: {error}"),
            ));

            return report;
        }
    };

    /*
     * These are deliberately checked again even though
     * DraftDisc itself normally protects the invariants.
     *
     * Preflight is the final independent gate before burn.
     */
    if draft.title().trim().is_empty() {
        report.push(PreflightIssue::new(
            PreflightIssueCode::DiscTitleInvalid,
            "disc title is empty",
        ));
    }

    let track_count = draft.track_count();

    if track_count == 0 || track_count > DISC_TRACK_CAPACITY {
        report.push(PreflightIssue::new(
            PreflightIssueCode::TrackCountInvalid,
            format!(
                "disc must contain between 1 and {DISC_TRACK_CAPACITY} tracks; found {track_count}"
            ),
        ));
    }

    let mut seen_track_ids = HashSet::new();

    for (index, track) in draft.tracks().iter().enumerate() {
        let position = index + 1;

        if !seen_track_ids.insert(track.id()) {
            report.push(PreflightIssue::for_track(
                PreflightIssueCode::DuplicateTrackId,
                position,
                track.source_path(),
                format!("track {position} has a duplicate track ID: {}", track.id()),
            ));
        }

        validate_track_metadata(position, track, &mut report);

        validate_track_source(&draft_path, position, track, &mut report);
    }

    if let Some(stored_image) = draft.appearance().image() {
        match inspect_disc_image(stored_image.source_path()) {
            Ok(actual_image) => {
                let matches = actual_image.source_fingerprint()
                    == stored_image.source_fingerprint()
                    && actual_image.width() == stored_image.width()
                    && actual_image.height() == stored_image.height()
                    && actual_image.format() == stored_image.format();

                if !matches {
                    report.push(PreflightIssue::for_path(
                        PreflightIssueCode::ArtworkInvalid,
                        stored_image.source_path(),
                        "disc artwork changed after customization",
                    ));
                }
            }

            Err(error) => {
                report.push(PreflightIssue::for_path(
                    PreflightIssueCode::ArtworkInvalid,
                    stored_image.source_path(),
                    format!("disc artwork is unavailable or invalid: {error}"),
                ));
            }
        }
    }

    report
}

fn validate_track_metadata(
    position: usize,
    track: &crate::DraftTrack,
    report: &mut PreflightReport,
) {
    let fields = [
        ("title", track.title()),
        ("artist", track.artist()),
        ("album", track.album()),
        ("genre", track.genre()),
    ];

    for (field_name, value) in fields {
        if let Some(value) = value
            && value.trim().is_empty()
        {
            report.push(PreflightIssue::for_track(
                PreflightIssueCode::MetadataInvalid,
                position,
                track.source_path(),
                format!("track {position} contains an empty {field_name} metadata value"),
            ));
        }
    }
}

fn validate_track_source(
    draft_path: &Path,
    position: usize,
    track: &crate::DraftTrack,
    report: &mut PreflightReport,
) {
    let source_path = track.source_path();

    let filesystem_metadata = match fs::metadata(source_path) {
        Ok(metadata) => metadata,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.push(PreflightIssue::for_track(
                PreflightIssueCode::SourceMissing,
                position,
                source_path,
                format!("track {position} source file is missing"),
            ));

            return;
        }

        Err(error) => {
            report.push(PreflightIssue::for_track(
                PreflightIssueCode::SourceUnreadable,
                position,
                source_path,
                format!("track {position} source file could not be inspected: {error}"),
            ));

            return;
        }
    };

    if !filesystem_metadata.is_file() {
        report.push(PreflightIssue::for_track(
            PreflightIssueCode::SourceNotRegular,
            position,
            source_path,
            format!("track {position} source is not a regular file"),
        ));

        return;
    }

    match track.source_fingerprint() {
        Some(expected) => {
            if filesystem_metadata.len() != expected.size_bytes() {
                report.push(PreflightIssue::for_track(
                    PreflightIssueCode::SourceChanged,
                    position,
                    source_path,
                    format!("track {position} source size changed after import"),
                ));
            } else {
                match SourceFingerprint::from_file(source_path) {
                    Ok(actual) => {
                        if &actual != expected {
                            report.push(PreflightIssue::for_track(
                                PreflightIssueCode::SourceChanged,
                                position,
                                source_path,
                                format!("track {position} source bytes changed after import"),
                            ));
                        }
                    }

                    Err(error) => {
                        report.push(PreflightIssue::for_track(
                            PreflightIssueCode::SourceUnreadable,
                            position,
                            source_path,
                            format!("track {position} source could not be fingerprinted: {error}"),
                        ));
                    }
                }
            }
        }

        None => {
            report.push(PreflightIssue::for_track(
                PreflightIssueCode::SourceFingerprintMissing,
                position,
                source_path,
                format!("track {position} has no stored source fingerprint"),
            ));
        }
    }

    /*
     * Re-run actual media validation.
     *
     * Fingerprinting proves identity.
     * Decoding proves the burn input is still a usable
     * audio file through the supported VDISC pipeline.
     */
    let audio = AddTrackRequest::begin(draft_path)
        .and_then(|request| {
            request
                .select_source(TrackSourceKind::Local)
                .select_local_file(source_path)
        })
        .and_then(ValidatedLocalAudio::validate);

    match audio {
        Ok(audio) => {
            if let Err(error) = TrackMetadata::extract(&audio) {
                report.push(PreflightIssue::for_track(
                    PreflightIssueCode::MetadataInvalid,
                    position,
                    source_path,
                    format!("track {position} metadata could not be read: {error}"),
                ));
            }
        }

        Err(error) => {
            report.push(PreflightIssue::for_track(
                PreflightIssueCode::AudioInvalid,
                position,
                source_path,
                format!("track {position} audio validation failed: {error}"),
            ));
        }
    }
}

fn validate_output_destination(output_path: &Path, report: &mut PreflightReport) {
    let valid_extension = output_path
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("vdisc");

    if !valid_extension {
        report.push(PreflightIssue::for_path(
            PreflightIssueCode::OutputExtensionInvalid,
            output_path,
            "burn output must use the .vdisc extension",
        ));
    }

    if output_path.exists() {
        report.push(PreflightIssue::for_path(
            PreflightIssueCode::OutputAlreadyExists,
            output_path,
            "burn output already exists; V0.1 does not define overwrite behavior",
        ));
    }

    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let parent_metadata = match fs::metadata(parent) {
        Ok(metadata) => metadata,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.push(PreflightIssue::for_path(
                PreflightIssueCode::OutputParentMissing,
                parent,
                "burn output parent directory does not exist",
            ));

            return;
        }

        Err(error) => {
            report.push(PreflightIssue::for_path(
                PreflightIssueCode::OutputParentMissing,
                parent,
                format!("burn output parent could not be inspected: {error}"),
            ));

            return;
        }
    };

    if !parent_metadata.is_dir() {
        report.push(PreflightIssue::for_path(
            PreflightIssueCode::OutputParentNotDirectory,
            parent,
            "burn output parent is not a directory",
        ));

        return;
    }

    if parent_metadata.permissions().readonly() {
        report.push(PreflightIssue::for_path(
            PreflightIssueCode::OutputParentReadOnly,
            parent,
            "burn output parent is marked read-only",
        ));
    }
}
