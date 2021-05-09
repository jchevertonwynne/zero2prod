use chrono::Utc;

use actix_web::{HttpResponse, Responder, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
) -> impl Responder {
    match sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pool.get_ref())
    .await {
        Ok(_) => return HttpResponse::Ok(),
        Err(err) => {
            eprintln!("failed to execute query: {:?}", err);
            return HttpResponse::InternalServerError()
        }
    }
}
