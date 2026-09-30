//! TG-406: `PollState` grew additively. The TG-007 fields keep their exact bytes (pinned in
//! `ws_frame_snapshot_extension_test.rs`); every TG-406 field is omitted at its default and
//! serialised in declaration order when set. Also pins the `broadcast` frame's optional `poll`.

use serde_json::json;
use uuid::Uuid;

use chat_room::models::{ChatMessage, PollOption, PollState};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn serialized(message: &ChatMessage) -> serde_json::Value {
    serde_json::to_value(message).unwrap()
}

/// TG-406 grew `PollState` additively: the fields above keep their exact bytes (asserted in
/// the TG-007 case), and every TG-406 field is omitted at its default.
#[test]
fn poll_updated_carries_the_tg406_additive_fields() {
    assert_eq!(
        serialized(&ChatMessage::PollUpdated {
            message_id: id(52),
            poll: PollState {
                id: id(52),
                question: "2+2?".into(),
                closed: true,
                total_voters: 4,
                options: vec![
                    PollOption {
                        text: "4".into(),
                        voters: 3
                    },
                    PollOption {
                        text: "5".into(),
                        voters: 1
                    },
                ],
                public_voters: true,
                multiple_choice: false,
                quiz: true,
                correct_option: Some(0),
                explanation: Some("arithmetic".into()),
                chosen: Some(vec![1]),
                revision: 9,
            },
        }),
        json!({ "type": "poll_updated", "message_id": id(52), "poll": {
            "id": id(52), "question": "2+2?", "closed": true, "total_voters": 4,
            "options": [ { "text": "4", "voters": 3 }, { "text": "5", "voters": 1 } ],
            "public_voters": true, "quiz": true, "correct_option": 0,
            "explanation": "arithmetic", "chosen": [1], "revision": 9
        } })
    );
}

#[test]
fn a_default_poll_state_serialises_only_the_tg007_fields() {
    let poll = PollState {
        id: id(1),
        question: "q".into(),
        ..PollState::default()
    };
    assert_eq!(
        serde_json::to_string(&poll).unwrap(),
        format!(
            r#"{{"id":"{}","question":"q","closed":false,"total_voters":0,"options":[]}}"#,
            id(1)
        )
    );
    // A pre-TG-406 payload still parses, with every new field at its default.
    let parsed: PollState = serde_json::from_value(json!({
        "id": id(1), "question": "q", "closed": false, "total_voters": 0, "options": []
    }))
    .unwrap();
    assert_eq!(parsed, poll);
}
