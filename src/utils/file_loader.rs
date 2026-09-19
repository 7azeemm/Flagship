use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fs::File;
use std::path::Path;
use tokio::fs::{create_dir_all, read_to_string, write};

pub struct FileLoader;

impl FileLoader {
    pub async fn load_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
        let content = read_to_string(path).await.map_err(|e| format!("Failed to load {path}: {e}"))?;
        Ok(serde_json::from_str(&content).map_err(|e| format!("Failed to load {path}: {e}"))?)
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

    pub async fn save_file<T: Serialize>(path: &str, data: &T) -> Result<(), String> {
        Self::ensure_parent_directory(path).await?;

        let json = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize data for {path}: {e}"))?;

        write(path, json).await.map_err(|e| format!("Failed to write to {path}: {e}"))?;

        Ok(())
    }

    async fn ensure_parent_directory(path: &str) -> Result<(), String> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.exists() {
                create_dir_all(parent)
                    .await
                    .map_err(|e| format!("Failed to create path for {path}: {e}"))?;
            }
        }
        Ok(())
    }

    pub async fn read_csv_records(path: &str) -> Result<Vec<Value>, String> {
        let file = File::open(path).map_err(|err| format!("Failed to open file: {err}"))?;
        let mut csv_reader = csv::Reader::from_reader(file);

        let headers = csv_reader
            .headers()
            .map_err(|err| format!("Failed to read headers: {err}"))?
            .clone();

        let mut records = Vec::new();
        for result in csv_reader.records() {
            let record = result.map_err(|err| format!("Failed to read record: {err}"))?;

            let mut obj = serde_json::Map::new();
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = headers.get(i) {
                    obj.insert(header.to_string(), Value::String(field.to_string()));
                }
            }

            records.push(Value::Object(obj));
        }

        Ok(records)
    }
}
