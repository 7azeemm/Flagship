use crate::core::product::{CachedProductInfo, Product};
use crate::core::storage::ProductStorage;
use crate::scraping::tracking::error_tracker::ErrorTracker;
use crate::scraping::tracking::scan_report::ScanReport;
use chrono::{Duration, Utc};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Postgres};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::LazyLock;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration as StdDuration;
use tokio::sync::Mutex;
use crate::core::catalog::overrides;

const CACHE_EXPIRATION_DURATION: Duration = Duration::days(14);
// Bulk loads build documents directly: a SQL function with its own search_path
// executes once per row and dominates large catalog reads.
pub(crate) const PRODUCT_DOCUMENT_SQL: &str = "(products.details || jsonb_build_object(\
    'id',products.id,'section',products.section,'retailer',products.retailer,\
    'url',products.url,'price',products.price,'status',products.status,\
    'title',products.title,'description',products.description,'image',products.image,\
    'components',products.components,'views',coalesce(catalog_views.views,0)))";
static DATABASE: OnceLock<PgPool> = OnceLock::new();

static ADMIN_REVISION: AtomicI64 = AtomicI64::new(-1);
static PUBLISHED_CATALOG_REVISION: AtomicI64 = AtomicI64::new(-1);
static CATALOG_REFRESH: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub struct Database;

impl Database {
    pub async fn initialize_catalog_sync() -> Result<(), String> {
        Self::refresh_catalog(Database::pool(), true).await
    }

