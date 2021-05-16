use common::spawn_app;

mod common;

#[actix_rt::test]
async fn subscribe_returns_200_for_valid_form() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();
    let body = "name=joseph&email=jchevertonwynne%40gmail.com";

    let response = client
        .post(format!("{}/subscriptions", test_app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("failed to execute request");

    assert_eq!(response.status().as_u16(), 200);
    let saved = sqlx::query!("select email, name from subscriptions")
        .fetch_one(&test_app.db_pool)
        .await
        .expect("failed to execute query");
    assert_eq!(saved.email, "jchevertonwynne@gmail.com");
    assert_eq!(saved.name, "joseph");
}

#[actix_rt::test]
async fn subscribe_returns_400_for_invalid_form() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    let test_cases = [
        ("name=joseph", "missing email"),
        ("email=jchevertonwynne%40gmail.com", "missing name"),
        ("", "missing both params"),
    ];

    for &(body, err_msg) in test_cases.iter() {
        let response = client
            .post(format!("{}/subscriptions", test_app.address))
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
