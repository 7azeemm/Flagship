use crate::utils::file_loader::FileLoader;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use tokio::sync::OnceCell;

static FEATURES: OnceCell<Vec<FeatureDefinition>> = OnceCell::const_new();

#[derive(Debug, Deserialize, Clone)]
pub struct FeatureDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct Feature {
    pub id: String,
    pub name: String,
    pub description: String,
    pub hype_count: i64,
    pub hyped: bool,
}

pub async fn load() -> Result<(), String> {
    let features: Vec<FeatureDefinition> = FileLoader::load_json("config/roadmap.json").await?;
    FEATURES.set(features).map_err(|_| "Roadmap loaded twice".to_string())
}

pub async fn valid_feature(id: &str) -> Result<bool, String> {
    Ok(FEATURES.get().unwrap().iter().any(|feature| feature.id == id))
}

pub async fn features_list(pool: &PgPool, user: Option<&str>, browser: Option<&str>) -> Result<Vec<Feature>, String> {
    let counts: Vec<(String, i64, bool)> =
        sqlx::query_as("
            SELECT feature_id, COUNT(*), bool_or(coalesce(user_id = $1, false) OR coalesce(browser_hash = $2, false))
            FROM feature_votes
            GROUP BY feature_id
        ")
            .bind(user)
            .bind(browser)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    let counts = counts.into_iter().map(|(id, count, hyped)| (id, (count, hyped))).collect::<HashMap<_, _>>();
    Ok(FEATURES.get().cloned().unwrap()
        .into_iter()
        .map(|feature| {
            let FeatureDefinition { id, name, description, .. } = feature;
            let count = counts.get(&id);
            Feature {
                id,
                name,
                description,
                hype_count: count.map_or(0, |row| row.0),
                hyped: count.is_some_and(|row| row.1),
            }
        })
        .collect())
}

pub async fn set_vote(pool: &PgPool, feature: &str, user: Option<&str>, browser: &str, hyped: bool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    if let Some(user) = user {
        // Merge this browser's guest votes on its next authenticated vote. Duplicates collapse.
        sqlx::query("INSERT INTO feature_votes(feature_id,user_id) SELECT feature_id,$1 FROM feature_votes WHERE browser_hash=$2 ON CONFLICT(feature_id,user_id) DO NOTHING")
            .bind(user)
            .bind(browser)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM feature_votes WHERE browser_hash=$1").bind(browser).execute(&mut *tx).await?;
        if hyped {
            sqlx::query("INSERT INTO feature_votes(feature_id,user_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
                .bind(feature)
                .bind(user)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("DELETE FROM feature_votes WHERE feature_id=$1 AND user_id=$2")
                .bind(feature)
                .bind(user)
                .execute(&mut *tx)
                .await?;
        }
    } else if hyped {
        sqlx::query("INSERT INTO feature_votes(feature_id,browser_hash) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(feature)
            .bind(browser)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("DELETE FROM feature_votes WHERE feature_id=$1 AND browser_hash=$2")
            .bind(feature)
            .bind(browser)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}
