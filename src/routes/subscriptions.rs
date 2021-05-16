use chrono::Utc;

use actix_web::{web, HttpResponse, Responder};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

#[tracing::instrument(name="Adding a new subscriber", skip(form, pool), fields(email = %form.email, user_name = %form.name))]
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> impl Responder {
    match insert_new_user(&form, pool.get_ref()).await {
        Ok(_) => {
            tracing::info!("new subscriber details have been saved");
            HttpResponse::Ok()
        }
        Err(err) => {
            tracing::error!("failed to execute query: {:?}", err);
            HttpResponse::InternalServerError()
        }
    }
}

#[tracing::instrument(name = "saving new subscriber to the database", skip(form, pool))]
async fn insert_new_user(form: &FormData, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pool)
    .await?;
    Ok(())
}
