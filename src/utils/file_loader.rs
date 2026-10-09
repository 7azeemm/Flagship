use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::Path;
use tokio::fs::{create_dir_all, read_to_string, write};

pub struct FileLoader;

impl FileLoader {
    pub async fn load_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
        let content = read_to_string(path).await.map_err(|e| format!("Failed to load {path}: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse {path}: {e}"))
    }

    pub async fn load_or_create_default<T: DeserializeOwned + Serialize + Default>(path: &str) -> Result<T, String> {
        match read_to_string(path).await {
            Ok(content) => {
                // File exists, parse the content
                serde_json::from_str(&content).map_err(|e| format!("Failed to parse {path}: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Self::ensure_parent_directory(path).await?;

                // File not found, create default content
                let default_val = T::default();
                let json = serde_json::to_string_pretty(&default_val)
                    .map_err(|e| format!("Failed to serialize default content for {path}: {e}"))?;

                write(path, json).await.map_err(|e| format!("Failed to create default file at {path}: {e}"))?;

                Ok(default_val)
            }
            Err(e) => Err(format!("Failed to load {path}: {e}")),
        }
    }

    async fn ensure_parent_directory(path: &str) -> Result<(), String> {
        if let Some(parent) = Path::new(path).parent() && !parent.exists() {
            create_dir_all(parent).await.map_err(|e| format!("Failed to create path for {path}: {e}"))?;
        }
        Ok(())
    }
}
