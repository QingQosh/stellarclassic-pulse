use anyhow::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub stellar_rpc_url: String,
    pub start_ledger: u64,
    pub start_ledger_fallback: bool,
    pub port: u16,
    pub api_key: Option<String>,
    pub behind_proxy: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

        let start_ledger: u64 = env::var("START_LEDGER")
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .context("START_LEDGER must be a valid u64")?;

        let port_str = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        let port: u16 = port_str.parse().context("PORT must be a valid u16")?;
        
        if port == 0 {
            anyhow::bail!("PORT must be a valid u16");
        }

        let start_ledger_fallback = env::var("START_LEDGER_FALLBACK")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

        let behind_proxy = env::var("BEHIND_PROXY")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

        Ok(Self {
            database_url,
            stellar_rpc_url: env::var("STELLAR_RPC_URL")
                .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string()),
            start_ledger,
            start_ledger_fallback,
            port,
            api_key: env::var("API_KEY").ok(),
            behind_proxy,
        })
    }
}
