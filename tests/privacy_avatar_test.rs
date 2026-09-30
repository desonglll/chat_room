//! TG-505: the anonymous avatar request. `<img>` sends no bearer, so an anonymous download
//! must be refused whenever any signed-in viewer could be — otherwise dropping one's
//! credentials would bypass a deny exception, a block, or a contacts/nobody tier.

mod privacy_support;

use privacy_support::start_server;
use reqwest::StatusCode;

#[tokio::test]
async fn anonymous_avatar_requests_are_admitted_only_when_nobody_could_be_refused() {
    let server = start_server().await;
    let owner = server.account("av-owner").await;
    let someone = server.account("av-someone").await;
    let url = server.upload_avatar(&owner).await;
    let anonymous = || async {
        let response = server.client.get(server.url(&url)).send().await.unwrap();
        (
            response.status(),
            response
                .headers()
                .get("cache-control")
                .map(|value| value.to_str().unwrap().to_string()),
        )
    };
    let (status, cache) = anonymous().await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("public, max-age=300"));

    server
        .put_rule(&owner, "profile_photo", "everybody", &[], &[&someone])
        .await;
    assert_eq!(anonymous().await.0, StatusCode::NOT_FOUND);
    let authed = server
        .client
        .get(server.url(&url))
        .bearer_auth(&owner.token)
        .send()
        .await
        .unwrap();
    assert_eq!(authed.status(), StatusCode::OK);
    assert_eq!(
        authed.headers()["cache-control"].to_str().unwrap(),
        "private, max-age=300"
    );

    server
        .put_rule(&owner, "profile_photo", "contacts", &[], &[])
        .await;
    assert_eq!(anonymous().await.0, StatusCode::NOT_FOUND);
    server
        .put_rule(&owner, "profile_photo", "everybody", &[], &[])
        .await;
    assert_eq!(anonymous().await.0, StatusCode::OK);
    server.block(&owner, &someone).await;
    assert_eq!(anonymous().await.0, StatusCode::NOT_FOUND);
}
