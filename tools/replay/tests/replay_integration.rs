use uuid::Uuid;
use x4mp_proto::{Authority, EventEnvelope};
use x4mp_replay::{ReplayConfig, ReplayStats};
use x4mp_server::{EventLog, DEV_SESSION_ID};

fn test_data_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("x4mp-replay-int-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn replay_sample_log_against_running_server() {
    let data_root = test_data_root();
    std::env::set_var("X4MP_DATA_DIR", data_root.to_string_lossy().as_ref());

    let log = EventLog::for_session(&data_root, DEV_SESSION_ID).unwrap();
    for seq in 1..=3 {
        log.append(&EventEnvelope {
            v: 1,
            event_type: "session.ping".into(),
            session_id: DEV_SESSION_ID,
            seq,
            timestamp_sim: 0.0,
            sender_id: Uuid::new_v4(),
            authority: Authority::Client,
            payload: serde_json::json!({}),
        })
        .unwrap();
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        x4mp_server::run_on_listener(listener).await.unwrap();
    });

    let config = ReplayConfig {
        session_id: DEV_SESSION_ID,
        from_seq: 1,
        server_url: format!("ws://{addr}/ws"),
        data_root: data_root.clone(),
        display_name: "ReplayBot".into(),
        join_code: "ABCD-1234".into(),
    };

    let stats = x4mp_replay::replay_events(&config).await.unwrap();
    assert_eq!(
        stats,
        ReplayStats {
            events_sent: 3,
            last_seq: Some(3),
        }
    );

    let _ = std::fs::remove_dir_all(&data_root);
    std::env::remove_var("X4MP_DATA_DIR");
}
