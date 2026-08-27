use chat_room::ApiDoc;
use utoipa::OpenApi;

#[test]
fn in_progress_features_are_present_in_the_openapi_contract() {
    let document = serde_json::to_value(ApiDoc::openapi()).expect("serialize OpenAPI document");
    let paths = document["paths"].as_object().expect("OpenAPI paths object");

    for (path, methods) in [
        ("/api/rooms/discover", &["get"][..]),
        ("/api/rooms/{room_id}/pins", &["get"][..]),
        (
            "/api/rooms/{room_id}/pins/{message_id}",
            &["post", "delete"][..],
        ),
        ("/api/rooms/{id}/messages/search", &["get"][..]),
        ("/api/messages/search", &["get"][..]),
        ("/api/notifications", &["get"][..]),
        ("/api/notifications/unread-count", &["get"][..]),
        ("/api/notifications/{id}/read", &["post"][..]),
        ("/api/notifications/read-all", &["post"][..]),
        (
            "/api/rooms/{id}/messages/{message_id}/context",
            &["get"][..],
        ),
        ("/api/rooms/{id}/ai/suggest/events", &["post"][..]),
        ("/api/rooms/{id}/ai-policy", &["get", "patch"][..]),
        ("/api/ai/threads/{id}/runs", &["post"][..]),
        ("/api/ai/threads/{id}/catch-up", &["post"][..]),
        ("/api/ai/runs/{id}", &["get"][..]),
        ("/api/ai/runs/{id}/events", &["get"][..]),
        ("/api/rooms/{room_id}/tasks", &["get", "post"][..]),
        ("/api/rooms/{room_id}/ai/extractions", &["post"][..]),
        ("/api/ai/extractions/{id}", &["get"][..]),
        ("/api/ai/extraction-candidates/{id}", &["patch"][..]),
        ("/api/admin/ai-governance", &["get", "patch"][..]),
        ("/api/admin/ai-usage", &["get"][..]),
        (
            "/api/rooms/{room_id}/tasks/{task_id}",
            &["patch", "delete"][..],
        ),
        ("/api/favorites/attachments", &["post"][..]),
    ] {
        let operations = paths
            .get(path)
            .unwrap_or_else(|| panic!("OpenAPI is missing {path}"));
        for method in methods {
            assert!(
                operations.get(method).is_some(),
                "OpenAPI is missing {method} {path}"
            );
        }
    }
}
