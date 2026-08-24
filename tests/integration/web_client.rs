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
    #[cfg(feature = "react")]
    assert!(html.contains("<div id=\"root\"></div>"));
    #[cfg(not(feature = "react"))]
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

    #[cfg(feature = "react")]
    {
        assert!(!script.is_empty());
        let api_chunk = reqwest::get(format!("{}/assets/api.js", web))
            .await
            .unwrap();
        assert_eq!(api_chunk.status(), 200);
        let api_chunk = api_chunk.text().await.unwrap();
        assert!(api_chunk.contains("/rooms"));
        assert!(api_chunk.contains("/admin/overview"));

        let chat_chunk = reqwest::get(format!("{}/assets/ChatPage.js", web))
            .await
            .unwrap();
        assert_eq!(chat_chunk.status(), 200);
        assert!(chat_chunk.text().await.unwrap().contains("WebSocket"));
    }
    #[cfg(not(feature = "react"))]
    {
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
        let admin_dashboard = admin_dashboard.text().await.unwrap();
        assert!(
            script.contains("/api/admin/overview")
                || admin_dashboard.contains("/api/admin/overview")
        );

        let lazy_dialog = reqwest::get(format!("{}/assets/AuthDialog.js", web))
            .await
            .unwrap();
        assert_eq!(lazy_dialog.status(), 200);
        assert_eq!(
            lazy_dialog.headers()[reqwest::header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
    }

    let missing_asset = reqwest::get(format!("{}/assets/not-built.js", web))
        .await
        .unwrap();
    assert_eq!(missing_asset.status(), 404);

    #[cfg(not(feature = "react"))]
    {
        let archive_chunk = reqwest::get(format!("{}/assets/jszip.min.js", web))
            .await
            .unwrap();
        assert_eq!(archive_chunk.status(), 200);
        assert_eq!(
            archive_chunk.headers()[reqwest::header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
    }

    let css = reqwest::get(format!("{}/assets/app.css", web))
        .await
        .unwrap();
    assert_eq!(css.status(), 200);
    assert_eq!(
        css.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),
        "text/css; charset=utf-8"
    );
    #[cfg(feature = "react")]
    assert!(!css.text().await.unwrap().is_empty());
    #[cfg(not(feature = "react"))]
    assert!(css.text().await.unwrap().contains("--p-primary-color"));
}
