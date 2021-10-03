use crate::config::EngineConfig;
use bson::Document;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::fs;
use tokio::sync::RwLock;
use uuid::Uuid;

pub type RusDbCollection = Arc<RwLock<BTreeMap<Uuid, Document>>>;
struct RusCollection {
    pub last_access: SystemTime,
    pub flush_at: SystemTime,
    pub collection: RusDbCollection,
}

#[derive(Clone)]
pub struct RusDbEngine {
    cache: Arc<RwLock<BTreeMap<String, RusCollection>>>,
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
        let _dir = config.dir.clone().unwrap_or("./rusdb".to_string());
        if !dir_exists(&_dir).await {
            let mut p = std::env::current_dir().unwrap();
            p.push(&_dir);
            p.push("collections");
            fs::create_dir_all(&p).await.unwrap();
        }

        let engine = Arc::new(Self {
            cache: Arc::new(RwLock::new(BTreeMap::new())),
            config: Arc::new(config.clone()),
        });

        let engine_inner = engine.clone();
        let engine_inner_2 = engine.clone();

        let cache_time = config.cache_time as u64;
        let flush_time = config.flush_time as u64;

        tokio::spawn(async move {
            let engine = engine_inner_2;
            let shutdown_ = crate::SHUTDOWN_CHANNEL.0.clone();
            let mut shutdown = shutdown_.subscribe();
            let flush_task = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(60u64 * flush_time)).await;
                    let engine = engine.clone();
                    engine.flush_cache().await;
                }
            });
            tokio::select! {
                _ = flush_task => {},
                _ = shutdown.recv() => {},
            }
            debug!("Cache timeout flushing task closed.");
            drop(shutdown_);
        });

        tokio::spawn(async move {
            let engine = engine_inner;
            let engine_ = engine.clone();
            let shutdown_ = crate::SHUTDOWN_CHANNEL.0.clone();
            let mut shutdown = shutdown_.subscribe();
            let cache_loop = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(60u64 * cache_time)).await;
                    let engine = engine.clone();
                    engine.sync_cache().await;
                }
            });
            tokio::select! {
                _ = cache_loop => {},
                _ = shutdown.recv() => {},
            }
            debug!("Cache task loop finished.");
            engine_.sync_cache().await;
            drop(shutdown);
            drop(shutdown_);
        });
        engine
    }
    fn get_dir(&self) -> PathBuf {
        let mut p = std::env::current_dir().unwrap();
        p.push(&self.config.dir.clone().unwrap_or("./rusdb".to_string()));
        p
    }
    pub async fn flush_cache(&self) {
        let mut lock = self.cache.write().await;
        let mut entries: Vec<String> = vec![];
        let now = SystemTime::now();
        for (k, v) in &*lock {
            if &now >= &v.flush_at {
                // flush from the cache.
                debug!("Flushing {} from the cache...", k);
                let mut path = self.get_dir();
                path.push("collections");
                path.push(format!("{}.bson", k));
                let ilock = v.collection.read().await;
                let data = bson::to_vec(&*ilock).unwrap();
                fs::write(&path, &data).await.unwrap();
                entries.push(k.clone());
            }
        }
        for entry in entries.drain(..) {
            (*lock).remove(&entry);
        }
    }
    pub async fn sync_cache(&self) {
        let lock = self.cache.read().await;
        if (*lock).len() > 0 {
            for (k, v) in &*lock {
                let v = &v.collection;
                let mut path = self.get_dir();
                path.push("collections");
                path.push(format!("{}.bson", k));
                let ilock = v.read().await;
                let data = bson::to_vec(&*ilock).unwrap();
                fs::write(&path, &data).await.unwrap();
            }
        }
    }
    pub async fn get_collection(&self, name: &str) -> Option<RusDbCollection> {
        debug!("Attempting to load collection: {}", name);
        let is_cached = {
            let lock = self.cache.read().await;
            (*lock).contains_key(name)
        };
        if !is_cached {
            debug!("Collection is not cached.");
            let mut path = self.get_dir();
            path.push("collections");
            path.push(format!("{}.bson", &name));
            if let Some(data) = col_exists_file(&path).await {
                debug!("Loaded collection from disk.");
                let btree: RusDbCollection = Arc::new(RwLock::new(
                    bson::from_slice::<BTreeMap<Uuid, Document>>(&data).unwrap(),
                ));
                let now = SystemTime::now();
                let icol = RusCollection {
                    collection: btree.clone(),
                    last_access: now.clone(),
                    flush_at: now
                        .checked_add(Duration::from_secs(self.config.flush_time as u64 * 60u64))
                        .unwrap(),
                };
                {
                    let mut lock = self.cache.write().await;
                    (*lock).insert(name.to_string(), icol);
                }
                Some(btree)
            } else {
                // Doesn't exist, create it.
                debug!("Writing empty collection to disk.");
                let btree: BTreeMap<Uuid, Document> = BTreeMap::new();
                let data = bson::to_vec(&btree).unwrap();
                fs::write(&path, &data).await.unwrap();
                let btree = Arc::new(RwLock::new(btree));
                let now = SystemTime::now();
                let icol = RusCollection {
                    collection: btree.clone(),
                    last_access: now.clone(),
                    flush_at: now
                        .checked_add(Duration::from_secs(self.config.flush_time as u64 * 60u64))
                        .unwrap(),
                };
                {
                    let mut lock = self.cache.write().await;
                    (*lock).insert(name.to_string(), icol);
                }
                Some(btree)
            }
        } else {
            debug!("Collection was cached.");
            let mut lock = self.cache.write().await;
            if let Some(col) = (*lock).get_mut(name) {
                let now = SystemTime::now();
                col.last_access = now.clone();
                col.flush_at = now
                    .checked_add(Duration::from_secs(self.config.flush_time as u64 * 60u64))
                    .unwrap();
                debug!("Flushing the cache at {:?}", &col.flush_at);
                Some(col.collection.clone())
            } else {
                None
            }
        }
    }
}
