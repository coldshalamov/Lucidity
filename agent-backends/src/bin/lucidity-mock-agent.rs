//! `lucidity-mock-agent` — deterministic PTY-friendly mock coding-agent TUI.
//!
//! This binary is the AT-103 mock harness. It simulates a native coding
//! agent TUI so the protected lifecycle can be tested without vendor CLIs,
//! network access, or credentials:
//!
//! - Slash commands: `/new`, `/work`, `/ask`, `/approve`, `/fail`,
//!   `/complete`, `/usage`, `/quit`. `/usage` emits the same structured
//!   `usage_updated` event shape the demo consumes and never needs hidden
//!   host input.
//! - `/new` changes the native session identity **in the same process** and
//!   emits an authoritative `session_start` structured event, exactly the
//!   transition the reducer must rebind on.
//! - Structured events use the frozen ADR-002 transport
//!   (`OSC 1337 SetUserVar=lucidity.agent-event.v1=<base64 JSON>`) with a
//!   monotonic per-process sequence (`evt-NNNN`, `data.seq`).
//! - `--resume <native-id>` restores identity, counters, and sequence from
//!   the deterministic session store; `--with-child` spawns an optional
//!   child process whose PID and cleanup are reported on stable marker
//!   lines.
//!
//! Determinism rules: no wall clock (event timestamps are a fixed epoch
//! plus the sequence number), no randomness (identities derive from
//! `--seed`), and no machine-dependent data in the session store. The only
//! non-deterministic bytes on stdout are OS-assigned PIDs, which the
//! transcript tests normalize before hashing.

use agent_backends::events;
use agent_protocol::{
    AdapterId, AgentEventEnvelope, AgentEventKind, ProfileId, AGENT_EVENT_VERSION,
};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{json, Map, Value};
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};

/// Fixed epoch for event timestamps; sequence seconds are added to it so
/// runs never depend on the machine clock.
const MOCK_EPOCH: &str = "2026-08-06T00:00:00Z";
const DEFAULT_ADAPTER: &str = "mock-agent";
const DEFAULT_PROFILE: &str = "11111111-1111-4111-8111-111111111111";
const DEFAULT_SEED: u64 = 1;
const USAGE_INPUT_PER_TURN: u64 = 120;
const USAGE_OUTPUT_PER_TURN: u64 = 42;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            println!("MOCK ERROR {message}");
            ExitCode::from(2)
        }
    }
}

struct Config {
    adapter: String,
    profile: String,
    store: PathBuf,
    seed: u64,
    pane: u64,
    resume: Option<String>,
    with_child: bool,
    cwd: PathBuf,
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    if args.iter().any(|arg| arg == "--child") {
        return run_child();
    }
    if args.iter().any(|arg| arg == "--print-version") {
        println!("lucidity-mock-agent 0.1.0");
        return Ok(ExitCode::SUCCESS);
    }

    let mut config = Config {
        adapter: DEFAULT_ADAPTER.to_owned(),
        profile: DEFAULT_PROFILE.to_owned(),
        store: PathBuf::from("mock-store"),
        seed: DEFAULT_SEED,
        pane: 0,
        resume: None,
        with_child: false,
        cwd: env::current_dir().map_err(|error| error.to_string())?,
    };

    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let mut take_value = |flag: &str| -> Result<String, String> {
            index += 1;
            args.get(index)
                .cloned()
                .ok_or_else(|| format!("{flag} requires a value"))
        };
        match flag {
            "--adapter" => config.adapter = take_value(flag)?,
            "--profile" => config.profile = take_value(flag)?,
            "--store" => config.store = PathBuf::from(take_value(flag)?),
            "--seed" => {
                config.seed = take_value(flag)?
                    .parse()
                    .map_err(|_| "invalid --seed value".to_owned())?
            }
            "--pane" => {
                config.pane = take_value(flag)?
                    .parse()
                    .map_err(|_| "invalid --pane value".to_owned())?
            }
            "--resume" => config.resume = Some(take_value(flag)?),
            "--with-child" => config.with_child = true,
            "--cwd" => config.cwd = PathBuf::from(take_value(flag)?),
            other => return Err(format!("unknown argument {other:?}")),
        }
        index += 1;
    }

    let profile_id: ProfileId = config
        .profile
        .parse()
        .map_err(|_| format!("invalid --profile UUID {:?}", config.profile))?;

    let mut session = MockSession::start(&config, profile_id)?;
    session.repl()
}

