use reqwest::Client;

use crate::domain::SubscriberEmail;

pub struct EmailClient {
    client: Client,
    base_url: String,
    sender: SubscriberEmail,
    auth_token: String,
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail, auth_token: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();
        Self {
            client,
            base_url,
            sender,
            auth_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let dest_url = format!("{}/email", self.base_url);
        let request_body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject,
            html_body: html_content,
            text_body: text_content,
        };
        tracing::info!("sending email request to {} with auth token of {}", dest_url, self.auth_token);
        self.client
            .post(&dest_url)
            .header("X-Postmark-Server-Token", &self.auth_token)
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use claim::{assert_err, assert_ok};
    use fake::{
        faker::{internet::en::SafeEmail, lorem::en::Sentence},
        Fake, Faker,
    };
    use wiremock::{
        matchers::{any, header, header_exists, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::domain::SubscriberEmail;

    use super::EmailClient;

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &wiremock::Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);

            if let Ok(body) = result {
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            } else {
                false
            }
        }
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn content() -> String {
        Sentence(1..10).fake()
    }

    fn email() -> SubscriberEmail {
        SafeEmail().fake::<String>().try_into().unwrap()
    }

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        let mock_server = MockServer::start().await;
        let email_client = EmailClient::new(mock_server.uri(), email(), Faker.fake());

        Mock::given(header_exists("X-Postmark-Server-Token"))
            .and(header("Content-Type", "application/json"))
            .and(path("/email"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_server_returns_500() {
        let mock_server = MockServer::start().await;
        let email_client = EmailClient::new(mock_server.uri(), email(), Faker.fake());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome);
    }
}
