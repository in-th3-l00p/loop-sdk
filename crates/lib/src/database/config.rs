#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Driver {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub url: String,
    pub driver: Driver,
}

impl Config {
    pub fn from_url(url: impl Into<String>) -> Config {
        let url = url.into();
        let driver = if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Driver::Postgres
        } else {
            Driver::Sqlite
        };
        Config { url, driver }
    }

    pub fn from_env() -> Option<Config> {
        std::env::var("LOOP_DB_URL").ok().map(Config::from_url)
    }
}
