use std::{io, path::PathBuf};

use anyhow::{anyhow, Result};
use config::{Config, Environment, File};
use serde::Deserialize;
use storage::StorageSettings;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "clicktsdb", about = "A ClickHouse backed time series db.")]
pub struct CommandLineArgs {
    #[structopt(long)]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrometheusSettings {
    pub read: bool,
    pub write: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub web: WebSettings,
    pub storage: StorageSettings,
    pub prometheus: PrometheusSettings,
}

impl Settings {
    pub fn load(config_path_opt: Option<PathBuf>) -> Result<Self> {
        let config_path = config_path_opt.unwrap_or_else(|| default_config_path().unwrap());
        let config = Config::builder()
            .add_source(File::from(config_path))
            .add_source(Environment::with_prefix("CLICKTSDB"))
            .build()?;

        config
            .try_deserialize()
            .map_err(|err| anyhow!("Failed to read config: {:?}", err))
    }
}

fn default_config_path() -> io::Result<PathBuf> {
    let mut dir = std::env::current_exe()?;
    dir.pop();
    dir.push("configs/clicktsdb.yml");
    Ok(dir)
}