/// Child-process mode: stay alive (and observable in the process tree)
/// until the parent closes our stdin, then exit cleanly and silently. All
/// user-visible child evidence is printed by the parent, which knows the
/// child PID; this keeps transcripts deterministic.
fn run_child() -> Result<ExitCode, String> {
    let mut stdin = std::io::stdin().lock();
    let mut buffer = [0u8; 256];
    loop {
        match stdin.read(&mut buffer) {
            Ok(0) => return Ok(ExitCode::SUCCESS),
            Ok(_) => continue,
            Err(_) => return Ok(ExitCode::SUCCESS),
        }
    }
}

struct MockSession {
    adapter: AdapterId,
    profile_id: ProfileId,
    store_dir: PathBuf,
    cwd: PathBuf,
    pane: u64,
    seed: u64,
    counter: u64,
    seq: u64,
    turns: u64,
    child: Option<Child>,
}

impl MockSession {
    fn start(config: &Config, profile_id: ProfileId) -> Result<Self, String> {
        std::fs::create_dir_all(&config.store)
            .map_err(|error| format!("cannot create store dir: {error}"))?;

        let (seed, counter, seq, turns) = match &config.resume {
            Some(native_id) => {
                let record = StoreRecord::load(&config.store, native_id)?;
                let (seed, counter) = parse_native_id(native_id)
                    .ok_or_else(|| format!("malformed native id {native_id:?}"))?;
                (seed, counter, record.seq, record.turns)
            }
            None => (config.seed, 0, 0, 0),
        };

        let child = if config.with_child {
            let executable = env::current_exe().map_err(|error| error.to_string())?;
            let child = Command::new(executable)
                .arg("--child")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| format!("cannot spawn child: {error}"))?;
            Some(child)
        } else {
            None
        };

