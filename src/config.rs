use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub stellar_rpc_url: String,
    pub start_ledger: u64,
    pub start_ledger_fallback: bool,
    pub port: u16,
    pub api_key: Option<String>,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub behind_proxy: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let behind_proxy = env::var("BEHIND_PROXY")
            .ok()
            .map(|v| {
                let v = v.to_ascii_lowercase();
                matches!(v.as_str(), "true" | "1" | "yes" | "y")
            })
            .unwrap_or(false);

        let start_ledger = env::var("START_LEDGER")
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .expect("START_LEDGER must be a number");

        let start_ledger_fallback = env::var("START_LEDGER_FALLBACK")
            .ok()
            .map(|v| {
                let v = v.to_ascii_lowercase();
                matches!(v.as_str(), "true" | "1" | "yes" | "y")
            })
            .unwrap_or(false);

        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .expect("PORT must be a number");

        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            stellar_rpc_url: env::var("STELLAR_RPC_URL")
                .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string()),
            start_ledger,
            start_ledger_fallback,
            port,
            api_key: env::var("API_KEY").ok(),
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("DB_MAX_CONNECTIONS must be a number"),
            db_min_connections: env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .expect("DB_MIN_CONNECTIONS must be a number"),
            behind_proxy,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "postgres://localhost/soroban_pulse".to_string(),
            stellar_rpc_url: "https://soroban-testnet.stellar.org".to_string(),
            start_ledger: 0,
            start_ledger_fallback: false,
            port: 8080,
            api_key: None,
            db_max_connections: 10,
            db_min_connections: 1,
            behind_proxy: false,
        }
    }
}
