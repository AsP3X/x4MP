use uuid::Uuid;

const PING: &str = r#"{"v":1,"type":"session.ping","session_id":"00000000-0000-0000-0000-000000000001","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{}}"#;

fn test_data_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("x4mp-bridge-test-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn bridge_handshake_and_echo() {
    let data_dir = test_data_root();
    std::env::set_var("X4MP_DATA_DIR", data_dir.to_string_lossy().as_ref());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        x4mp_server::run_on_listener(listener).await.unwrap();
    });

    let config = x4mp_bridge::BridgeConfig {
        server_url: format!("ws://{addr}/ws"),
        display_name: "Alice".into(),
        join_code: "ABCD-1234".into(),
        game_version: "7.5".into(),
        mods_fingerprint: "sha256:0000".into(),
    };

    let (_hs, mut ws) = x4mp_bridge::perform_handshake(&config).await.unwrap();
    let echoed = x4mp_bridge::send_and_receive(&mut ws, PING).await.unwrap();
    assert_eq!(echoed.event_type, "session.ping");
    assert!(echoed.seq >= 1);

    let _ = std::fs::remove_dir_all(&data_dir);
    std::env::remove_var("X4MP_DATA_DIR");
}