        let mut session = Self {
            adapter: AdapterId::from(config.adapter.as_str()),
            profile_id,
            store_dir: config.store.clone(),
            cwd: config.cwd.clone(),
            pane: config.pane,
            seed,
            counter,
            seq,
            turns,
            child,
        };
        session.emit(AgentEventKind::SessionStart, json!({}))?;
        session.persist(true)?;
        println!(
            "MOCK READY adapter={} profile={} pid={} pane={} session={} child={} seq={}",
            session.adapter,
            session.profile_id,
            std::process::id(),
            session.pane,
            session.native_id(),
            session
                .child
                .as_ref()
                .map(|child| child.id().to_string())
                .unwrap_or_else(|| "none".to_owned()),
            session.seq,
        );
        Ok(session)
    }

    fn native_id(&self) -> String {
        format!("mock-{}-{}", self.seed, self.counter)
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        let epoch: DateTime<Utc> = MOCK_EPOCH.parse().expect("static epoch parses");
        epoch + TimeDelta::seconds(self.seq as i64)
    }

    fn emit(&mut self, kind: AgentEventKind, extra: Value) -> Result<(), String> {
        self.seq += 1;
        let mut data = Map::new();
        data.insert("seq".to_owned(), json!(self.seq));
        if let Value::Object(extra) = extra {
            data.extend(extra);
        }
        let envelope = AgentEventEnvelope {
            v: AGENT_EVENT_VERSION,
            event_id: format!("evt-{0:04}", self.seq),
            adapter: self.adapter.clone(),
            profile_id: self.profile_id,
            event: kind,
            session_id: self.native_id(),
            cwd: self.cwd.clone(),
            transcript_path: None,
            occurred_at: self.occurred_at(),
            data: Value::Object(data),
        };
        let sequence = events::encode_osc_sequence(&envelope)
            .map_err(|error| format!("cannot encode event: {error}"))?;
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        lock.write_all(sequence.as_bytes())
            .and_then(|()| lock.write_all(b"\n"))
            .and_then(|()| lock.flush())
            .map_err(|error| format!("cannot write event: {error}"))
    }

    fn persist(&self, active: bool) -> Result<(), String> {
        let record = StoreRecord {
            active,
            adapter: self.adapter.to_string(),
            counter: self.counter,
            native_session_id: self.native_id(),
            profile_id: self.profile_id.to_string(),
            seed: self.seed,
            seq: self.seq,
            turns: self.turns,
            usage_input_tokens: self.turns * USAGE_INPUT_PER_TURN,
            usage_output_tokens: self.turns * USAGE_OUTPUT_PER_TURN,
        };
        record.save(&self.store_dir)
    }

    /// Line-oriented REPL. Prompts are flushed without a newline so the TUI
    /// behaves under a PTY; CRLF input is accepted; EOF is an implicit
    /// `/quit` so pipe-driven transcripts terminate deterministically.
    fn repl(&mut self) -> Result<ExitCode, String> {
        let stdin = std::io::stdin();
        let mut lines = BufReader::new(stdin.lock()).lines();
        loop {
            print!("mock> ");
            std::io::stdout()
                .flush()
                .map_err(|error| error.to_string())?;
            let line = match lines.next() {
                Some(Ok(line)) => line,
                Some(Err(error)) => return Err(format!("stdin error: {error}")),
                None => "/quit".to_owned(),
            };
            let line = line.trim_end_matches('\r').trim().to_owned();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(2, ' ');
            let command = parts.next().unwrap_or_default();
            let argument = parts.next().unwrap_or("").to_owned();
            match command {
                "/new" => self.command_new()?,
                "/work" => self.command_work(&argument)?,
                "/ask" => self.command_ask(&argument)?,
                "/approve" => self.command_approve()?,
                "/fail" => self.command_fail(&argument)?,
                "/complete" => self.command_complete()?,
                "/usage" => self.command_usage()?,
                "/quit" => return self.command_quit(),
                other => println!("MOCK ERROR unknown command {other:?}"),
            }
        }
    }

    /// `/new` changes the native identity in the same process: the old
    /// store record is settled, the counter advances, and an authoritative
    /// `session_start` for the new native ID is emitted on the same PTY.
    fn command_new(&mut self) -> Result<(), String> {
        self.persist(false)?;
        self.counter += 1;
        self.turns = 0;
        self.emit(AgentEventKind::SessionStart, json!({}))?;
        self.persist(true)?;
        println!(
            "MOCK OK /new pid={} session={}",
            std::process::id(),
            self.native_id()
        );
        Ok(())
    }

    fn command_work(&mut self, text: &str) -> Result<(), String> {
        self.turns += 1;
        self.emit(
            AgentEventKind::PromptSubmit,
            json!({ "turn": self.turns, "text": text }),
        )?;
        self.emit(AgentEventKind::ToolComplete, json!({ "turn": self.turns }))?;
        self.emit(AgentEventKind::TurnComplete, json!({ "turn": self.turns }))?;
        self.persist(true)?;
        println!("MOCK OK /work turn={}", self.turns);
        Ok(())
    }

    fn command_ask(&mut self, question: &str) -> Result<(), String> {
        self.emit(
            AgentEventKind::QuestionAsked,
            json!({ "question": question }),
        )?;
        println!("MOCK OK /ask");
        Ok(())
    }

    fn command_approve(&mut self) -> Result<(), String> {
        self.emit(
            AgentEventKind::PermissionRequest,
            json!({ "action": "tool" }),
        )?;
        self.emit(
            AgentEventKind::PermissionReplied,
            json!({ "approved": true }),
        )?;
        println!("MOCK OK /approve");
        Ok(())
    }

    fn command_fail(&mut self, message: &str) -> Result<(), String> {
        self.turns += 1;
        self.emit(
            AgentEventKind::TurnFailed,
            json!({ "turn": self.turns, "message": message }),
        )?;
        self.persist(true)?;
        println!("MOCK OK /fail turn={}", self.turns);
        Ok(())
    }

    fn command_complete(&mut self) -> Result<(), String> {
        self.turns += 1;
        self.emit(AgentEventKind::TurnComplete, json!({ "turn": self.turns }))?;
        self.persist(true)?;
        println!("MOCK OK /complete turn={}", self.turns);
        Ok(())
    }

    /// `/usage` emits the structured `usage_updated` event the demo
    /// consumes. The host never injects this command into a live TUI; here
    /// it is an explicit user command with deterministic payload.
    fn command_usage(&mut self) -> Result<(), String> {
        self.emit(
            AgentEventKind::UsageUpdated,
            json!({
                "inputTokens": self.turns * USAGE_INPUT_PER_TURN,
                "outputTokens": self.turns * USAGE_OUTPUT_PER_TURN,
            }),
        )?;
        println!(
            "MOCK OK /usage input={} output={}",
            self.turns * USAGE_INPUT_PER_TURN,
            self.turns * USAGE_OUTPUT_PER_TURN,
        );
        Ok(())
    }

    fn command_quit(&mut self) -> Result<ExitCode, String> {
        self.emit(AgentEventKind::SessionEnd, json!({}))?;
        self.persist(false)?;
        if let Some(mut child) = self.child.take() {
            let pid = child.id();
            // Closing stdin is the deterministic shutdown signal.
            drop(child.stdin.take());
            let status = child
                .wait()
                .map_err(|error| format!("cannot wait for child: {error}"))?;
            println!(
                "MOCK CHILD CLEANUP pid={} status={}",
                pid,
                status.code().unwrap_or(-1),
            );
        }
        println!("MOCK BYE session={}", self.native_id());
        Ok(ExitCode::SUCCESS)
    }
}

