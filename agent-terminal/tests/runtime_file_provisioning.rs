#[path = "../build_support.rs"]
mod build_support;

use build_support::{
    content_fingerprint, discover_git_watch_paths, profile_output_dir, provision_if_stale,
    resolve_build_version, select_declared_build_version, validate_build_version,
    BuildVersionSource,
};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

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

#[test]
fn declared_build_versions_are_validated_in_environment_then_tag_order() {
    assert_eq!(
        select_declared_build_version(Some("1.2.3-env"), Some("1.2.3-tag"))
            .unwrap()
            .unwrap(),
        ("1.2.3-env".to_owned(), BuildVersionSource::Environment)
    );
    assert_eq!(
        select_declared_build_version(None, Some("  1.2.3-tag\r\n"))
            .unwrap()
            .unwrap(),
        ("1.2.3-tag".to_owned(), BuildVersionSource::Tag)
    );
    assert!(select_declared_build_version(None, None).unwrap().is_none());
    assert!(select_declared_build_version(Some("bad version"), Some("valid-tag")).is_err());
    assert!(validate_build_version("UNKNOWN", "fixture").is_err());
    assert!(validate_build_version("unknown", "fixture").is_err());
    assert!(validate_build_version("", "fixture").is_err());
    assert!(validate_build_version("bad\"version", "fixture").is_err());
    assert!(validate_build_version("bad/version", "fixture").is_err());
}

#[test]
fn git_watch_paths_cover_normal_linked_and_detached_worktrees() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repository");
    let linked = temp.path().join("linked");
    fs::create_dir(&repository).unwrap();

    run_git(&repository, &["init", "-b", "main"]);
    run_git(&repository, &["config", "user.name", "Lucidity Test"]);
    run_git(
        &repository,
        &["config", "user.email", "lucidity-test@example.invalid"],
    );
    run_git(&repository, &["commit", "--allow-empty", "-m", "fixture"]);
    run_git(&repository, &["pack-refs", "--all"]);

    let normal = discover_git_watch_paths(&repository).unwrap().unwrap();
    assert!(normal.head.is_file());
    assert!(normal.head.ends_with(Path::new(".git").join("HEAD")));
    assert!(normal
        .symbolic_ref
        .as_ref()
        .unwrap()
        .ends_with(Path::new(".git").join("refs/heads/main")));
    assert!(normal
        .packed_refs
        .ends_with(Path::new(".git").join("packed-refs")));
    let resolution = resolve_build_version(&repository).unwrap();
    assert!(!resolution.version.is_empty());
    assert_eq!(resolution.git_watch_paths.as_ref().unwrap(), &normal);

    run_git(
        &repository,
        &["worktree", "add", "-b", "linked", linked.to_str().unwrap()],
    );
    let linked_paths = discover_git_watch_paths(&linked).unwrap().unwrap();
    assert!(linked_paths.head.is_file());
    assert_ne!(linked_paths.head, normal.head);
    assert!(linked_paths
        .head
        .to_string_lossy()
        .replace('\\', "/")
        .contains("/.git/worktrees/linked/HEAD"));
    assert!(linked_paths
        .symbolic_ref
        .as_ref()
        .unwrap()
        .ends_with(Path::new(".git").join("refs/heads/linked")));
    assert_eq!(linked_paths.packed_refs, normal.packed_refs);

    run_git(&linked, &["checkout", "--detach"]);
    let detached = discover_git_watch_paths(&linked).unwrap().unwrap();
    assert_eq!(detached.head, linked_paths.head);
    assert_eq!(detached.packed_refs, normal.packed_refs);
    assert!(detached.symbolic_ref.is_none());
}

fn run_git(current_dir: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .current_dir(current_dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed:\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
