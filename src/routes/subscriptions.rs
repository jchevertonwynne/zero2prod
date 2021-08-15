use std::{
    convert::TryInto,
    fmt::{Debug, Display},
};

use actix_http::StatusCode;
use anyhow::Context;
use chrono::Utc;

use actix_web::{web, HttpResponse, ResponseError};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{domain::NewSubscriber, email_client::EmailClient, startup::ApplicationBaseUrl};

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

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

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> actix_http::StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub struct StoreTokenError(sqlx::Error);

impl Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error occured whilst attempting to store a subscription token"
        )
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
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
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("failed to retrieve connection from pool")?;

    let user_id = insert_new_user(&new_subscriber, &mut transaction)
        .await
        .context("failed to insert new user")?;

    let confirmation_token = add_subscription_token(user_id, &mut transaction)
        .await
        .context("failed to insert confirmation token")?;

    send_confirmation_email(
        &email_client,
        new_subscriber,
        base_url.as_ref(),
        confirmation_token,
    )
    .await
    .context("failed to send confirmaiton email")?;

    transaction
        .commit()
        .await
        .context("failed to complete transaction")?;

    Ok(HttpResponse::Ok().finish())
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
) -> Result<SubscriptionToken, StoreTokenError> {
    let token = generate_subscription_token();
    sqlx::query!(
        "INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)",
        token.0,
        subscriber_id
    )
    .execute(transaction)
    .await
    .map_err(|err| StoreTokenError(err))?;
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

#[derive(serde::Deserialize, Debug)]
pub struct ConfirmRegistrationParams {
    subscription_token: String,
}

#[derive(thiserror::Error)]
pub enum ConfirmationError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for ConfirmationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ConfirmationError {
    fn status_code(&self) -> StatusCode {
        match self {
            ConfirmationError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ConfirmationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[allow(clippy::async_yields_async)]
#[tracing::instrument(name = "confirming registration", skip(params, pool))]
pub async fn confirm_registration(
    params: web::Query<ConfirmRegistrationParams>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ConfirmationError> {
    let mut transaction = pool.begin().await.context("failed to create transaction")?;

    let user_id = find_user_id_by_confirmation_token(&params, &mut transaction)
        .await
        .context("failed to query registration token")?
        .ok_or_else(|| {
            ConfirmationError::ValidationError("failed to find token in database".to_string())
        })?;

    update_user_status_to_confirmed(user_id, &mut transaction)
        .await
        .context("failed to update user's status to confirmed")?;

    delete_used_subscription_token(&params, &mut transaction)
        .await
        .context("failed to remove used confirmation token")?;

    transaction
        .commit()
        .await
        .context("failed to complete transaction")?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "finding user id for confirmation token",
    skip(transaction)
)]
async fn find_user_id_by_confirmation_token(
    params: &web::Query<ConfirmRegistrationParams>,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query!(
        "SELECT subscriber_id from subscription_tokens WHERE subscription_token = $1",
        params.0.subscription_token
    )
    .fetch_optional(transaction)
    .await
    .map(|r| r.map(|v| v.subscriber_id))
}

#[tracing::instrument(
    name = "updating user id to confirmed",
    skip(transaction)
)]
async fn update_user_status_to_confirmed(
    user_id: Uuid,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE subscriptions SET status = 'confirmed' WHERE id = $1",
        user_id
    )
    .execute(transaction)
    .await
    .map(|_| ())
}

#[tracing::instrument(
    name = "deleting used confirmation token",
    skip(transaction)
)]
async fn delete_used_subscription_token(
    params: &web::Query<ConfirmRegistrationParams>,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM subscription_tokens WHERE subscription_token = $1",
        params.0.subscription_token
    )
    .execute(transaction)
    .await
    .map(|_| ())
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
