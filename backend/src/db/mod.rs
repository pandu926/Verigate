pub mod assessments;
pub mod audit_events;
pub mod cases;
pub mod disclosed_facts;
pub mod submissions;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Connect to PostgreSQL and run pending migrations.
pub async fn connect_db(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
