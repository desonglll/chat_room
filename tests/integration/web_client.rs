use super::*;

#[tokio::test]
async fn web_client_is_only_served_when_enabled() {
    let api_only = start_server().await;
    assert_eq!(
        reqwest::get(format!("{}/", api_only))
            .await
            .unwrap()
            .status(),
        404
    );

    let web = start_web_server().await;
    let response = reqwest::get(format!("{}/", web)).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .unwrap(),
        "text/html; charset=utf-8"
    );
    let html = response.text().await.unwrap();
    assert!(html.contains("<div id=\"root\"></div>"));
    assert!(html.contains("type=\"module\""));

    // The entry is hash-named by vite, so read it out of the shell instead of pinning it.
    let entry = html
        .split("src=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("index.html references a script entry");
    assert!(entry.starts_with("/assets/"));
    assert!(entry.ends_with(".js"));

    let script = reqwest::get(format!("{}{}", web, entry)).await.unwrap();
    assert_eq!(script.status(), 200);
    assert_eq!(
        script.headers()[reqwest::header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    let script = script.text().await.unwrap();
    assert!(script.contains("createRoot"));
    assert!(!script.contains("primevue"));

    // The Vue entry must stay gone: a browser still holding the old service worker
    // resolves it, gets 404, and unpins itself via the retired worker below.
    let vue_entry = reqwest::get(format!("{}/assets/app.js", web)).await.unwrap();
    assert_eq!(vue_entry.status(), 404);

    let missing_asset = reqwest::get(format!("{}/assets/not-built.js", web))
        .await
        .unwrap();
    assert_eq!(missing_asset.status(), 404);

    let manifest = reqwest::get(format!("{}/manifest.webmanifest", web))
        .await
        .unwrap();
    assert_eq!(manifest.status(), 200);
    assert_eq!(
        manifest.headers()[reqwest::header::CONTENT_TYPE],
        "application/manifest+json; charset=utf-8"
    );
    let manifest: serde_json::Value = manifest.json().await.unwrap();
    assert_eq!(manifest["display"], "standalone");
    assert_eq!(manifest["icons"][0]["src"], "/pwa-192.png");

    // Until packages/web ships its own worker, /sw.js is the generated retiring worker:
    // it may reference no asset, no API path, and must unregister itself.
    let worker = reqwest::get(format!("{}/sw.js", web)).await.unwrap();
    assert_eq!(worker.status(), 200);
    assert_eq!(worker.headers()[reqwest::header::CACHE_CONTROL], "no-cache");
    assert_eq!(worker.headers()["service-worker-allowed"], "/");
    let worker = worker.text().await.unwrap();
    assert!(worker.contains("unregister"));
    assert!(!worker.contains("/assets/"));
    assert!(!worker.contains("/api/"));
    assert!(!worker.contains("/ws"));

    let pwa_icon = reqwest::get(format!("{}/pwa-192.png", web)).await.unwrap();
    assert_eq!(pwa_icon.status(), 200);
    assert_eq!(
        pwa_icon.headers()[reqwest::header::CONTENT_TYPE],
        "image/png"
    );

    let theme_bootstrap = reqwest::get(format!("{}/theme-bootstrap.js", web))
        .await
        .unwrap();
    assert_eq!(theme_bootstrap.status(), 200);
    assert_eq!(
        theme_bootstrap.headers()[reqwest::header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert!(theme_bootstrap
        .text()
        .await
        .unwrap()
        .contains("localStorage"));

    let favicon = reqwest::get(format!("{}/favicon.svg", web)).await.unwrap();
    assert_eq!(favicon.status(), 200);
    assert_eq!(
        favicon.headers()[reqwest::header::CONTENT_TYPE],
        "image/svg+xml"
    );
}
