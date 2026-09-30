//! TG-406 behaviour over HTTP and WebSocket: creation, the message path, voting modes, quiz
//! rules and closing. Lifecycle is `polls_lifecycle_test.rs`, authorization `polls_privacy_test.rs`.

mod poll_support;

use poll_support::*;
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn a_poll_message_flows_through_history_and_realtime_with_live_counts() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "poll-flow-alice").await;
    let bob = register(base, "poll-flow-bob").await;
    let chat = create_chat(base, &alice, "poll-flow").await;
    join(base, &bob, chat).await;
    let mut alice_socket = open_socket(base, chat, &alice).await;
    let mut bob_socket = open_socket(base, chat, &bob).await;

    let (status, created) = create_poll(
        base,
        &alice,
        chat,
        json!({ "question": " Lunch? ", "options": ["noodles", "rice", "salad"] }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["content"], "Lunch?");
    assert_eq!(created["poll"]["question"], "Lunch?");
    assert_eq!(created["poll"]["chosen"], json!([]));
    assert_eq!(
        created["poll"]["options"][2],
        json!({ "text": "salad", "voters": 0 })
    );
    assert!(
        created["poll"].get("public_voters").is_none(),
        "anonymous by default"
    );
    let poll_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(
        created["poll"]["id"], poll_id,
        "a poll is keyed by its message"
    );

    // The ordinary message path delivers it, with the viewer's own (empty) ballot.
    let broadcast = next_type(&mut bob_socket, "broadcast").await;
    assert_eq!(broadcast["message_id"], poll_id);
    assert_eq!(broadcast["poll"]["question"], "Lunch?");
    assert_eq!(broadcast["poll"]["chosen"], json!([]));

    let poll = poll_id.parse().unwrap();
    let (status, voted) = vote(base, &bob, poll, &[1]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(voted["chosen"], json!([1]));
    assert_eq!(voted["total_voters"], 1);
    assert_eq!(voted["options"][1]["voters"], 1);

    let update = next_type(&mut alice_socket, "poll_updated").await;
    assert_eq!(update["message_id"], poll_id);
    assert_eq!(update["poll"]["total_voters"], 1);
    assert!(
        update["poll"].get("chosen").is_none(),
        "chat-wide frames carry no ballot"
    );

    let (_, changed) = vote(base, &bob, poll, &[0]).await;
    assert_eq!(changed["options"][0]["voters"], 1);
    assert_eq!(changed["options"][1]["voters"], 0);
    assert_eq!(changed["total_voters"], 1);
    assert!(changed["revision"].as_i64().unwrap() > voted["revision"].as_i64().unwrap());

    // History carries each viewer's own ballot.
    for (account, chosen) in [(&bob, json!([0])), (&alice, json!([]))] {
        let message = history(base, account, chat)
            .await
            .iter()
            .find(|message| message["id"] == poll_id)
            .unwrap()
            .clone();
        assert_eq!(message["poll"]["chosen"], chosen);
        assert_eq!(message["poll"]["options"][0]["voters"], 1);
    }

    let (status, retracted) = retract(base, &bob, poll).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retracted["total_voters"], 0);
    assert_eq!(retracted["chosen"], json!([]));

    assert_eq!(
        vote(base, &bob, poll, &[0, 1]).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        vote(base, &bob, poll, &[3]).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(vote(base, &bob, poll, &[]).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn multiple_choice_counts_voters_once_and_options_separately() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "poll-multi-alice").await;
    let bob = register(base, "poll-multi-bob").await;
    let chat = create_chat(base, &alice, "poll-multi").await;
    join(base, &bob, chat).await;
    let poll = poll(
        base,
        &alice,
        chat,
        json!({ "question": "Which days?", "options": ["Mon", "Tue", "Wed"], "multiple_choice": true }),
    )
    .await;

    let (_, state) = vote(base, &bob, poll, &[2, 0]).await;
    assert_eq!(state["chosen"], json!([0, 2]));
    let (_, state) = vote(base, &alice, poll, &[0]).await;
    assert_eq!(state["total_voters"], 2);
    assert_eq!(state["options"][0]["voters"], 2);
    assert_eq!(state["options"][2]["voters"], 1);
    assert_eq!(state["multiple_choice"], true);
    assert_eq!(
        vote(base, &bob, poll, &[1, 1]).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn quiz_answers_are_final_and_reveal_the_answer_only_to_those_who_answered() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "quiz-alice").await;
    let bob = register(base, "quiz-bob").await;
    let carol = register(base, "quiz-carol").await;
    let chat = create_chat(base, &alice, "quiz").await;
    join(base, &bob, chat).await;
    join(base, &carol, chat).await;
    let mut alice_socket = open_socket(base, chat, &alice).await;

    let bad = json!({ "question": "2+2?", "options": ["3", "4"], "quiz": true,
        "correct_option": 1, "multiple_choice": true });
    assert_eq!(
        create_poll(base, &alice, chat, bad).await.0,
        StatusCode::BAD_REQUEST
    );
    let quiz = poll(
        base,
        &alice,
        chat,
        json!({ "question": "2+2?", "options": ["3", "4", "5"], "quiz": true,
            "correct_option": 1, "explanation": "Arithmetic." }),
    )
    .await;

    let (_, unanswered) = get_poll(base, &carol, quiz).await;
    assert_eq!(unanswered["quiz"], true);
    assert!(
        unanswered.get("correct_option").is_none(),
        "hidden before answering"
    );
    assert!(unanswered.get("explanation").is_none());

    let (status, answered) = vote(base, &bob, quiz, &[0]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answered["correct_option"], 1);
    assert_eq!(answered["explanation"], "Arithmetic.");
    assert_eq!(answered["chosen"], json!([0]));

    assert_eq!(
        vote(base, &bob, quiz, &[1]).await.0,
        StatusCode::CONFLICT,
        "cannot change"
    );
    assert_eq!(
        retract(base, &bob, quiz).await.0,
        StatusCode::CONFLICT,
        "cannot retract"
    );

    let update = next_type(&mut alice_socket, "poll_updated").await;
    assert!(
        update["poll"].get("correct_option").is_none(),
        "chat-wide frame keeps the answer"
    );

    assert_eq!(
        close_poll(base, &bob, quiz).await.0,
        StatusCode::FORBIDDEN,
        "not the author, not an admin"
    );
    let (status, closed) = close_poll(base, &alice, quiz).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(closed["closed"], true);
    let closed_frame = next_type(&mut alice_socket, "poll_updated").await;
    assert_eq!(closed_frame["poll"]["closed"], true);
    assert_eq!(
        closed_frame["poll"]["correct_option"], 1,
        "revealed to all once closed"
    );

    assert_eq!(
        vote(base, &carol, quiz, &[1]).await.0,
        StatusCode::CONFLICT,
        "closed"
    );
    let (_, after) = get_poll(base, &carol, quiz).await;
    assert_eq!(after["correct_option"], 1);
}

#[tokio::test]
async fn closing_is_limited_to_the_author_and_chat_administrators() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "close-owner").await;
    let author = register(base, "close-author").await;
    let member = register(base, "close-member").await;
    let chat = create_chat(base, &owner, "close").await;
    join(base, &author, chat).await;
    join(base, &member, chat).await;
    let poll = poll(
        base,
        &author,
        chat,
        json!({ "question": "Q", "options": ["a", "b"] }),
    )
    .await;
    assert_eq!(
        close_poll(base, &member, poll).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        close_poll(base, &owner, poll).await.0,
        StatusCode::OK,
        "the chat owner may stop any poll"
    );
    assert_eq!(
        close_poll(base, &author, poll).await.0,
        StatusCode::OK,
        "closing twice is a no-op"
    );
    assert_eq!(retract(base, &member, poll).await.0, StatusCode::CONFLICT);
}
