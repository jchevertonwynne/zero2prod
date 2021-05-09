mod common;

#[actix_rt::test]
async fn health_check_works() {
    let test_app = common::spawn_app().await;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/health_check", test_app.address))
        .send()
        .await
        .expect("failed to execute request");

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.content_length(), Some(0));
}
