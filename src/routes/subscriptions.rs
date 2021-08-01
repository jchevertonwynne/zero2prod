use std::convert::TryInto;

use chrono::Utc;

use actix_web::{web, HttpResponse, Responder};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{domain::NewSubscriber, email_client::EmailClient, startup::ApplicationBaseUrl};

#[derive(serde::Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

impl TryInto<NewSubscriber> for FormData {
    type Error = String;

    fn try_into(self) -> Result<NewSubscriber, Self::Error> {
        let email = self.email.try_into()?;
        let name = self.name.try_into()?;
        Ok(NewSubscriber { email, name })
    }
}

#[allow(clippy::async_yields_async)]
#[tracing::instrument(
    name="Adding a new subscriber",
    skip(form, pool, email_client, base_url),
    fields(
        user_email = %form.email,
        user_name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> impl Responder {
    let new_subscriber = match form.0.try_into() {
        Ok(sub) => sub,
        Err(err) => {
            tracing::error!("failed to validate form: {}", err);
            return HttpResponse::BadRequest();
        }
    };

    let mut transaction = match pool.begin().await {
        Ok(transaction) => transaction,
        Err(err) => {
            tracing::info!("failed to create transaction: {}", err);
            return HttpResponse::InternalServerError();
        }
    };

    let user_id = match insert_new_user(&new_subscriber, &mut transaction).await {
        Ok(confirmation_token) => {
            tracing::info!("new subscriber details have been saved");
            confirmation_token
        }
        Err(err) => {
            tracing::error!("failed to insert user: {:?}", err);
            return HttpResponse::InternalServerError();
        }
    };

    let confirmation_token = match add_subscription_token(user_id, &mut transaction).await {
        Ok(token) => token,
        Err(err) => {
            tracing::error!("failed to create subscription token: {:?}", err);
            return HttpResponse::InternalServerError();
        }
    };

    if let Err(err) = send_confirmation_email(
        &email_client,
        new_subscriber,
        base_url.as_ref(),
        confirmation_token,
    )
    .await
    {
        tracing::error!("failed to send email: {:?}", err);
        return HttpResponse::InternalServerError();
    }

    if let Err(err) = transaction.commit().await {
        tracing::info!("failed to complete transaction: {}", err);
        return HttpResponse::InternalServerError();
    };

    HttpResponse::Ok()
}

#[tracing::instrument(
    name = "saving new subscriber to the database",
    skip(subscriber, transaction)
)]
async fn insert_new_user(
    subscriber: &NewSubscriber,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    let uuid = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending')
        "#,
        uuid,
        subscriber.email.as_ref(),
        subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(transaction)
    .await?;
    Ok(uuid)
}

#[tracing::instrument(
    name = "adding subscription token to the database",
    skip(subscriber_id, transaction)
)]
async fn add_subscription_token(
    subscriber_id: Uuid,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<SubscriptionToken, sqlx::Error> {
    let token = generate_subscription_token();
    sqlx::query!(
        "INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)",
        token.0,
        subscriber_id
    )
    .execute(transaction)
    .await?;
    Ok(token)
}

#[tracing::instrument(
    name = "send a confirmation email to the subscriber",
    skip(email_client, subscriber, base_url, confirmation_token)
)]
async fn send_confirmation_email(
    email_client: &EmailClient,
    subscriber: NewSubscriber,
    base_url: &ApplicationBaseUrl,
    confirmation_token: SubscriptionToken,
) -> Result<(), reqwest::Error> {
    let url = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url.0, confirmation_token.0
    );
    email_client
        .send_email(
            subscriber.email,
            "Welcome!",
            &format!("Welcome to the newsletter! <br> Click <a href=\"{}\">here</a> to confirm your subscription", url),
            &format!("Welcome to the newsletter!\nVisit {} to confirm your subscription", url),
        )
        .await
}

#[derive(serde::Deserialize)]
pub struct ConfirmRegistrationParams {
    subscription_token: String,
}

#[tracing::instrument(name = "confirming registration", skip(params, pool))]
pub async fn confirm_registration(
    params: web::Query<ConfirmRegistrationParams>,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let mut transaction = match pool.begin().await {
        Ok(transaction) => transaction,
        Err(err) => {
            tracing::info!("failed to create transaction: {}", err);
            return HttpResponse::InternalServerError();
        }
    };
    let response = sqlx::query!(
        "SELECT subscriber_id from subscription_tokens WHERE subscription_token = $1",
        params.0.subscription_token
    )
    .fetch_optional(&mut transaction)
    .await;

    let record = match response {
        Ok(record) => record,
        Err(err) => {
            tracing::info!("failed to query registration token: {}", err);
            return HttpResponse::InternalServerError();
        }
    };

    let record = match record {
        Some(record) => record,
        None => {
            tracing::info!(
                "failed to find registration token: {}",
                params.0.subscription_token
            );
            return HttpResponse::BadRequest();
        }
    };

    if let Err(err) = sqlx::query!(
        "UPDATE subscriptions SET status = 'confirmed' WHERE id = $1",
        record.subscriber_id
    )
    .execute(&mut transaction)
    .await
    {
        tracing::info!("failed to update user status: {}", err);
        return HttpResponse::InternalServerError();
    };

    if let Err(err) = sqlx::query!(
        "DELETE FROM subscription_tokens WHERE subscription_token = $1",
        params.0.subscription_token
    )
    .execute(&mut transaction)
    .await
    {
        tracing::info!("failed to remove used subscription token: {}", err);
        return HttpResponse::InternalServerError();
    };

    if let Err(err) = transaction.commit().await {
        tracing::info!("failed to complete transaction: {}", err);
        return HttpResponse::InternalServerError();
    };

    HttpResponse::Ok()
}

struct SubscriptionToken(String);

fn generate_subscription_token() -> SubscriptionToken {
    let mut rng = thread_rng();
    let inner = std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect();
    SubscriptionToken(inner)
}
