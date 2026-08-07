use agent_backends::discovery::{ExecutableLocator, SearchPaths};
use agent_backends::manifest::load_package;
use agent_protocol::{AdapterId, AdapterManifest};
use std::fs;
use std::path::{Path, PathBuf};

struct AdapterCase {
    directory: &'static str,
    id: &'static str,
    executable: &'static str,
    launch_args: &'static [&'static str],
    resume_args: &'static [&'static str],
}

const ADAPTERS: [AdapterCase; 3] = [
    AdapterCase {
        directory: "claude.agent-adapter",
        id: "claude",
        executable: "claude",
        launch_args: &[],
        resume_args: &["--resume", "{nativeSessionId}"],
    },
    AdapterCase {
        directory: "codex.agent-adapter",
        id: "codex",
        executable: "codex",
        launch_args: &["--cd", "{projectPath}"],
        resume_args: &["resume", "{nativeSessionId}", "--cd", "{projectPath}"],
    },
    AdapterCase {
        directory: "kimi.agent-adapter",
        id: "kimi",
        executable: "kimi",
        launch_args: &[],
        resume_args: &["--session", "{nativeSessionId}"],
    },
];

#[test]
fn installed_cli_adapter_packages_match_the_frozen_manifest_contract() {
    for case in &ADAPTERS {
        let manifest = load(case);
        let launch_args = strings(case.launch_args);
        let resume_args = strings(case.resume_args);
        assert_eq!(manifest.schema_version, 1, "{} schema", case.id);
        assert_eq!(manifest.id, AdapterId::from(case.id), "{} id", case.id);
        assert_eq!(
            manifest.launch.executable, case.executable,
            "{} launch",
            case.id
        );
        assert_eq!(manifest.launch.args, launch_args, "{} launch argv", case.id);
        assert!(!manifest.launch.shell, "{} launch must be direct", case.id);

        let resume = manifest
            .resume
            .as_ref()
            .expect("production CLI supports resume");
        assert_eq!(resume.executable, case.executable, "{} resume", case.id);
        assert_eq!(resume.args, resume_args, "{} resume argv", case.id);
        assert!(!resume.shell, "{} resume must be direct", case.id);

        let probe = manifest
            .executable
            .version_probe
            .as_ref()
            .expect("production CLI has a version probe");
        assert_eq!(probe.executable, case.executable, "{} probe", case.id);
        assert_eq!(probe.args, strings(&["--version"]), "{} probe argv", case.id);
        assert!(!probe.shell, "{} probe must be direct", case.id);
        assert!(manifest.event_capabilities.is_empty(), "{} must not claim unwired events", case.id);
    }
}

#[test]
fn declared_names_are_usable_by_deterministic_executable_discovery() {
    let temporary = tempfile::tempdir().expect("temporary discovery root");
    for case in &ADAPTERS {
        let manifest = load(case);
        let bin = temporary.path().join(case.id);
        fs::create_dir_all(&bin).expect("create fake bin");
        let declared_name = manifest
            .executable
            .names
            .first()
            .expect("adapter declares a PATH name");
        let executable = bin.join(declared_name);
        fs::write(&executable, b"").expect("create fake executable");

        let located = SearchPaths::new([bin])
            .locate(&manifest.executable)
            .expect("declared executable is discoverable");
        assert_eq!(located, executable, "{} discovery", case.id);
    }
}

fn load(case: &AdapterCase) -> AdapterManifest {
    load_package(&adapter_root().join(case.directory))
        .unwrap_or_else(|errors| panic!("{} package failed validation: {errors:?}", case.id))
}

fn adapter_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("adapters")
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
