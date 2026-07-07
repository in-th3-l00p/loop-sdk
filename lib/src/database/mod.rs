#[cfg(not(any(feature = "db-sqlite", feature = "db-postgres")))]
compile_error!(
    "the `database` feature needs a driver: enable `db-sqlite` and/or `db-postgres`"
);

mod config;
mod connection;
mod driver;
mod error;
mod global;
mod migration;
mod query;
mod sql;

#[cfg(all(test, feature = "db-sqlite"))]
mod tests;

pub use config::{Config, Driver};
pub use connection::Database;
pub use error::DatabaseError;
pub use global::{get, init, query, try_get};
#[cfg(feature = "macros")]
pub use global::{Migrations, init_from_env, registered_migrations};
pub use migration::{Migration, checksum};
pub use query::Query;
pub use sql::Dialect;
