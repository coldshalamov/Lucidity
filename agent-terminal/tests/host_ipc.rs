use agent_protocol::{
    HostEvent, HostInstanceId, HostRequest, IpcRequest, MAX_FRAME_BYTES,
    MAX_PENDING_EVENTS_PER_SUBSCRIBER,
};
use agent_terminal::ipc::{
    encode_frame, owner_dacl_sddl, pipe_name_for_owner_sid, read_frame, read_frame_string,
    HostEventSubscriber, SubscriberPush,
};
use std::io::Cursor;
use uuid::Uuid;

fn host() -> HostInstanceId {
    HostInstanceId(Uuid::from_u128(0xfeed))
}

#[test]
fn ipc_frame_roundtrip_rejects_zero_oversized_truncated_and_invalid_utf8() {
    let request = IpcRequest {
        id: 42,
        request: HostRequest::HostStatus,
    };
    let frame = encode_frame(&request).unwrap();
    let decoded: IpcRequest = read_frame(&mut Cursor::new(frame)).unwrap();
    assert_eq!(decoded, request);

    let zero = 0u32.to_le_bytes().to_vec();
    assert!(matches!(
        read_frame_string(&mut Cursor::new(zero)),
        Err(agent_terminal::ipc::FrameError::ZeroLength)
    ));

    let oversized = ((MAX_FRAME_BYTES + 1) as u32).to_le_bytes().to_vec();
    assert!(matches!(
        read_frame_string(&mut Cursor::new(oversized)),
        Err(agent_terminal::ipc::FrameError::Oversized { .. })
    ));

    let mut truncated = 10u32.to_le_bytes().to_vec();
    truncated.extend_from_slice(b"abc");
    assert!(matches!(
        read_frame_string(&mut Cursor::new(truncated)),
        Err(agent_terminal::ipc::FrameError::Truncated {
            expected: 10,
            actual: 3
        })
    ));

    let mut invalid = 2u32.to_le_bytes().to_vec();
    invalid.extend_from_slice(&[0xff, 0xff]);
    assert!(matches!(
        read_frame_string(&mut Cursor::new(invalid)),
        Err(agent_terminal::ipc::FrameError::InvalidUtf8(_))
    ));
}

#[test]
fn subscriber_queue_overflow_requires_snapshot_resync() {
    let mut subscriber = HostEventSubscriber::new();
    for index in 0..MAX_PENDING_EVENTS_PER_SUBSCRIBER {
        assert_eq!(
            subscriber.push(HostEvent::CatalogChanged {
                catalog_snapshot_version: index as u64,
            }),
            SubscriberPush::Queued
        );
    }
    assert_eq!(
        subscriber.push(HostEvent::CatalogChanged {
            catalog_snapshot_version: MAX_PENDING_EVENTS_PER_SUBSCRIBER as u64,
        }),
        SubscriberPush::ResyncRequired
    );
    assert!(subscriber.resync_required());
    assert_eq!(subscriber.len(), 0);
    subscriber.clear_after_snapshot();
    assert!(!subscriber.resync_required());
}

#[test]
fn pipe_name_and_owner_dacl_are_deterministic_and_sid_scoped() {
    let sid = "S-1-5-21-111-222-333-1001";
    let pipe_a = pipe_name_for_owner_sid(sid);
    let pipe_b = pipe_name_for_owner_sid(sid);
    let other = pipe_name_for_owner_sid("S-1-5-21-111-222-333-1002");
    assert_eq!(pipe_a, pipe_b);
    assert_ne!(pipe_a, other);
    assert!(pipe_a.starts_with(r"\\.\pipe\lucidity-agent-"));

    let dacl = owner_dacl_sddl(sid);
    assert_eq!(dacl, "D:P(A;;GA;;;S-1-5-21-111-222-333-1001)(A;;GA;;;SY)");
}

#[cfg(windows)]
#[test]
fn windows_current_process_sid_probe_matches_owner() {
    let sid = agent_terminal::ipc::named_pipe::current_user_sid_string().unwrap();
    let pid = std::process::id();
    assert!(agent_terminal::ipc::named_pipe::process_sid_equals_owner(pid, &sid).unwrap());
    let probe = agent_terminal::windows::probe_named_pipe_security();
    assert_eq!(probe.current_user_sid.as_deref(), Some(sid.as_str()));
    assert!(probe
        .pipe_name
        .as_deref()
        .unwrap()
        .starts_with(r"\\.\pipe\lucidity-agent-"));
}
