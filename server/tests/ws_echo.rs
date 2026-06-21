use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use x4mp_proto::EventEnvelope;
use x4mp_server::{data_root, DEV_SESSION_ID};

const HANDSHAKE: &str = r#"{"v":1,"type":"handshake","session_id":"00000000-0000-0000-0000-000000000000","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{"mod_version":"0.1.0","proto_version":1,"bridge_version":"0.1.0","game_version":"7.5","mods_fingerprint":"sha256:0000","join_code":"ABCD-1234","display_name":"Alice"}}"#;

fn test_data_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("x4mp-echo-test-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn ws_echoes_envelope_with_seq() {
    let data_dir = test_data_root();
    std::env::set_var("X4MP_DATA_DIR", data_dir.to_string_lossy().as_ref());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        x4mp_server::run_on_listener(listener).await.unwrap();
    });

    let (mut ws, _) = connect_async(format!("ws://{addr}/ws"))
        .await
        .expect("connect");

    ws.send(Message::Text(HANDSHAKE.into())).await.unwrap();
    let ack = ws.next().await.unwrap().unwrap().into_text().unwrap();
    let ack: serde_json::Value = serde_json::from_str(&ack).unwrap();
    assert_eq!(ack["type"], "handshake.ack");

    ws.send(Message::Text(
        r#"{"v":1,"type":"session.ping","session_id":"00000000-0000-0000-0000-000000000001","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{}}"#.into(),
    ))
    .await
    .unwrap();

    let msg = ws.next().await.unwrap().unwrap();
    let text = msg.into_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["type"], "session.ping");
    let echoed_seq = parsed["seq"].as_u64().unwrap();
    assert!(echoed_seq >= 1);

    // Verify event log was written with matching seq.
    let log_path = data_root()
        .join("sessions")
        .join(DEV_SESSION_ID.to_string())
        .join("event.log");
    let content = std::fs::read_to_string(&log_path).expect("event.log exists");
    let last_line = content.lines().last().unwrap();
    let logged: EventEnvelope = serde_json::from_str(last_line).unwrap();
    assert_eq!(logged.seq, echoed_seq);

    let _ = std::fs::remove_dir_all(&data_dir);
    std::env::remove_var("X4MP_DATA_DIR");
}
