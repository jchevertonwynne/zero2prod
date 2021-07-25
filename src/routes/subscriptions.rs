use std::convert::TryInto;

use chrono::Utc;

use actix_web::{web, HttpResponse, Responder};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::NewSubscriber;

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

#[tracing::instrument(
    name="Adding a new subscriber",
    skip(form, pool),
    fields(
        user_email = %form.email,
        user_name = %form.name
    )
)]
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> impl Responder {
    let new_subscriber = match form.0.try_into() {
        Ok(sub) => sub,
        Err(err) => {
            tracing::error!("failed to validate form: {}", err);
            return HttpResponse::BadRequest().finish();
        }
    };

    match insert_new_user(&new_subscriber, pool.get_ref()).await {
        Ok(_) => {
            tracing::info!("new subscriber details have been saved");
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            tracing::error!("failed to execute query: {:?}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(name = "saving new subscriber to the database", skip(subscriber, pool))]
async fn insert_new_user(subscriber: &NewSubscriber, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        subscriber.email.as_ref(),
        subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(pool)
    .await
    .map_err(|err| {
        tracing::error!("failed to execute query: {:?}", err);
        err
    })?;
    Ok(())
}
