mod common;

use std::{error::Error, fs, path::Path};

use vdisc_core::{
    AddTrackRequest, DraftDisc, TrackSourceKind, ValidatedLocalAudio, VdiscError, save_draft,
};

fn write_test_wav(path: &Path) -> Result<(), Box<dyn Error>> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;

    // 0.25 seconds of stereo silence.
    for _ in 0..(44_100 / 4) {
        writer.write_sample::<i16>(0)?;
        writer.write_sample::<i16>(0)?;
    }

    writer.finalize()?;

    Ok(())
}

fn local_selection(
    draft_path: &Path,
    audio_path: &Path,
) -> Result<vdisc_core::LocalFileSelection, Box<dyn Error>> {
    Ok(AddTrackRequest::begin(draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(audio_path)?)
}

#[test]
fn valid_wav_is_accepted() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("valid.wav");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;
    write_test_wav(&audio_path)?;

    let selection = local_selection(&draft_path, &audio_path)?;

    let audio = ValidatedLocalAudio::validate(selection)?;

    assert_eq!(audio.container(), "wav");
    assert_eq!(audio.codec(), "pcm");
    assert_eq!(audio.sample_rate(), Some(44_100));
    assert_eq!(audio.channels(), Some(2));

    let duration = audio.duration_ms().unwrap();

    assert!(
        (240..=260).contains(&duration),
        "unexpected duration: {duration} ms"
    );

    Ok(())
}

#[test]
fn fake_wav_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("fake.wav");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    fs::write(&audio_path, b"this is not audio")?;

    let selection = local_selection(&draft_path, &audio_path)?;

    let result = ValidatedLocalAudio::validate(selection);

    assert!(matches!(result, Err(VdiscError::AudioValidation { .. })));

    Ok(())
}

#[test]
fn fake_flac_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("fake.flac");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    fs::write(&audio_path, b"definitely not flac")?;

    let selection = local_selection(&draft_path, &audio_path)?;

    let result = ValidatedLocalAudio::validate(selection);

    assert!(matches!(result, Err(VdiscError::AudioValidation { .. })));

    Ok(())
}

#[test]
fn validation_does_not_modify_source() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("valid.wav");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;
    write_test_wav(&audio_path)?;

    let before = fs::read(&audio_path)?;

    let selection = local_selection(&draft_path, &audio_path)?;

    let _audio = ValidatedLocalAudio::validate(selection)?;

    let after = fs::read(&audio_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn validation_does_not_modify_draft() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("valid.wav");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;
    write_test_wav(&audio_path)?;

    let before = fs::read(&draft_path)?;

    let selection = local_selection(&draft_path, &audio_path)?;

    let _audio = ValidatedLocalAudio::validate(selection)?;

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}
