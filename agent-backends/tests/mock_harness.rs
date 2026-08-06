use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug)]
struct MockRun {
    stdout: String,
    stderr: String,
    store_hash: String,
}

#[test]
fn mock_transcript_is_deterministic_after_pid_normalization_and_store_hashing() {
    let commands = concat!(
        "/work hello\n",
        "/usage\n",
        "/new\n",
        "/work after-new\n",
        "/ask continue?\n",
        "/approve\n",
        "/complete\n",
        "/quit\n",
    );
    let first_temp = tempfile::tempdir().unwrap();
    let second_temp = tempfile::tempdir().unwrap();
    let first = run_mock(first_temp.path().join("store"), commands, 7, None, true);
    let second = run_mock(second_temp.path().join("store"), commands, 7, None, true);

    assert_eq!(
        normalize_pids(&first.stdout),
        normalize_pids(&second.stdout)
    );
    assert_eq!(first.store_hash, second.store_hash);
    assert_eq!(first.stderr, "");
    assert_eq!(second.stderr, "");
    assert!(first.stdout.contains("session=mock-7-0"));
    assert!(first.stdout.contains("session=mock-7-1"));
    assert!(normalize_pids(&first.stdout).contains("MOCK CHILD CLEANUP pid=<pid> status=0"));
}

#[test]
fn new_command_rebinds_identity_without_restarting_parent_process() {
    let temp = tempfile::tempdir().unwrap();
    let run = run_mock(
        temp.path().join("store"),
        concat!("/new\n", "/new\n", "/quit\n"),
        11,
        None,
        false,
    );

    let parent_pids = parent_pid_markers(&run.stdout);
    assert!(
        parent_pids.len() >= 3,
        "expected ready plus two /new parent PID markers in:\n{}",
        run.stdout
    );
    assert!(
        parent_pids.iter().all(|pid| pid == &parent_pids[0]),
        "parent PID changed across /new markers: {parent_pids:?}"
    );
    assert!(run.stdout.contains("session=mock-11-0"));
    assert!(run.stdout.contains("session=mock-11-1"));
    assert!(run.stdout.contains("session=mock-11-2"));
}

#[test]
fn resume_restores_native_identity_and_monotonic_sequence_from_store() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("store");
    let first = run_mock(
        store.clone(),
        concat!("/complete\n", "/quit\n"),
        9,
        None,
        false,
    );
    assert!(first.stdout.contains("session=mock-9-0"));

    let resumed = run_mock(
        store,
        concat!("/usage\n", "/quit\n"),
        9,
        Some("mock-9-0"),
        false,
    );

    assert!(
        resumed.stdout.contains("MOCK READY")
            && resumed.stdout.contains("session=mock-9-0")
            && resumed.stdout.contains("seq=4"),
        "resume did not restore identity and sequence:\n{}",
        resumed.stdout
    );
}

fn run_mock(
    store: PathBuf,
    commands: &str,
    seed: u64,
    resume: Option<&str>,
    with_child: bool,
) -> MockRun {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lucidity-mock-agent"));
    command
        .arg("--adapter")
        .arg("mock-agent")
        .arg("--profile")
        .arg("11111111-1111-4111-8111-111111111111")
        .arg("--store")
        .arg(&store)
        .arg("--seed")
        .arg(seed.to_string())
        .arg("--pane")
        .arg("7")
        .arg("--cwd")
        .arg(r"C:\work\mock")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(native_id) = resume {
        command.arg("--resume").arg(native_id);
    }
    if with_child {
        command.arg("--with-child");
    }

    let mut child = command.spawn().expect("mock agent spawns");
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(commands.as_bytes())
        .expect("commands write");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("mock exits");
    assert!(
        output.status.success(),
        "mock failed: status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    MockRun {
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
        store_hash: hash_store(&store),
    }
}

fn normalize_pids(input: &str) -> String {
    normalize_after(&normalize_after(input, "pid="), "child=")
}

fn normalize_after(input: &str, prefix: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(index) = rest.find(prefix) {
        output.push_str(&rest[..index]);
        output.push_str(prefix);
        output.push_str("<pid>");
        let after_prefix = &rest[index + prefix.len()..];
        let digits = after_prefix
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .map(char::len_utf8)
            .sum();
        rest = &after_prefix[digits..];
    }
    output.push_str(rest);
    output
}

fn parent_pid_markers(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| line.contains("MOCK READY") || line.contains("MOCK OK /new"))
        .filter_map(|line| value_after(line, "pid="))
        .collect()
}

fn value_after(line: &str, prefix: &str) -> Option<String> {
    let value = line.split_once(prefix)?.1;
    Some(
        value
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect(),
    )
}

fn hash_store(store: &Path) -> String {
    let mut files = fs::read_dir(store)
        .unwrap_or_else(|error| panic!("cannot read store {}: {error}", store.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    files.sort();

    let mut hasher = Sha256::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy();
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(fs::read(&path).unwrap());
        hasher.update([0]);
    }
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
