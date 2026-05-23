use std::path::Path;

#[test]
fn test_read_generated_mkv() {
    let path = Path::new("/tmp/test_movie.mkv");
    assert!(path.exists(), "Run mkvgen.py first to create test file");

    let mkv = matroska::open(path).expect("Should open generated MKV");
    assert_eq!(mkv.info.title.as_deref(), Some("The Dark Knight, The"));

    let video_tracks: Vec<_> = mkv.video_tracks().collect();
    assert!(
        !video_tracks.is_empty(),
        "Should have at least one video track"
    );
    if let matroska::Settings::Video(v) = &video_tracks[0].settings {
        assert_eq!(v.pixel_width, 1920);
        assert_eq!(v.pixel_height, 1080);
    } else {
        panic!("First track should be video");
    }
}
