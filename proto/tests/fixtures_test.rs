use std::fs;

fn round_trip(path: &str, expected_type: &str) -> x4mp_proto::EventEnvelope {
    let raw = fs::read_to_string(path).unwrap();
    let env: x4mp_proto::EventEnvelope = serde_json::from_str(&raw).unwrap();
    assert_eq!(env.event_type, expected_type);
    let again = serde_json::to_string(&env).unwrap();
    let back: x4mp_proto::EventEnvelope = serde_json::from_str(&again).unwrap();
    assert_eq!(env, back);
    env
}

#[test]
fn fixture_handshake_round_trip() {
    let env = round_trip("tests/fixtures/handshake.json", "handshake");
    let _p: x4mp_proto::HandshakePayload = serde_json::from_value(env.payload).unwrap();
}

#[test]
fn fixture_handshake_ack_round_trip() {
    let env = round_trip("tests/fixtures/handshake.ack.json", "handshake.ack");
    let _p: x4mp_proto::HandshakeAckPayload = serde_json::from_value(env.payload).unwrap();
}

#[test]
fn fixture_session_ping_round_trip() {
    round_trip("tests/fixtures/session.ping.json", "session.ping");
}