/// Deterministic session-store record. Keys serialize in sorted order via
/// `serde_json::Value`; there are no timestamps, PIDs, or secrets, so the
/// store hashes identically across runs and machines.
struct StoreRecord {
    active: bool,
    adapter: String,
    counter: u64,
    native_session_id: String,
    profile_id: String,
    seed: u64,
    seq: u64,
    turns: u64,
    usage_input_tokens: u64,
    usage_output_tokens: u64,
}

impl StoreRecord {
    fn path(store_dir: &Path, native_id: &str) -> PathBuf {
        store_dir.join(format!("{native_id}.json"))
    }

    fn to_value(&self) -> Value {
        json!({
            "active": self.active,
            "adapter": self.adapter,
            "counter": self.counter,
            "nativeSessionId": self.native_session_id,
            "profileId": self.profile_id,
            "seed": self.seed,
            "seq": self.seq,
            "turns": self.turns,
            "usage": {
                "inputTokens": self.usage_input_tokens,
                "outputTokens": self.usage_output_tokens,
            },
        })
    }

    fn save(&self, store_dir: &Path) -> Result<(), String> {
        let path = Self::path(store_dir, &self.native_session_id);
        let mut text =
            serde_json::to_string_pretty(&self.to_value()).map_err(|error| error.to_string())?;
        text.push('\n');
        std::fs::write(&path, text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    fn load(store_dir: &Path, native_id: &str) -> Result<Self, String> {
        let path = Self::path(store_dir, native_id);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot load session {native_id:?}: {error}"))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("corrupt store record: {error}"))?;
        Ok(Self {
            active: value["active"].as_bool().unwrap_or(false),
            adapter: value["adapter"].as_str().unwrap_or_default().to_owned(),
            counter: value["counter"].as_u64().unwrap_or(0),
            native_session_id: native_id.to_owned(),
            profile_id: value["profileId"].as_str().unwrap_or_default().to_owned(),
            seed: value["seed"].as_u64().unwrap_or(0),
            seq: value["seq"].as_u64().unwrap_or(0),
            turns: value["turns"].as_u64().unwrap_or(0),
            usage_input_tokens: value["usage"]["inputTokens"].as_u64().unwrap_or(0),
            usage_output_tokens: value["usage"]["outputTokens"].as_u64().unwrap_or(0),
        })
    }
}

fn parse_native_id(native_id: &str) -> Option<(u64, u64)> {
    let rest = native_id.strip_prefix("mock-")?;
    let (seed, counter) = rest.split_once('-')?;
    Some((seed.parse().ok()?, counter.parse().ok()?))
}
