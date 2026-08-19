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
    assert!(html.contains("<div id=\"app\"></div>"));
    assert!(html.contains("/assets/app.css"));
    assert!(html.contains("/assets/app.js"));

    let favicon = reqwest::get(format!("{}/favicon.svg", web)).await.unwrap();
    assert_eq!(favicon.status(), 200);
    assert_eq!(
        favicon.headers()[reqwest::header::CONTENT_TYPE],
        "image/svg+xml"
    );

    let script = reqwest::get(format!("{}/assets/app.js", web))
        .await
        .unwrap();
    assert_eq!(script.status(), 200);
    let script = script.text().await.unwrap();
    assert!(script.contains("/api/rooms"));
    assert!(script.contains("WebSocket"));

    let admin_dashboard = reqwest::get(format!("{}/assets/AdminDashboard.js", web))
        .await
        .unwrap();
    assert_eq!(admin_dashboard.status(), 200);
    assert_eq!(
        admin_dashboard.headers()[reqwest::header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert!(admin_dashboard
        .text()
        .await
        .unwrap()
        .contains("/api/admin/overview"));

    let archive_chunk = reqwest::get(format!("{}/assets/jszip.min.js", web))
        .await
        .unwrap();
    assert_eq!(archive_chunk.status(), 200);
    assert_eq!(
        archive_chunk.headers()[reqwest::header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );

    let css = reqwest::get(format!("{}/assets/app.css", web))
        .await
        .unwrap();
    assert_eq!(css.status(), 200);
    assert_eq!(
        css.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),
        "text/css; charset=utf-8"
    );
    assert!(css.text().await.unwrap().contains("--p-primary-color"));
}