    pub async fn refresh_catalog(pool: &PgPool, initial: bool) -> Result<(), String> {
        let _guard = CATALOG_REFRESH.lock().await;
        let current: i64 = sqlx::query_scalar("SELECT revision FROM admin_revision WHERE id=1")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
        if !initial && current == ADMIN_REVISION.load(Ordering::Acquire) {
            return Ok(());
        }
        
        // Serialize with scan commits and manual edits before locking the database.
        let _update = ProductStorage::lock_updates().await;
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        Database::lock_catalog(&mut tx).await.map_err(|e| e.to_string())?;
        
        let revision = sqlx::query_scalar("SELECT revision FROM admin_revision WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        
        let reviews = sqlx::query_as("SELECT key,reviewed,reviewed_at FROM scrape_errors")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        
        if !initial && revision != PUBLISHED_CATALOG_REVISION.load(Ordering::Acquire) {
            let (products, removed_products) = Database::load_products_in(&mut tx).await?;
            tx.commit().await.map_err(|e| e.to_string())?;
            ProductStorage::publish(ProductStorage { products, removed_products }).await;
            Self::mark_catalog_published(revision);
        } else {
            tx.commit().await.map_err(|e| e.to_string())?;
        }
        
        drop(_update);
        ErrorTracker::apply_admin_reviews(&reviews).await;
        ADMIN_REVISION.store(revision, Ordering::Release);
        Ok(())
    }

    pub async fn init() -> Result<(), String> {
        let database_url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set".to_string())?;
        let options = PgConnectOptions::from_str(&database_url)
            .map_err(|error| format!("Invalid DATABASE_URL: {error}"))?
            .application_name("flagship");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(StdDuration::from_secs(10))
            .connect_with(options)
            .await
            .map_err(|error| format!("Failed to connect to PostgreSQL: {error}"))?;

        Self::migrate(&pool).await?;
        DATABASE.set(pool).map_err(|_| "Database was initialized twice".to_string())?;
        Ok(())
    }

    async fn migrate(pool: &PgPool) -> Result<(), String> {
        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .map_err(|error| format!("Failed to apply PostgreSQL migrations: {error}. Databases created with the previous migration files require a reset: back up needed data, stop Flagship/Directus, then run start.bat reset (deletes all database data)."))
    }

    pub async fn load_scrape_errors() -> Result<std::collections::BTreeMap<String, crate::scraping::tracking::error_tracker::ErrorRecord>, String> {
        let payloads: Vec<String> = sqlx::query_scalar("SELECT (payload||jsonb_build_object('key',key,'retailer',retailer,'active',active,'reviewed',reviewed,'reviewed_at',reviewed_at,'last_seen',last_seen))::text FROM scrape_errors")
            .fetch_all(Database::pool()).await.map_err(db_error)?;
        payloads
            .into_iter()
            .map(|payload| {
                let record: crate::scraping::tracking::error_tracker::ErrorRecord = serde_json::from_str(&payload).map_err(json_error)?;
                Ok((record.key.clone(), record))
            })
            .collect()
    }

    pub async fn save_scrape_errors(records: &std::collections::BTreeMap<String, crate::scraping::tracking::error_tracker::ErrorRecord>) -> Result<(), String> {
        let mut tx = Database::pool().begin().await.map_err(db_error)?;
        let keys: Vec<&str> = records.keys().map(String::as_str).collect();
        sqlx::query("DELETE FROM scrape_errors WHERE NOT(key=ANY($1))")
            .bind(&keys)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        
        for record in records.values() {
            let mut payload = serde_json::to_value(record).map_err(json_error)?;
            for field in ["key", "retailer", "active", "reviewed", "reviewed_at", "last_seen"] {
                payload.as_object_mut().ok_or("Scrape error must be an object")?.remove(field);
            }
            sqlx::query("INSERT INTO scrape_errors(key,retailer,active,reviewed,last_seen,payload,reviewed_at) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(key) DO UPDATE SET retailer=excluded.retailer,active=excluded.active,last_seen=excluded.last_seen,payload=excluded.payload WHERE (scrape_errors.retailer,scrape_errors.active,scrape_errors.last_seen,scrape_errors.payload) IS DISTINCT FROM (excluded.retailer,excluded.active,excluded.last_seen,excluded.payload)")
                .bind(&record.key).bind(&record.retailer).bind(record.active).bind(record.reviewed)
                .bind(record.last_seen).bind(sqlx::types::Json(payload)).bind(record.reviewed_at)
                .execute(&mut *tx).await.map_err(db_error)?;
        }
        
        tx.commit().await.map_err(db_error)
    }

    // Serialize scan commits and alert creation so a creation cursor always
    // describes the same committed catalog used for its initial-price check.
    pub async fn lock_catalog(tx: &mut sqlx::Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT pg_advisory_xact_lock(1179402567)").execute(&mut **tx).await?;
        Ok(())
    }

    pub(crate) async fn load_products_in(transaction: &mut sqlx::Transaction<'_, Postgres>) -> Result<(HashMap<String, Product>, Vec<Product>), String> {
        let rows = sqlx::query_as(&format!("SELECT removed,{PRODUCT_DOCUMENT_SQL}::text FROM products LEFT JOIN catalog_views ON target_type='product' AND target_id=products.id"))
            .fetch_all(&mut **transaction)
            .await
            .map_err(db_error)?;
        Self::decode_products(rows)
    }

    fn decode_products(rows: Vec<(bool, String)>) -> Result<(HashMap<String, Product>, Vec<Product>), String> {
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

    pub(crate) async fn begin_catalog_update(pool: &PgPool) -> Result<(sqlx::Transaction<'static, Postgres>, ProductStorage), String> {
        let mut transaction = pool.begin().await.map_err(db_error)?;
        Self::lock_catalog(&mut transaction).await.map_err(db_error)?;
        // Directus does not take the application advisory lock. Block catalog
        // writers before reading the canonical rows, including unpolled edits.
        sqlx::query("LOCK TABLE products IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *transaction).await.map_err(db_error)?;
        let revision = Self::catalog_revision(&mut transaction).await?;
        let published = PUBLISHED_CATALOG_REVISION.load(Ordering::Acquire);
        if published >= 0 && revision == published {
            let storage = ProductStorage::get_storage().read().await.clone();
            return Ok((transaction, storage));
        }
        let (products, removed_products) = Self::load_products_in(&mut transaction).await?;
        Ok((transaction, ProductStorage { products, removed_products }))
    }

    pub(crate) async fn catalog_revision(tx: &mut sqlx::Transaction<'_, Postgres>) -> Result<i64, String> {
        sqlx::query_scalar("SELECT revision FROM admin_revision WHERE id=1").fetch_one(&mut **tx).await.map_err(db_error)
    }

    pub(crate) fn mark_catalog_published(revision: i64) {
        PUBLISHED_CATALOG_REVISION.store(revision, Ordering::Release);
    }

    pub(crate) async fn save_products_in(
        transaction: &mut sqlx::Transaction<'_, Postgres>,
        storage: &ProductStorage,
        previous: &ProductStorage,
        changed_ids: &std::collections::HashSet<String>,
        report: Option<&ScanReport>,
        variants: &crate::core::variant::VariantStorage,
    ) -> Result<(), String> {
        
        if let Some(report) = report {
            let observed_at = Utc::now().to_rfc3339();
            let watchers: Vec<(String, Option<i32>)> = sqlx::query_as("SELECT rule,lowest_seen FROM alerts WHERE state IN ('active','pending') AND expires_at>$1")
                .bind(&observed_at)
                .fetch_all(&mut **transaction)
                .await
                .map_err(db_error)?;
            
            let watchers: Vec<(crate::alerts::AlertRule, Option<i32>)> = watchers
                .into_iter()
                .filter_map(|(rule, lowest)| serde_json::from_str(&rule).ok().map(|rule| (rule, lowest)))
                .collect();
            
            // No snapshot queries or queue writes for unwatched/ineligible offers.
            for product in report
                .added_items
                .iter()
                .chain(report.edited_items.iter().map(|(p, _)| p))
                .filter(|product| watchers.iter().any(|(rule, _)| rule.eligible(product)))
            {
                let current = crate::alerts::offer_snapshot(product);
                let old = previous.products.get(&product.id);
                let event = crate::alerts::AlertEvent {
                    product: current,
                    previous: old.map(crate::alerts::offer_snapshot),
                };
                
                if !watchers.iter().any(|(rule, lowest)| {
                    if rule.recurring_low {
                        rule.eligible(&event.product) && lowest.is_none_or(|price| event.product.price < price)
                    } else {
                        rule.triggers(&event)
                    }
                }) {
                    continue;
                }
                
                sqlx::query("INSERT INTO alert_events(event_key,section,product_id,observed_at,payload,batch_id) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT(event_key) DO NOTHING")
                    .bind(format!("{}:{}", report.started_at.to_rfc3339(), product.id))
                    .bind(product.section.to_string())
                    .bind(&product.id)
                    .bind(&observed_at)
                    .bind(serde_json::to_string(&event).map_err(json_error)?)
                    .bind(report.started_at.to_rfc3339())
                    .execute(&mut **transaction)
                    .await
                    .map_err(db_error)?;
            }
        }
        
        if let Some(report) = report {
            let observed = Utc::now().to_rfc3339();
            let rows = report
                .added_items
                .iter()
                .map(|p| (p, "added", Vec::new()))
                .chain(report.edited_items.iter().map(|(p, changes)| (p, "edited", changes.clone())))
                .chain(report.removed_items.iter().map(|p| (p, "removed", Vec::new())))
                .map(|(product,kind,changes)| serde_json::json!({
                    "event_key":format!("{}:{}:{}",report.started_at.to_rfc3339(),kind,product.id),
                    "observed_at":observed,
                    "kind":kind,
                    "section":product.section,
                    "retailer":product.retailer,
                    "product_id":product.id,
                    "title":product.title,
                    "payload":{
                        "price":product.price,
                        "status":product.status,
                        "changes":changes
                    }
                })).collect::<Vec<_>>();
            for batch in rows.chunks(500) {
                sqlx::query("INSERT INTO catalog_activity(event_key,observed_at,kind,section,retailer,product_id,title,payload) SELECT event_key,observed_at,kind,section,retailer,product_id,title,payload FROM jsonb_to_recordset($1) AS rows(event_key text,observed_at text,kind text,section text,retailer text,product_id text,title text,payload jsonb) ON CONFLICT(event_key) DO NOTHING")
                    .bind(sqlx::types::Json(batch)).execute(&mut **transaction).await.map_err(db_error)?;
            }
        }
        
        let removed_index = previous.removed_products.iter().map(|p| (p.id.as_str(), p)).collect::<HashMap<_, _>>();
        let removed_products = storage.removed_products.iter().map(|p| (p.id.as_str(), p)).collect::<HashMap<_, _>>();
        let mut changed = Vec::new();
        for id in changed_ids {
            let (product, removed) = if let Some(product) = storage.products.get(id) { (product, false) }
                else if let Some(product) = removed_products.get(id.as_str()) { (*product, true) }
                else { continue };
            let old = if removed { removed_index.get(product.id.as_str()).copied() }
                else { previous.products.get(&product.id) };
            if old.is_none_or(|old| !old.same_catalog_data(product)) {
                changed.push((product, removed));
            }
        }
        upsert_products(transaction, changed).await?;
        variants.persist_in(transaction).await
    }

    pub async fn save_manual_product(product: &Product) -> Result<Product, String> {
        let entry = product.history.last().ok_or("Manual product edit has no history entry")?;
        let mut transaction = Database::pool().begin().await.map_err(db_error)?;
        Self::lock_catalog(&mut transaction).await.map_err(db_error)?;
        
        let (removed, payload): (bool, String) = sqlx::query_as("SELECT removed,product_document(products)::text FROM products WHERE id=$1 FOR UPDATE")
            .bind(&product.id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(db_error)?;
        
        let mut current: Product = serde_json::from_str(&payload).map_err(json_error)?;
        let url = current.url.clone();
        // Merge only this edit's deltas into the latest database record, so an
        // unrelated Directus edit is preserved even before cache polling runs.
        overrides::apply_entries(&mut current, std::slice::from_ref(entry), &url)?;
        sqlx::query("SELECT set_config('flagship.override_source','discord',true)")
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        
        upsert_product(&mut transaction, &current, removed).await?;
        transaction.commit().await.map_err(db_error)?;
        Ok(current)
    }

    pub(crate) async fn load_variants_in<T: DeserializeOwned + Default>(transaction: &mut sqlx::Transaction<'_, Postgres>) -> Result<T, String> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT variants.id,(payload||jsonb_build_object('id',variants.id,'views',coalesce(catalog_views.views,0)))::text FROM variants LEFT JOIN catalog_views ON target_type='variant' AND target_id=variants.id")
            .fetch_all(&mut **transaction)
            .await
            .map_err(db_error)?;
        if rows.is_empty() {
            return Ok(T::default());
        }
        let variants = rows
            .into_iter()
            .map(|(id, payload)| Ok((id, serde_json::from_str(&payload).map_err(json_error)?)))
            .collect::<Result<serde_json::Map<String, serde_json::Value>, String>>()?;
        serde_json::from_value(serde_json::json!({ "variants": variants })).map_err(json_error)
    }

    pub(crate) async fn save_variants_in<'a, I, V>(transaction: &mut sqlx::Transaction<'_, Postgres>, variants: I) -> Result<(), String>
    where
        I: IntoIterator<Item = (&'a String, &'a V)>,
        V: Serialize + 'a,
    {
        let mut rows = Vec::new();
        for (id, variant) in variants {
            let mut payload = serde_json::to_value(variant).map_err(json_error)?;
            let object = payload.as_object_mut().ok_or("Variant must be an object")?;
            object.remove("id");
            object.remove("views");
            rows.push((id, payload));
        }
        for batch in rows.chunks(500) {
            let mut query = sqlx::QueryBuilder::<Postgres>::new("INSERT INTO variants(id,payload) ");
            query.push_values(batch, |mut row, (id, payload)| { row.push_bind(*id).push_bind(sqlx::types::Json(payload)); });
            query.push(" ON CONFLICT(id) DO UPDATE SET payload=excluded.payload WHERE variants.payload IS DISTINCT FROM excluded.payload");
            query.build()
                .execute(&mut **transaction)
                .await
                .map_err(db_error)?;
        }
        Ok(())
    }

    pub async fn record_product_view(id: &str) -> Result<u64, String> {
        Self::record_view("product", id).await
    }

    pub async fn record_variant_view(id: &str) -> Result<u64, String> {
        Self::record_view("variant", id).await
    }

    async fn record_view(kind: &str, id: &str) -> Result<u64, String> {
        Self::record_view_in(Database::pool(), kind, id).await
    }

    async fn record_view_in(pool: &PgPool, kind: &str, id: &str) -> Result<u64, String> {
        let views: i64 = sqlx::query_scalar("INSERT INTO catalog_views(target_type,target_id,views) VALUES($1,$2,1) ON CONFLICT(target_type,target_id) DO UPDATE SET views=LEAST(catalog_views.views::numeric+1,9223372036854775807)::bigint RETURNING views")
            .bind(kind).bind(id).fetch_one(pool).await.map_err(db_error)?;
        Ok(views as u64)
    }

    pub async fn load_product_info_cache() -> Result<HashMap<String, CachedProductInfo>, String> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("DELETE FROM product_info_cache WHERE expires_at < $1::text::timestamptz")
            .bind(&now)
            .execute(Database::pool())
            .await
            .map_err(db_error)?;
        
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT url,jsonb_build_object('timestamp',fetched_at,'description',description,'images',images)::text FROM product_info_cache")
                .fetch_all(Database::pool())
                .await
                .map_err(db_error)?;
        
        rows.into_iter()
            .map(|(url, payload)| Ok((url, serde_json::from_str(&payload).map_err(json_error)?)))
            .collect()
    }

    pub async fn save_product_info_cache(cache: &HashMap<String, CachedProductInfo>) -> Result<(), String> {
        Self::save_product_info_cache_in(Database::pool(), cache).await
    }

    async fn save_product_info_cache_in(pool: &PgPool, cache: &HashMap<String, CachedProductInfo>) -> Result<(), String> {
        let now = Utc::now();
        let mut transaction = pool.begin().await.map_err(db_error)?;
        sqlx::query("DELETE FROM product_info_cache WHERE expires_at<=$1")
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        
        for (url, info) in cache {
            let expires_at = info.timestamp + CACHE_EXPIRATION_DURATION;
            if expires_at <= now {
                continue;
            }

            sqlx::query("INSERT INTO product_info_cache (url,expires_at,fetched_at,description,images) VALUES ($1,$2,$3,$4,$5) ON CONFLICT(url) DO UPDATE SET expires_at=excluded.expires_at,fetched_at=excluded.fetched_at,description=excluded.description,images=excluded.images WHERE (product_info_cache.fetched_at,product_info_cache.description,product_info_cache.images) IS DISTINCT FROM (excluded.fetched_at,excluded.description,excluded.images)")
                .bind(url)
                .bind(expires_at)
                .bind(info.timestamp)
                .bind(&info.description)
                .bind(sqlx::types::Json(&info.images))
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        transaction.commit().await.map_err(db_error)
    }

    pub fn pool() -> &'static PgPool {
        DATABASE.get().expect("Database has not been initialized")
    }
}

pub(crate) async fn upsert_product(transaction: &mut sqlx::Transaction<'_, Postgres>, product: &Product, removed: bool) -> Result<(), String> {
    upsert_products(transaction, [(product, removed)]).await
}

async fn upsert_products<'a>(transaction: &mut sqlx::Transaction<'_, Postgres>, products: impl IntoIterator<Item = (&'a Product, bool)>) -> Result<(), String> {
    let mut rows = Vec::new();
    for (product, removed) in products {
        let mut details = serde_json::to_value(product).map_err(json_error)?;
        for field in ["id", "section", "retailer", "url", "price", "status", "title", "description", "image", "components", "views"] {
            details.as_object_mut().ok_or("Product must be an object")?.remove(field);
        }
        rows.push((product, removed, details));
    }
    for batch in rows.chunks(500) {
        let mut query = sqlx::QueryBuilder::<Postgres>::new("INSERT INTO products(id,section,retailer,url,price,status,removed,title,description,image,components,details) ");
        query.push_values(batch, |mut row, (product, removed, details)| {
            row.push_bind(&product.id).push_bind(product.section.to_string()).push_bind(&product.retailer).push_bind(&product.url)
                .push_bind(product.price).push_bind(product.status.to_string()).push_bind(*removed).push_bind(&product.title)
                .push_bind(&product.description).push_bind(&product.image).push_bind(sqlx::types::Json(&product.components)).push_bind(sqlx::types::Json(details));
        });
        query.push(" ON CONFLICT(id) DO UPDATE SET section=excluded.section,retailer=excluded.retailer,url=excluded.url,price=excluded.price, \
         status=excluded.status,removed=excluded.removed,title=excluded.title,description=excluded.description,image=excluded.image, \
         components=excluded.components,details=excluded.details \
         WHERE (products.section,products.retailer,products.url,products.price,products.status,products.removed,products.title,products.description,products.image,products.components,products.details) \
         IS DISTINCT FROM (excluded.section,excluded.retailer,excluded.url,excluded.price,excluded.status,excluded.removed,excluded.title,excluded.description,excluded.image,excluded.components,excluded.details)");
        query.build().execute(&mut **transaction).await.map_err(db_error)?;
    }
    Ok(())
}

fn db_error(error: sqlx::Error) -> String {
    format!("PostgreSQL operation failed: {error}")
}

fn json_error(error: serde_json::Error) -> String {
    format!("Failed to serialize database value: {error}")
}
