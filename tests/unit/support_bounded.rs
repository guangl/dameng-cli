use dameng_cli::support::bounded;
use std::io::{self, Read};

#[test]
fn bounded_reads_reject_excess_without_consuming_the_whole_stream() {
    let mut reader = io::Cursor::new(vec![b'x'; 1000]);
    assert!(bounded::read(&mut reader, 17).is_err());
    assert_eq!(reader.position(), 18);
    let mut next = [0; 1];
    reader.read_exact(&mut next).unwrap();
    assert_eq!(next, [b'x']);
    assert_eq!(bounded::read(&b"exact"[..], 5).unwrap(), b"exact");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("input");
    std::fs::write(&path, [0xff]).unwrap();
    assert!(bounded::text(&path, 5).is_err());
    assert!(bounded::file(&temp.path().join("absent"), 5).is_err());
}
