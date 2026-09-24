mod common;

use std::{error::Error, fs, path::Path};

use lofty::{
    config::WriteOptions,
    tag::{Accessor, Tag, TagExt, TagType},
};

use vdisc_core::{
    AddTrackRequest, DraftDisc, TrackMetadata, TrackSourceKind, ValidatedLocalAudio, save_draft,
};

fn write_test_wav(path: &Path) -> Result<(), Box<dyn Error>> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;

    for _ in 0..(44_100 / 4) {
        writer.write_sample::<i16>(0)?;
        writer.write_sample::<i16>(0)?;
    }

    writer.finalize()?;

    Ok(())
}

fn write_test_metadata(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut tag = Tag::new(TagType::Id3v2);

    tag.set_title("夜のドライブ".to_string());

    tag.set_artist("Akito Test Artist".to_string());

    tag.set_album("VDISC Test Album".to_string());

    tag.set_genre("Electronic".to_string());

    tag.set_track(3);
    tag.set_track_total(10);

    tag.set_disk(2);
    tag.set_disk_total(3);

    tag.save_to_path(path, WriteOptions::default())?;

    Ok(())
}

fn validated_audio(
    draft_path: &Path,
    audio_path: &Path,
) -> Result<ValidatedLocalAudio, Box<dyn Error>> {
    let selection = AddTrackRequest::begin(draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(audio_path)?;

    Ok(ValidatedLocalAudio::validate(selection)?)
}

#[test]
fn tagged_wav_metadata_is_extracted() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("tagged.wav");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    write_test_wav(&audio_path)?;
    write_test_metadata(&audio_path)?;

    let audio = validated_audio(&draft_path, &audio_path)?;

    let metadata = TrackMetadata::extract(&audio)?;

    assert_eq!(metadata.title(), Some("夜のドライブ"));

    assert_eq!(metadata.artist(), Some("Akito Test Artist"));

    assert_eq!(metadata.album(), Some("VDISC Test Album"));

    assert_eq!(metadata.genre(), Some("Electronic"));

    assert_eq!(metadata.track_number(), Some(3));

    assert_eq!(metadata.track_total(), Some(10));

    assert_eq!(metadata.disc_number(), Some(2));

    assert_eq!(metadata.disc_total(), Some(3));

    Ok(())
}

#[test]
fn untagged_audio_is_valid_metadata_state() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("untagged.wav");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    write_test_wav(&audio_path)?;

    let audio = validated_audio(&draft_path, &audio_path)?;

    let metadata = TrackMetadata::extract(&audio)?;

    assert_eq!(metadata, TrackMetadata::default());

    Ok(())
}

#[test]
fn metadata_read_does_not_modify_source() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("tagged.wav");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    write_test_wav(&audio_path)?;
    write_test_metadata(&audio_path)?;

    let audio = validated_audio(&draft_path, &audio_path)?;

    let before = fs::read(&audio_path)?;

    let _metadata = TrackMetadata::extract(&audio)?;

    let after = fs::read(&audio_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn metadata_read_does_not_modify_draft() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("tagged.wav");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    write_test_wav(&audio_path)?;
    write_test_metadata(&audio_path)?;

    let audio = validated_audio(&draft_path, &audio_path)?;

    let before = fs::read(&draft_path)?;

    let _metadata = TrackMetadata::extract(&audio)?;

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn validated_audio_preserves_target_identity() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let audio_path = sandbox.path().join("song.wav");

    let draft = DraftDisc::new("Disc")?;

    let expected_id = draft.id();

    save_draft(&draft_path, &draft)?;

    write_test_wav(&audio_path)?;

    let audio = validated_audio(&draft_path, &audio_path)?;

    assert_eq!(audio.target_disc_id(), expected_id);

    assert_eq!(audio.target_path(), draft_path);

    Ok(())
}
