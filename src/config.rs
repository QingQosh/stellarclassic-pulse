use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub stellar_rpc_url: String,
    pub start_ledger: u64,
    pub port: u16,
    pub behind_proxy: bool,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            stellar_rpc_url: env::var("STELLAR_RPC_URL")
                .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string()),
            start_ledger: env::var("START_LEDGER")
                .unwrap_or_else(|_| "0".to_string())
                .parse()
                .expect("START_LEDGER must be a number"),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a number"),
            behind_proxy: env::var("BEHIND_PROXY")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        }
    }
}
