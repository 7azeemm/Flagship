use crate::core::product::{CachedProductInfo, Product};
use crate::core::sections::Section;
use chrono::{Duration, Utc};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Sqlite, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::Duration as StdDuration;

const DEFAULT_DATABASE_URL: &str = "sqlite://state/mythos.db";
const CACHE_EXPIRATION_DURATION: Duration = Duration::days(14);
static DATABASE: OnceLock<SqlitePool> = OnceLock::new();

pub struct Database;

impl Database {
    pub async fn init() -> Result<(), String> {
        let database_url = DEFAULT_DATABASE_URL;
        ensure_parent_directory(&database_url).await?;

        let options = SqliteConnectOptions::from_str(&database_url)
            .map_err(|error| format!("Invalid DATABASE_URL: {error}"))?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(StdDuration::from_secs(10));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await
            .map_err(|error| format!("Failed to open SQLite database: {error}"))?;

        sqlx::migrate!().run(&pool).await.map_err(|error| format!("Failed to apply SQLite schema migrations: {error}"))?;
        DATABASE.set(pool).map_err(|_| "Database was initialized twice".to_string())?;
        Ok(())
    }

    pub async fn load_products() -> Result<(HashMap<String, Product>, Vec<Product>), String> {
        let rows: Vec<(bool, String)> = sqlx::query_as("SELECT removed, payload FROM products").fetch_all(pool()?).await.map_err(db_error)?;
        let mut products = HashMap::new();
        let mut removed_products = Vec::new();
        for (removed, payload) in rows {
            let product: Product = serde_json::from_str(&payload).map_err(json_error)?;
            if removed {
                removed_products.push(product);
            } else {
                products.insert(product.id.clone(), product);
            }
        }
        Ok((products, removed_products))
    }

    pub async fn save_products(products: &HashMap<String, Product>, sections: &[Section], removed_products: Option<&[Product]>) -> Result<(), String> {
        let mut transaction = pool()?.begin().await.map_err(db_error)?;
        for section in sections.iter().copied().collect::<HashSet<_>>() {
            sqlx::query("DELETE FROM products WHERE removed = 0 AND section = ?")
                .bind(section.to_string())
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }

        let selected = products.values().filter(|product| sections.contains(&product.section));
        for product in selected {
            upsert_product(&mut transaction, product, false).await?;
        }

        if let Some(removed_products) = removed_products {
            sqlx::query("DELETE FROM products WHERE removed = 1").execute(&mut *transaction).await.map_err(db_error)?;
            for product in removed_products {
                upsert_product(&mut transaction, product, true).await?;
            }
        }
        transaction.commit().await.map_err(db_error)
    }

    pub async fn load_variants<T: DeserializeOwned + Default>() -> Result<T, String> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, payload FROM variants").fetch_all(pool()?).await.map_err(db_error)?;
        if rows.is_empty() {
            return Ok(T::default());
        }
        let variants = rows
            .into_iter()
            .map(|(id, payload)| Ok((id, serde_json::from_str(&payload).map_err(json_error)?)))
            .collect::<Result<serde_json::Map<String, serde_json::Value>, String>>()?;
        serde_json::from_value(serde_json::json!({ "variants": variants })).map_err(json_error)
    }

    pub async fn save_variants<'a, I, V>(variants: I) -> Result<(), String>
    where
        I: IntoIterator<Item = (&'a String, &'a V)>,
        V: Serialize + 'a,
    {
        let mut transaction = pool()?.begin().await.map_err(db_error)?;
        sqlx::query("DELETE FROM variants").execute(&mut *transaction).await.map_err(db_error)?;
        for (id, variant) in variants {
            let payload = serde_json::to_string(variant).map_err(json_error)?;
            sqlx::query("INSERT INTO variants (id, payload) VALUES (?, ?)")
                .bind(id)
                .bind(payload)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        transaction.commit().await.map_err(db_error)
    }

    pub async fn load_product_info_cache() -> Result<HashMap<String, CachedProductInfo>, String> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("DELETE FROM product_info_cache WHERE expires_at < ?").bind(&now).execute(pool()?).await.map_err(db_error)?;
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT url, payload FROM product_info_cache").fetch_all(pool()?).await.map_err(db_error)?;
        rows.into_iter().map(|(url, payload)| Ok((url, serde_json::from_str(&payload).map_err(json_error)?))).collect()
    }

    pub async fn save_product_info_cache(cache: &HashMap<String, CachedProductInfo>) -> Result<(), String> {
        let mut transaction = pool()?.begin().await.map_err(db_error)?;
        sqlx::query("DELETE FROM product_info_cache").execute(&mut *transaction).await.map_err(db_error)?;
        for (url, info) in cache {
            let expires_at = (info.timestamp + CACHE_EXPIRATION_DURATION).to_rfc3339();
            let payload = serde_json::to_string(info).map_err(json_error)?;
            sqlx::query("INSERT INTO product_info_cache (url, expires_at, payload) VALUES (?, ?, ?)")
                .bind(url)
                .bind(expires_at)
                .bind(payload)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        transaction.commit().await.map_err(db_error)
    }

    pub async fn load_state<T: DeserializeOwned + Default>(key: &str) -> Result<T, String> {
        let payload: Option<String> = sqlx::query_scalar("SELECT payload FROM app_state WHERE key = ?").bind(key).fetch_optional(pool()?).await.map_err(db_error)?;
        payload.map_or_else(|| Ok(T::default()), |payload| serde_json::from_str(&payload).map_err(json_error))
    }

    pub async fn save_state<T: Serialize>(key: &str, value: &T) -> Result<(), String> {
        let payload = serde_json::to_string(value).map_err(json_error)?;
        sqlx::query(
            "INSERT INTO app_state (key, payload) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET payload = excluded.payload",
        )
        .bind(key)
        .bind(payload)
        .execute(pool()?)
        .await
        .map_err(db_error)?;
        Ok(())
    }
}

fn pool() -> Result<&'static SqlitePool, String> {
    DATABASE.get().ok_or_else(|| "Database has not been initialized".to_string())
}

async fn upsert_product(transaction: &mut sqlx::Transaction<'_, Sqlite>, product: &Product, removed: bool) -> Result<(), String> {
    let payload = serde_json::to_string(product).map_err(json_error)?;
    sqlx::query(
        "INSERT INTO products (id, section, retailer, url, price, status, removed, payload) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET section = excluded.section, retailer = excluded.retailer, \
         url = excluded.url, price = excluded.price, status = excluded.status, \
         removed = excluded.removed, payload = excluded.payload",
    )
    .bind(&product.id)
    .bind(product.section.to_string())
    .bind(&product.retailer)
    .bind(&product.url)
    .bind(product.price)
    .bind(product.status.to_string())
    .bind(removed)
    .bind(payload)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;
    Ok(())
}

async fn ensure_parent_directory(database_url: &str) -> Result<(), String> {
    let Some(path) = database_url.strip_prefix("sqlite://") else { return Ok(()) };
    if path == ":memory:" { return Ok(()); }
    if let Some(parent) = Path::new(path).parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| format!("Failed to create database directory: {error}"))?;
    }
    Ok(())
}

fn db_error(error: sqlx::Error) -> String {
    format!("SQLite operation failed: {error}")
}

fn json_error(error: serde_json::Error) -> String {
    format!("Failed to serialize database value: {error}")
}
