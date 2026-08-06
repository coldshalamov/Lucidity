#[path = "../build_support.rs"]
mod build_support;

use build_support::{content_fingerprint, profile_output_dir, provision_if_stale};
use std::fs;

#[test]
fn missing_current_and_stale_destinations_are_distinguished_by_content_hash() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.dll");
    let destination = temp.path().join("nested").join("destination.dll");
    fs::write(&source, b"first-runtime-payload").unwrap();

    assert!(provision_if_stale(&source, &destination).unwrap().copied);
    assert_eq!(
        content_fingerprint(&source).unwrap(),
        content_fingerprint(&destination).unwrap()
    );
    assert!(!provision_if_stale(&source, &destination).unwrap().copied);

    fs::write(&destination, b"stale-runtime-payload").unwrap();
    assert!(provision_if_stale(&source, &destination).unwrap().copied);
    assert_eq!(fs::read(&destination).unwrap(), b"first-runtime-payload");
}

#[test]
fn cargo_out_dir_maps_to_the_profile_output_directory() {
    let out = std::path::Path::new("target")
        .join("release")
        .join("build")
        .join("agent-terminal-hash")
        .join("out");

    assert_eq!(
        profile_output_dir(&out).unwrap(),
        std::path::Path::new("target").join("release")
    );
}

#[test]
fn fingerprints_are_stable_sha256() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = temp.path().join("fixture");
    fs::write(&fixture, b"abc").unwrap();
    let fingerprint = content_fingerprint(&fixture).unwrap();
    let hex = fingerprint
        .sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    assert_eq!(fingerprint.byte_len, 3);
    assert_eq!(
        hex,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
