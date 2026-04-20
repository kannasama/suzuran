use suzuran_server::cue::{parse_cue, CueSheet};

const SAMPLE_CUE: &str = r#"
REM GENRE Rock
REM DATE 1979
PERFORMER "Pink Floyd"
TITLE "The Wall (Disc 2)"
FILE "disc2.flac" WAVE

  TRACK 01 AUDIO
    TITLE "Hey You"
    PERFORMER "Pink Floyd"
    INDEX 01 00:00:00

  TRACK 02 AUDIO
    TITLE "Is There Anybody Out There?"
    PERFORMER "Pink Floyd"
    INDEX 01 04:42:00

  TRACK 03 AUDIO
    TITLE "Nobody Home"
    INDEX 01 07:19:00
"#;

#[test]
fn test_parse_cue_sheet() {
    let sheet = parse_cue(SAMPLE_CUE).unwrap();
    assert_eq!(sheet.album_title.as_deref(), Some("The Wall (Disc 2)"));
    assert_eq!(sheet.performer.as_deref(), Some("Pink Floyd"));
    assert_eq!(sheet.date.as_deref(), Some("1979"));
    assert_eq!(sheet.audio_file, "disc2.flac");
    assert_eq!(sheet.tracks.len(), 3);

    assert_eq!(sheet.tracks[0].number, 1);
    assert_eq!(sheet.tracks[0].title.as_deref(), Some("Hey You"));
    assert!((sheet.tracks[0].index_01_secs - 0.0).abs() < 0.01);

    // INDEX 01 04:42:00 → 4*60 + 42 + 0/75 = 282.0 seconds
    assert!((sheet.tracks[1].index_01_secs - 282.0).abs() < 0.01);

    // INDEX 01 07:19:00 → 7*60 + 19 + 0/75 = 439.0 seconds
    assert!((sheet.tracks[2].index_01_secs - 439.0).abs() < 0.01);
}

#[test]
fn test_track_duration_calc() {
    let sheet = parse_cue(SAMPLE_CUE).unwrap();
    // track 1 ends at track 2 start (282.0), track 3 has no end (None = EOF)
    let end_0 = sheet.tracks.get(1).map(|t| t.index_01_secs);
    assert_eq!(end_0, Some(282.0));
    assert!(sheet.tracks.get(3).is_none());
}
