use serde::{Deserialize, Serialize};
use tokio::fs;
use toml;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrpcConfig {
    pub ip: String,
    pub port: u32,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            ip: "127.0.0.1".to_string(),
            port: 8009,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EngineConfig {
    pub cache_time: u32,
    pub dir: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            cache_time: 1,
            dir: "./rusdb".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RusDbConfig {
    pub grpc: GrpcConfig,
    pub engine: EngineConfig,
}

pub async fn load() -> RusDbConfig {
    let exists = match fs::metadata(".rusdb.toml").await {
        Ok(meta) => {
            if meta.is_file() {
                true
            } else {
                panic!("config file is not a file");
            }
        }
        Err(_) => false,
    };
    if exists {
        match fs::read(".rusdb.toml").await {
            Ok(data) => toml::from_slice(&data).unwrap(),
            Err(err) => {
                panic!("unable to read config: {}", err);
            }
        }
    } else {
        println!("Unable to find config file. Creating defaults...");
        let config = RusDbConfig::default();
        fs::write(".rusdb.toml", toml::to_vec(&config).unwrap())
            .await
            .unwrap();
        config
    }
}
