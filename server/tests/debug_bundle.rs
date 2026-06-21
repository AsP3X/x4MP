use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;
use x4mp_proto::{Authority, EventEnvelope};
use x4mp_server::{
    write_debug_bundle, AclDecision, DebugBundleRequest, EventLog, HashBreakdown,
};

fn test_data_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("x4mp-debug-bundle-{}", Uuid::new_v4()))
}

fn write_sample_log(log: &EventLog, session_id: Uuid, count: usize) {
    for seq in 1..=count {
        log.append(&EventEnvelope {
            v: 1,
            event_type: "session.ping".into(),
            session_id,
            seq: seq as u64,
            timestamp_sim: 0.0,
            sender_id: Uuid::new_v4(),
            authority: Authority::Client,
            payload: serde_json::json!({}),
        })
        .unwrap();
    }
}

fn assert_bundle_files(bundle_dir: &Path) {
    assert!(bundle_dir.join("meta.json").is_file());
    assert!(bundle_dir.join("last_events.jsonl").is_file());
    assert!(bundle_dir.join("acl_decisions.json").is_file());
    assert!(bundle_dir.join("hashes.json").is_file());
}

#[test]
fn debug_bundle_writes_all_expected_files_on_rejection() {
    let data_root = test_data_root();
    let session_id = Uuid::new_v4();
    let log = EventLog::for_session(&data_root, session_id).unwrap();
    write_sample_log(&log, session_id, 3);

    let request = DebugBundleRequest::from_rejection(
        session_id,
        "event.rejected",
        Some(Uuid::new_v4()),
        Some(Uuid::new_v4()),
        "INVALID_AUTHORITY",
        "Client cannot emit this event type.",
    );

    let bundle_dir = write_debug_bundle(&data_root, &log, &request).unwrap();
    assert_bundle_files(&bundle_dir);

    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bundle_dir.join("meta.json")).unwrap())
            .unwrap();
    assert_eq!(meta["trigger"], "event.rejected");
    assert_eq!(meta["session_id"], session_id.to_string());

    let acl: Vec<AclDecision> =
        serde_json::from_str(&std::fs::read_to_string(bundle_dir.join("acl_decisions.json")).unwrap())
            .unwrap();
    assert_eq!(acl.len(), 1);
    assert_eq!(acl[0].code, "INVALID_AUTHORITY");

    let tail_content = std::fs::read_to_string(bundle_dir.join("last_events.jsonl")).unwrap();
    let tail_lines: Vec<&str> = tail_content.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(tail_lines.len(), 3);

    let hashes: HashBreakdown =
        serde_json::from_str(&std::fs::read_to_string(bundle_dir.join("hashes.json")).unwrap())
            .unwrap();
    assert!(hashes.host_hash.is_none());
    assert!(hashes.reporter_hash.is_none());

    let _ = std::fs::remove_dir_all(&data_root);
}

#[test]
fn debug_bundle_tails_at_most_200_events() {
    let data_root = test_data_root();
    let session_id = Uuid::new_v4();
    let log = EventLog::for_session(&data_root, session_id).unwrap();
    write_sample_log(&log, session_id, 205);

    let request = DebugBundleRequest::from_rejection(
        session_id,
        "event.rejected",
        None,
        None,
        "TEST",
        "test",
    );
    let bundle_dir = write_debug_bundle(&data_root, &log, &request).unwrap();

    let tail_content = std::fs::read_to_string(bundle_dir.join("last_events.jsonl")).unwrap();
    let tail_lines: Vec<&str> = tail_content.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(tail_lines.len(), 200);

    let first: EventEnvelope = serde_json::from_str(tail_lines[0]).unwrap();
    assert_eq!(first.seq, 6);

    let _ = std::fs::remove_dir_all(&data_root);
}

#[test]
fn debug_bundle_hash_mismatch_includes_hash_breakdown() {
    let data_root = test_data_root();
    let session_id = Uuid::new_v4();
    let log = EventLog::for_session(&data_root, session_id).unwrap();

    let reporter = Uuid::new_v4();
    let request = DebugBundleRequest::from_hash_mismatch(
        session_id,
        Some(Uuid::new_v4()),
        Some(reporter),
        HashBreakdown {
            host_hash: Some("abc123".into()),
            reporter_hash: Some("def456".into()),
            reporter_client_id: Some(reporter),
            seq_at_report: Some(42),
        },
        vec![AclDecision {
            timestamp_utc_secs: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            session_id,
            client_id: Some(reporter),
            trace_id: None,
            event_type: Some("world.hash".into()),
            code: "HASH_MISMATCH".into(),
            message: "Reporter hash differs from host.".into(),
        }],
    );

    let bundle_dir = write_debug_bundle(&data_root, &log, &request).unwrap();
    assert_bundle_files(&bundle_dir);

    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bundle_dir.join("meta.json")).unwrap())
            .unwrap();
    assert_eq!(meta["trigger"], "world.hash_mismatch");

    let hashes: HashBreakdown =
        serde_json::from_str(&std::fs::read_to_string(bundle_dir.join("hashes.json")).unwrap())
            .unwrap();
    assert_eq!(hashes.host_hash.as_deref(), Some("abc123"));
    assert_eq!(hashes.reporter_hash.as_deref(), Some("def456"));

    let _ = std::fs::remove_dir_all(&data_root);
}
