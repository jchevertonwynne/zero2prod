use actix_http::StatusCode;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::common::spawn_app;

#[actix_rt::test]
async fn subscribe_returns_200_for_valid_form_and_sends_email() {
    let test_app = spawn_app().await;
    let body = "name=joseph&email=jchevertonwynne%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let response = test_app.post_subscriptions(body.to_string()).await;

    assert_eq!(response.status(), StatusCode::OK);
    let saved = sqlx::query!("select email, name, status from subscriptions")
        .fetch_one(&test_app.db_pool)
        .await
        .expect("failed to execute query");

    assert_eq!(saved.email, "jchevertonwynne@gmail.com");
    assert_eq!(saved.name, "joseph");
    assert_eq!(saved.status, "pending");

    let email_request = &test_app.email_server.received_requests().await.unwrap()[0];
    let links = test_app.get_confirmation_links(email_request);

    assert_eq!(links.html, links.plain);
}

#[actix_rt::test]
async fn subscribe_returns_400_for_invalid_form() {
    let test_app = spawn_app().await;

    let test_cases = [
        ("name=joseph", "missing email"),
        ("email=jchevertonwynne%40gmail.com", "missing name"),
        ("", "missing both params"),
    ];

    for (body, description) in test_cases {
        let response = test_app.post_subscriptions(body.to_string()).await;

        assert_eq!(
            response.status().as_u16(),
            400,
            "the api did not return a 400 when {}",
            description
        );
    }
}

#[actix_rt::test]
async fn subscribe_returns_400_when_fields_are_present_but_empty() {
    let test_app = spawn_app().await;

    let test_cases = [
        ("name=&email=j@gmail.com", "empty name"),
        ("name=joseph&email=", "empty email"),
        ("name=joseph&email=yolo", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = test_app.post_subscriptions(body.to_string()).await;

        assert_eq!(
            response.status().as_u16(),
            400,
            "the api did not return a 200 OK when the payload was {}",
            description
        );
    }
}

#[actix_rt::test]
async fn confirmation_requests_without_token_are_rejected() {
    let test_app = spawn_app().await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", test_app.address))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST)
}

#[actix_rt::test]
async fn confirmation_requests_with_invalid_token_are_refused() {
    let test_app = spawn_app().await;

    let response = reqwest::get(&format!(
        "{}/subscriptions/confirm?subscription_token=hello",
        test_app.address
    ))
    .await
    .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST)
}

#[actix_rt::test]
async fn confirmation_requests_with_valid_token_are_accepted_and_user_marked_as_confirmed() {
    let test_app = spawn_app().await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&test_app.email_server)
        .await;

    let response = test_app
        .post_subscriptions("name=joseph&email=jchevertonwynne1%40gmail.com".to_string())
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let confirmation_status = sqlx::query!(
        "SELECT status FROM subscriptions WHERE email = $1",
        "jchevertonwynne1@gmail.com"
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("query failed");
    assert_eq!(confirmation_status.status, "pending");

    let requests = test_app.email_server.received_requests().await.unwrap();
    let links = test_app.get_confirmation_links(&requests[0]);
    assert_eq!(links.html.host_str().unwrap(), "127.0.0.1");

    let response = reqwest::get(links.html.as_ref()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let confirmation_status = sqlx::query!(
        "SELECT status FROM subscriptions WHERE email = $1",
        "jchevertonwynne1@gmail.com"
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("query failed");
    assert_eq!(confirmation_status.status, "confirmed");
}
