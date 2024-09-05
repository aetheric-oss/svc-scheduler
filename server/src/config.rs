//! # Config
//!
//! Define and implement config options for module

use anyhow::Result;
use config::{ConfigError, Environment};
use dotenv::dotenv;
use serde::Deserialize;

/// struct holding configuration options
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// port to be used for gRPC server
    pub docker_port_grpc: u16,

    /// port to be used for connecting to the storage service
    pub storage_port_grpc: u16,

    /// host to be used for connecting to the storage service
    pub storage_host_grpc: String,

    /// host to be used for connecting to the gis service
    pub gis_host_grpc: String,

    /// port to be used for connecting to the gis service
    pub gis_port_grpc: u16,

    /// path to log configuration YAML file
    pub log_config: String,

    /// config to be used for the Redis server
    pub redis: deadpool_redis::Config,
}

impl Default for Config {
    fn default() -> Self {
        log::warn!("(default) Creating Config object with default values.");
        Self::new()
    }
}

impl Config {
    /// Default values for Config
    pub fn new() -> Self {
        Config {
            docker_port_grpc: 50051,
            storage_port_grpc: 50051,
            storage_host_grpc: String::from("svc-storage"),
            gis_host_grpc: String::from("svc-gis"),
            gis_port_grpc: 50051,
            log_config: String::from("log4rs.yaml"),
            redis: deadpool_redis::Config {
                url: None,
                pool: None,
                connection: None,
            },
        }
    }

    /// Create a new `Config` object using environment variables
    pub fn try_from_env() -> Result<Self, ConfigError> {
        // read .env file if present
        dotenv().ok();
        let default_config = Config::default();

        config::Config::builder()
            .set_default("docker_port_grpc", default_config.docker_port_grpc)?
            .set_default("storage_port_grpc", default_config.storage_port_grpc)?
            .set_default("storage_host_grpc", default_config.storage_host_grpc)?
            .set_default("gis_port_grpc", default_config.gis_port_grpc)?
            .set_default("gis_host_grpc", default_config.gis_host_grpc)?
            .set_default("log_config", default_config.log_config)?
            .add_source(Environment::default().separator("__"))
            .build()?
            .try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use crate::Config;

    #[tokio::test]
    async fn test_config_from_default() {
        lib_common::logger::get_log_handle().await;
        ut_info!("Start.");

        let config = Config::default();

        assert_eq!(config.docker_port_grpc, 50051);
        assert_eq!(config.storage_port_grpc, 50051);
        assert_eq!(config.storage_host_grpc, String::from("svc-storage"));
        assert_eq!(config.gis_port_grpc, 50051);
        assert_eq!(config.gis_host_grpc, String::from("svc-gis"));
        assert_eq!(config.log_config, String::from("log4rs.yaml"));
        assert!(config.redis.url.is_none());
        assert!(config.redis.pool.is_none());
        assert!(config.redis.connection.is_none());

        ut_info!("Success.");
    }

    #[tokio::test]
    async fn test_config_from_env() {
        lib_common::logger::get_log_handle().await;
        ut_info!("Start.");

        std::env::set_var("DOCKER_PORT_GRPC", "6789");
        std::env::set_var("DOCKER_PORT_REST", "9876");
        std::env::set_var("STORAGE_HOST_GRPC", "test_host_storage_grpc");
        std::env::set_var("STORAGE_PORT_GRPC", "12345");
        std::env::set_var("GIS_HOST_GRPC", "test_host_gis_grpc");
        std::env::set_var("GIS_PORT_GRPC", "54321");
        std::env::set_var("LOG_CONFIG", "config_file.yaml");
        std::env::set_var("REDIS__URL", "redis://test_redis:6379");
        std::env::set_var("REDIS__POOL__MAX_SIZE", "16");
        std::env::set_var("REDIS__POOL__TIMEOUTS__WAIT__SECS", "2");
        std::env::set_var("REDIS__POOL__TIMEOUTS__WAIT__NANOS", "0");

        let config = Config::try_from_env();
        assert!(config.is_ok());
        let config = config.unwrap();

        assert_eq!(config.docker_port_grpc, 6789);
        assert_eq!(
            config.storage_host_grpc,
            String::from("test_host_storage_grpc")
        );
        assert_eq!(config.storage_port_grpc, 12345);
        assert_eq!(config.log_config, String::from("config_file.yaml"));

        assert_eq!(config.gis_host_grpc, String::from("test_host_gis_grpc"));
        assert_eq!(config.gis_port_grpc, 54321);
        assert_eq!(
            config.redis.url,
            Some(String::from("redis://test_redis:6379"))
        );
        assert!(config.redis.pool.is_some());

        ut_info!("Success.");
    }
}
