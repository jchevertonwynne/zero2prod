use actix_http::StatusCode;
use wiremock::{
    matchers::{any, method, path},
    Mock, ResponseTemplate,
};

use crate::common::{spawn_app, ConfirmationLinks, TestApp};

#[actix_rt::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "newsletter title",
        "content": {
            "text": "plain text body",
            "html": "<b>html body</b>"
        }
    });

    let response = app.post_newsletter(newsletter_request_body).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_rt::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "newsletter title",
        "content": {
            "text": "plain text body",
            "html": "<b>html body</b>"
        }
    });

    let response = app.post_newsletter(newsletter_request_body).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_rt::test]
async fn newsletters_returns_400_for_invalid_data() {
    let app = spawn_app().await;
    let test_cases = [
        (
            serde_json::json!({
                "contents": {
                    "text": "text cont",
                    "html": "<b>html cont</b>"
                }
            }),
            "missing title",
        ),
        (
            serde_json::json!({
                "title": "newsletter"
            }),
            "missing contents",
        ),
    ];

    for (body, error_message) in test_cases {
        let response = app.post_newsletter(body).await;

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "the api didn't 400 for a json body that was {}",
            error_message
        );
    }
}

#[actix_rt::test]
async fn request_missing_auth_are_rejected() {
    let app = spawn_app().await;

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletter", &app.address))
        .json(&serde_json::json!({
            "title": "some title",
            "content": {
                "text": "text cont",
                "html": "<b>html cont</b>"
            }
        }))
        .send()
        .await
        .expect("failed to execute request");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.headers()["WWW-Authenticate"],
        r#"Basic realm="publish""#
    );
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=joseph&email=joseph@google.com";

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;
    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
