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
    pub flush_time: u32,
    pub dir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogConfig {
    pub path: Option<String>,
    pub level: Option<u8>,
}

impl LogConfig {
    pub fn log_level(&self) -> log::LevelFilter {
        if let Some(lv) = self.level {
            match lv {
                0 => log::LevelFilter::Off,
                1 => log::LevelFilter::Error,
                2 => log::LevelFilter::Warn,
                3 => log::LevelFilter::Info,
                4 => log::LevelFilter::Debug,
                5 => log::LevelFilter::Trace,
                _ => log::LevelFilter::Trace,
            }
        } else {
            if cfg!(debug_assertions) {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            }
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            path: None,
            level: None,
        }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            cache_time: 1,
            flush_time: 10,
            dir: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RusDbConfig {
    pub grpc: GrpcConfig,
    pub engine: EngineConfig,
    pub logging: Option<LogConfig>,
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
        warn!("Unable to find config file. Creating defaults...");
        let config = RusDbConfig::default();
        fs::write(".rusdb.toml", toml::to_vec(&config).unwrap())
            .await
            .unwrap();
        println!("Default configuration file created. Please run again.");
        std::process::exit(1);
    }
}
