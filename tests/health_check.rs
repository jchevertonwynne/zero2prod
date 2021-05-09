use std::net::TcpListener;

#[actix_rt::test]
async fn health_check_works() {
    let address = spawn_app();
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/health_check", address))
        .send()
        .await
        .expect("failed to execute request");

    assert!(response.status().is_success());
    assert_eq!(response.content_length(), Some(0));
}

#[actix_rt::test]
async fn subscribe_returns_200_for_valid_form() {
    let address = spawn_app();
    let client = reqwest::Client::new();
    let body = "name=joseph&email=jchevertonwynne%40gmail.com";

    let response = client
        .post(format!("{}/subscriptions", address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("failed to execute request");

    assert_eq!(response.status().as_u16(), 200);
}

#[actix_rt::test]
async fn subscribe_returns_400_for_invalid_form() {
    let address = spawn_app();
    let client = reqwest::Client::new();

    let test_cases = [
        ("name=joseph", "missing email"),
        ("email=jchevertonwynne%40gmail.com", "missing name"),
        ("", "missing both params"),
    ];

    for &(body, err_msg) in test_cases.iter() {
        let response = client
            .post(format!("{}/subscriptions", address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("failed to execute request");

        assert_eq!(
            response.status().as_u16(),
            400,
            "the api did not return a 400 when {}",
            err_msg
        );
    }
}

fn spawn_app() -> String {
    let listener = TcpListener::bind("localhost:0").expect("failed to find random port");
    let port = listener.local_addr().expect("should have port").port();
    let server = zero2prod::startup::run_on(listener).expect("failed to bind address");
    let _ = tokio::spawn(server);
    format!("http://localhost:{}", port)
}
