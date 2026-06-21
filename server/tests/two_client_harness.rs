use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

const HANDSHAKE: &str = r#"{"v":1,"type":"handshake","session_id":"00000000-0000-0000-0000-000000000000","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{"mod_version":"0.1.0","proto_version":1,"bridge_version":"0.1.0","game_version":"7.5","mods_fingerprint":"sha256:0000","join_code":"ABCD-1234","display_name":"Alice"}}"#;

const PING: &str = r#"{"v":1,"type":"session.ping","session_id":"00000000-0000-0000-0000-000000000001","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{}}"#;

fn test_data_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("x4mp-harness-{}", Uuid::new_v4()))
}

async fn connect_and_handshake(addr: &str) -> tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
> {
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws"))
        .await
        .expect("connect");
    ws.send(Message::Text(HANDSHAKE.into())).await.unwrap();
    let ack = ws.next().await.unwrap().unwrap().into_text().unwrap();
    let ack: serde_json::Value = serde_json::from_str(&ack).unwrap();
    assert_eq!(ack["type"], "handshake.ack");
    ws
}

#[tokio::test]
async fn two_clients_receive_monotonic_seq() {
    let data_dir = test_data_root();
    std::env::set_var("X4MP_DATA_DIR", data_dir.to_string_lossy().as_ref());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        x4mp_server::run_on_listener(listener).await.unwrap();
    });

    let mut client_a = connect_and_handshake(&addr).await;
    let mut client_b = connect_and_handshake(&addr).await;

    client_a.send(Message::Text(PING.into())).await.unwrap();
    let msg_a = client_a.next().await.unwrap().unwrap().into_text().unwrap();
    let parsed_a: serde_json::Value = serde_json::from_str(&msg_a).unwrap();
    let seq_a = parsed_a["seq"].as_u64().unwrap();
    assert!(seq_a >= 1);

    client_b.send(Message::Text(PING.into())).await.unwrap();
    let msg_b = client_b.next().await.unwrap().unwrap().into_text().unwrap();
    let parsed_b: serde_json::Value = serde_json::from_str(&msg_b).unwrap();
    let seq_b = parsed_b["seq"].as_u64().unwrap();
    assert!(seq_b > seq_a);

    let _ = std::fs::remove_dir_all(&data_dir);
    std::env::remove_var("X4MP_DATA_DIR");
}
