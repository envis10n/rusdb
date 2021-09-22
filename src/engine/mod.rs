use bson::{Bson, Document};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::EngineConfig;

pub type RusDbCollection = Arc<RwLock<BTreeMap<Uuid, Document>>>;

#[derive(Clone)]
pub struct RusDbEngine {
    cache: Arc<RwLock<BTreeMap<String, RusDbCollection>>>,
    config: Arc<EngineConfig>,
}

impl Default for RusDbEngine {
    fn default() -> Self {
        Self {
            cache: Arc::new(RwLock::new(BTreeMap::new())),
            config: Arc::new(EngineConfig::default()),
        }
    }
}

async fn dir_exists(dst: &str) -> bool {
    if let Ok(meta) = fs::metadata(dst).await {
        meta.is_dir()
    } else {
        false
    }
}

async fn col_exists_file(path: &PathBuf) -> Option<Vec<u8>> {
    if let Ok(meta) = fs::metadata(path).await {
        if meta.is_file() {
            Some(fs::read(path).await.unwrap())
        } else {
            None
        }
    } else {
        None
    }
}

impl RusDbEngine {
    pub async fn create(config: &EngineConfig) -> Arc<Self> {
        if !dir_exists(&config.dir).await {
            let mut p = std::env::current_dir().unwrap();
            p.push(&config.dir);
            p.push("collections");
            fs::create_dir_all(&p).await.unwrap();
        }
        let inner_conf = config.clone();

        let engine = Arc::new(Self {
            cache: Arc::new(RwLock::new(BTreeMap::new())),
            config: Arc::new(config.clone()),
        });

        let engine_inner = engine.clone();

        tokio::spawn(async move {
            let config = inner_conf;
            let engine = engine_inner;
            loop {
                tokio::time::sleep(Duration::from_secs(60u64 * config.cache_time as u64)).await;
                println!("Syncing RusDb cache to disk...");
                let engine = engine.clone();
                engine.sync_cache().await;
                println!("RusDb cache synchronized to disk.");
            }
        });
        engine
    }
    fn get_dir(&self) -> PathBuf {
        let mut p = std::env::current_dir().unwrap();
        p.push(&self.config.dir);
        p
    }
    pub async fn sync_cache(&self) {
        let lock = self.cache.read().await;
        for (k, v) in &*lock {
            let mut path = self.get_dir();
            path.push("collections");
            path.push(format!("{}.bson", k));
            let ilock = v.read().await;
            let data = bson::to_vec(&*ilock).unwrap();
            fs::write(&path, &data).await.unwrap();
        }
    }
    pub async fn get_collection(&self, name: &str) -> Option<RusDbCollection> {
        let is_cached = {
            let lock = self.cache.read().await;
            (*lock).contains_key(name)
        };
        if !is_cached {
            let mut path = self.get_dir();
            path.push("collections");
            path.push(format!("{}.bson", &name));
            if let Some(data) = col_exists_file(&path).await {
                let btree: RusDbCollection = Arc::new(RwLock::new(
                    bson::from_slice::<BTreeMap<Uuid, Document>>(&data).unwrap(),
                ));
                {
                    let mut lock = self.cache.write().await;
                    (*lock).insert(name.to_string(), btree.clone());
                }
                Some(btree)
            } else {
                // Doesn't exist, create it.
                let btree: BTreeMap<Uuid, Document> = BTreeMap::new();
                let data = bson::to_vec(&btree).unwrap();
                fs::write(&path, &data).await.unwrap();
                let btree = Arc::new(RwLock::new(btree));
                {
                    let mut lock = self.cache.write().await;
                    (*lock).insert(name.to_string(), btree.clone());
                }
                Some(btree)
            }
        } else {
            let lock = self.cache.read().await;
            if let Some(col) = (*lock).get(name) {
                Some(col.clone())
            } else {
                None
            }
        }
    }
}
