use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

const HANDSHAKE: &str = r#"{"v":1,"type":"handshake","session_id":"00000000-0000-0000-0000-000000000000","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{"mod_version":"0.1.0","proto_version":1,"bridge_version":"0.1.0","game_version":"7.5","mods_fingerprint":"sha256:0000","join_code":"ABCD-1234","display_name":"Alice"}}"#;

const BAD_PROTO_HANDSHAKE: &str = r#"{"v":1,"type":"handshake","session_id":"00000000-0000-0000-0000-000000000000","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{"mod_version":"0.1.0","proto_version":99,"bridge_version":"0.1.0","game_version":"7.5","mods_fingerprint":"sha256:0000","join_code":"ABCD-1234","display_name":"Alice"}}"#;

fn test_data_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("x4mp-handshake-test-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn ws_handshake_assigns_client_id() {
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
    assert!(ack["payload"]["client_id"].as_str().unwrap().len() > 0);

    let _ = std::fs::remove_dir_all(&data_dir);
    std::env::remove_var("X4MP_DATA_DIR");
}

#[tokio::test]
async fn ws_rejects_incompatible_proto_version() {
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

    ws.send(Message::Text(BAD_PROTO_HANDSHAKE.into())).await.unwrap();
    let err = ws.next().await.unwrap().unwrap().into_text().unwrap();
    let err: serde_json::Value = serde_json::from_str(&err).unwrap();
    assert_eq!(err["type"], "error");
    assert_eq!(err["code"], "INCOMPATIBLE_VERSION");

    let _ = std::fs::remove_dir_all(&data_dir);
    std::env::remove_var("X4MP_DATA_DIR");
}
