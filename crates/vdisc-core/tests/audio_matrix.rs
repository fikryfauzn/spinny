mod common;

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use vdisc_core::{
    AddTrackRequest, DraftDisc, TrackMetadata, TrackSourceKind, ValidatedLocalAudio, save_draft,
};

struct AudioCase {
    file_name: &'static str,
    expected_codec: &'static str,
    expected_containers: &'static [&'static str],
    expected_sample_rate: u32,
    expected_channels: u16,
}

const CASES: &[AudioCase] = &[
    AudioCase {
        file_name: "valid.wav",
        expected_codec: "pcm",
        expected_containers: &["wav"],
        expected_sample_rate: 48_000,
        expected_channels: 2,
    },
    AudioCase {
        file_name: "valid.flac",
        expected_codec: "flac",
        expected_containers: &["flac"],
        expected_sample_rate: 48_000,
        expected_channels: 2,
    },
    AudioCase {
        file_name: "valid.mp3",
        expected_codec: "mp3",
        expected_containers: &["mpa", "mp3"],
        expected_sample_rate: 48_000,
        expected_channels: 2,
    },
    AudioCase {
        file_name: "valid.opus",
        expected_codec: "opus",
        expected_containers: &["ogg"],
        expected_sample_rate: 48_000,
        expected_channels: 2,
    },
];

fn fixture_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/audio")
        .join(file_name)
}

fn validate_fixture(file_name: &str) -> Result<ValidatedLocalAudio, Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("matrix.vdraft");

    let draft = DraftDisc::new("Compatibility Matrix")?;

    save_draft(&draft_path, &draft)?;

    let audio_path = fixture_path(file_name);

    let selection = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&audio_path)?;

    Ok(ValidatedLocalAudio::validate(selection)?)
}

#[test]
fn all_required_audio_formats_validate() -> Result<(), Box<dyn Error>> {
    for case in CASES {
        let audio = validate_fixture(case.file_name)?;

        assert_eq!(
            audio.codec(),
            case.expected_codec,
            "wrong codec for {}",
            case.file_name
        );

        assert!(
            case.expected_containers.contains(&audio.container()),
            "unexpected container for {}: {}",
            case.file_name,
            audio.container()
        );

        assert_eq!(
            audio.sample_rate(),
            Some(case.expected_sample_rate),
            "wrong sample rate for {}",
            case.file_name
        );

        assert_eq!(
            audio.channels(),
            Some(case.expected_channels),
            "wrong channel count for {}",
            case.file_name
        );

        let duration = audio
            .duration_ms()
            .expect("matrix fixture should report duration");

        assert!(
            (900..=1_150).contains(&duration),
            "unexpected duration for {}: {duration} ms",
            case.file_name
        );
    }

    Ok(())
}

#[test]
fn metadata_reader_accepts_all_required_formats() -> Result<(), Box<dyn Error>> {
    for case in CASES {
        let audio = validate_fixture(case.file_name)?;

        TrackMetadata::extract(&audio).map_err(|error| {
            format!("metadata extraction failed for {}: {error}", case.file_name)
        })?;
    }

    Ok(())
}

#[test]
fn matrix_validation_never_modifies_fixture_files() -> Result<(), Box<dyn Error>> {
    for case in CASES {
        let path = fixture_path(case.file_name);

        let before = fs::read(&path)?;

        let _audio = validate_fixture(case.file_name)?;

        let after = fs::read(&path)?;

        assert_eq!(
            before, after,
            "validation modified fixture {}",
            case.file_name
        );
    }

    Ok(())
}

#[test]
fn all_fixture_files_exist_and_are_nonempty() -> Result<(), Box<dyn Error>> {
    for case in CASES {
        let path = fixture_path(case.file_name);

        assert!(path.is_file(), "fixture missing: {}", path.display());

        let metadata = fs::metadata(&path)?;

        assert!(metadata.len() > 0, "fixture is empty: {}", path.display());
    }

    Ok(())
}
