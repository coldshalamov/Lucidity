use k9::assert_equal;
use lucidity_host::{serve_blocking, FakePaneBridge, PaneInfo, TYPE_JSON, TYPE_PANE_OUTPUT};
use lucidity_proto::frame::{encode_frame, FrameDecoder};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

fn read_next_frame(stream: &mut TcpStream, dec: &mut FrameDecoder) -> lucidity_proto::frame::Frame {
    let mut buf = [0u8; 4096];
    loop {
        if let Some(f) = dec.next_frame().unwrap() {
            return f;
        }
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0);
        dec.push(&buf[..n]);
    }
}

#[test]
fn tcp_server_lists_and_attaches_and_streams_output() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let fake = Arc::new(FakePaneBridge::new(vec![PaneInfo {
        pane_id: 123,
        title: "test".to_string(),
    }]));

    std::thread::spawn({
        let fake = Arc::clone(&fake);
        move || {
            serve_blocking(listener, fake).unwrap();
        }
    });

    let mut stream = TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();

    let list_req = serde_json::to_vec(&serde_json::json!({ "op": "list_panes" })).unwrap();
    stream.write_all(&encode_frame(TYPE_JSON, &list_req)).unwrap();

    let mut dec = FrameDecoder::new();
    let resp = read_next_frame(&mut stream, &mut dec);
    assert_equal!(resp.typ, TYPE_JSON);
    let v: serde_json::Value = serde_json::from_slice(&resp.payload).unwrap();
    assert_equal!(v["op"], "list_panes");
    assert_equal!(v["panes"][0]["pane_id"], 123);

    // Pairing info should be available
    let pair_info_req = serde_json::to_vec(&serde_json::json!({ "op": "pair_info" })).unwrap();
    stream
        .write_all(&encode_frame(TYPE_JSON, &pair_info_req))
        .unwrap();
    let pair_resp = read_next_frame(&mut stream, &mut dec);
    assert_equal!(pair_resp.typ, TYPE_JSON);
    let pair_v: serde_json::Value = serde_json::from_slice(&pair_resp.payload).unwrap();
    assert_equal!(pair_v["op"], "pair_info");
    let pairing_code = pair_v["pairing"]["pairing_code"].as_str().unwrap().to_string();

    // Claim by code should return the same token/code tuple
    let pair_claim_req = serde_json::to_vec(&serde_json::json!({
        "op": "pair_claim",
        "code": pairing_code,
    }))
    .unwrap();
    stream
        .write_all(&encode_frame(TYPE_JSON, &pair_claim_req))
        .unwrap();
    let claim_resp = read_next_frame(&mut stream, &mut dec);
    assert_equal!(claim_resp.typ, TYPE_JSON);
    let claim_v: serde_json::Value = serde_json::from_slice(&claim_resp.payload).unwrap();
    assert_equal!(claim_v["op"], "pair_info");

    let attach_req =
        serde_json::to_vec(&serde_json::json!({ "op": "attach", "pane_id": 123 })).unwrap();
    stream.write_all(&encode_frame(TYPE_JSON, &attach_req)).unwrap();

    // Wait for attach ok
    loop {
        let f = read_next_frame(&mut stream, &mut dec);
        if f.typ == TYPE_JSON {
            let v: serde_json::Value = serde_json::from_slice(&f.payload).unwrap();
            if v["op"] == "attach_ok" {
                break;
            }
        }
    }

    // Verify that input is accepted and routed to the selected pane
    stream
        .write_all(&encode_frame(lucidity_host::TYPE_PANE_INPUT, b"ls\r\n"))
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let inputs = fake.take_inputs();
    assert_equal!(inputs.len(), 1);
    assert_equal!(inputs[0].0, 123);
    assert_equal!(inputs[0].1, b"ls\r\n");

    fake.emit_output(123, b"hello");

    // Expect a pane output frame
    loop {
        let f = read_next_frame(&mut stream, &mut dec);
        if f.typ == TYPE_PANE_OUTPUT {
            assert_equal!(f.payload, b"hello");
            break;
        }
    }
}
