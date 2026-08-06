#![cfg(windows)]

use filedescriptor::OwnedHandle;
use portable_pty::cmdbuilder::WindowsProcessTreePolicy;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use winapi::shared::minwindef::FALSE;
use winapi::shared::winerror::WAIT_TIMEOUT;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::{WAIT_FAILED, WAIT_OBJECT_0};
use winapi::um::winnt::SYNCHRONIZE;

const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);

struct StopOnDrop(PathBuf);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        let _ = fs::write(self.0.join("stop"), b"stop");
    }
}

fn unique_helper_dir(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!(
        "lucidity-pty-{test_name}-{}-{nonce}",
        std::process::id()
    ))
}

fn spawn_tree(
    test_name: &str,
    policy: Option<WindowsProcessTreePolicy>,
) -> (
    portable_pty::PtyPair,
    Box<dyn portable_pty::Child + Send + Sync>,
    Box<dyn Write + Send>,
    PathBuf,
    StopOnDrop,
) {
    let dir = unique_helper_dir(test_name);
    fs::create_dir(&dir).unwrap();
    let stop = StopOnDrop(dir.clone());

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize::default()).unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    thread::spawn(move || {
        let _ = io::copy(&mut reader, &mut io::sink());
    });
    let mut writer = pair.master.take_writer().unwrap();
    writer.write_all(b"\x1b[1;1R").unwrap();
    writer.flush().unwrap();
    let helper = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("windows_process_tree_helper.ps1");
    let mut cmd = CommandBuilder::new("pwsh.exe");
    cmd.args(["-NoLogo", "-NoProfile", "-NonInteractive"]);
    cmd.args(["-ExecutionPolicy", "Bypass", "-File"]);
    cmd.arg(helper.as_os_str());
    cmd.args(["-Role", "root", "-ReceiptDirectory"]);
    cmd.arg(dir.as_os_str());
    if let Some(policy) = policy {
        cmd.set_windows_process_tree_policy(policy);
    }
    let child = pair.slave.spawn_command(cmd).unwrap();

    (pair, child, writer, dir, stop)
}

fn read_tree_pids(dir: &Path) -> [u32; 3] {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        let pids = ["root", "child", "grandchild"].map(|role| {
            fs::read_to_string(dir.join(format!("{role}.pid")))
                .ok()
                .and_then(|pid| pid.parse::<u32>().ok())
        });
        if let [Some(root), Some(child), Some(grandchild)] = pids {
            return [root, child, grandchild];
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for helper PID receipts in {}",
            dir.display()
        );
        thread::sleep(Duration::from_millis(25));
    }
}

fn open_processes(pids: [u32; 3]) -> [OwnedHandle; 3] {
    pids.map(|pid| {
        let handle = unsafe { OpenProcess(SYNCHRONIZE, FALSE, pid) };
        assert!(
            !handle.is_null(),
            "failed to open helper process {}: {}",
            pid,
            std::io::Error::last_os_error()
        );
        unsafe { OwnedHandle::from_raw_handle(handle as _) }
    })
}

fn process_is_running(process: &OwnedHandle) -> bool {
    let result = unsafe { WaitForSingleObject(process.as_raw_handle() as _, 0) };
    match result {
        WAIT_TIMEOUT => true,
        WAIT_OBJECT_0 => false,
        WAIT_FAILED => panic!(
            "failed to wait on helper process: {}",
            std::io::Error::last_os_error()
        ),
        other => panic!("unexpected process wait result: {}", other),
    }
}

fn wait_for_processes_to_exit(processes: &[OwnedHandle]) {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    for process in processes {
        loop {
            if !process_is_running(process) {
                break;
            }
            assert!(Instant::now() < deadline, "helper process did not exit");
            thread::sleep(Duration::from_millis(25));
        }
    }
}

#[test]
fn owned_job_killer_survives_clone_drop_order_and_terminates_tree() {
    let (_pair, child, _writer, dir, stop) =
        spawn_tree("owned-job", Some(WindowsProcessTreePolicy::OwnedJob));
    let processes = open_processes(read_tree_pids(&dir));
    assert!(processes.iter().all(process_is_running));

    let killer = child.clone_killer();
    let mut last_killer = killer.clone_killer();
    drop(child);
    drop(killer);
    thread::sleep(Duration::from_millis(200));
    assert!(
        processes.iter().all(process_is_running),
        "dropping non-final Job handles killed the tree"
    );

    last_killer.kill().unwrap();
    wait_for_processes_to_exit(&processes);
    drop(stop);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn direct_child_default_kill_leaves_descendants_running() {
    let (_pair, mut child, _writer, dir, stop) = spawn_tree("direct-child", None);
    let processes = open_processes(read_tree_pids(&dir));
    assert!(processes.iter().all(process_is_running));

    child.kill().unwrap();
    wait_for_processes_to_exit(&processes[..1]);
    assert!(
        processes[1..].iter().all(process_is_running),
        "stock DirectChild kill unexpectedly terminated descendants"
    );

    drop(stop);
    wait_for_processes_to_exit(&processes[1..]);
    fs::remove_dir_all(&dir).unwrap();
}
