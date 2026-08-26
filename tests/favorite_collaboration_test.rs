use reqwest::{Client, StatusCode};

#[allow(dead_code)]
mod favorites_support;
use favorites_support::{make_friends, register, start_server};

#[tokio::test]
async fn favorite_owner_and_collaborator_edit_with_version_conflict_protection() {
    let server = start_server().await;
    let client = Client::new();
    let (owner_token, owner_id) = register(&client, &server.base, "favorite-owner").await;
    let (editor_token, editor_id) = register(&client, &server.base, "favorite-editor").await;
    let (outsider_token, outsider_id) = register(&client, &server.base, "favorite-other").await;
    make_friends(
        &client,
        &server.base,
        &owner_token,
        owner_id,
        &editor_token,
        editor_id,
    )
    .await;

    let favorite: serde_json::Value = client
        .post(format!("{}/api/favorites", server.base))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "title": "协作文档", "content": "第一版" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let favorite_id = favorite["id"].as_str().unwrap();
    assert_eq!(favorite["access"], "owner");
    assert_eq!(favorite["version"], 1);

    let collaborator = client
        .post(format!(
            "{}/api/favorites/{favorite_id}/collaborators",
            server.base
        ))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "user_id": editor_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(collaborator.status(), StatusCode::CREATED);

    let editor_favorites: Vec<serde_json::Value> = client
        .get(format!("{}/api/favorites", server.base))
        .bearer_auth(&editor_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(editor_favorites[0]["access"], "editor");
    assert_eq!(editor_favorites[0]["owner_id"], owner_id.to_string());

    let edited: serde_json::Value = client
        .put(format!("{}/api/favorites/{favorite_id}", server.base))
        .bearer_auth(&editor_token)
        .json(&serde_json::json!({ "version": 1, "title": "协作文档", "content": "第二版" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(edited["content"], "第二版");
    assert_eq!(edited["version"], 2);

    assert_eq!(
        client
            .put(format!("{}/api/favorites/{favorite_id}", server.base))
            .bearer_auth(&owner_token)
            .json(&serde_json::json!({ "version": 1, "title": "过期", "content": "覆盖" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        client
            .put(format!("{}/api/favorites/{favorite_id}", server.base))
            .bearer_auth(&outsider_token)
            .json(&serde_json::json!({ "version": 2, "title": "越权", "content": "越权" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .post(format!(
                "{}/api/favorites/{favorite_id}/collaborators",
                server.base
            ))
            .bearer_auth(&owner_token)
            .json(&serde_json::json!({ "user_id": outsider_id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        client
            .delete(format!("{}/api/favorites/{favorite_id}", server.base))
            .bearer_auth(&editor_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    let collaborators: Vec<serde_json::Value> = client
        .get(format!(
            "{}/api/favorites/{favorite_id}/collaborators",
            server.base
        ))
        .bearer_auth(&editor_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(collaborators.len(), 1);
    assert_eq!(collaborators[0]["user_id"], editor_id.to_string());

    assert_eq!(
        client
            .delete(format!(
                "{}/api/favorites/{favorite_id}/collaborators/{editor_id}",
                server.base
            ))
            .bearer_auth(&editor_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let after_leave: Vec<serde_json::Value> = client
        .get(format!("{}/api/favorites", server.base))
        .bearer_auth(&editor_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(after_leave.is_empty());
}
