use std::path::Path;

use lib::database::{Config, Database, create_database, drop_database};

use crate::{manifest, migrations, runtime};

pub fn create(dir: &str) {
    run(|| create_inner(dir))
}

pub fn destroy(dir: &str, yes: bool) {
    run(|| destroy_inner(dir, yes))
}

pub fn migrate(dir: &str) {
    run(|| migrate_inner(dir))
}

pub fn reset(dir: &str, yes: bool) {
    run(|| {
        destroy_inner(dir, yes)?;
        create_inner(dir)?;
        migrate_inner(dir)
    })
}

pub fn url(dir: &str) {
    run(|| {
        let config = resolve_config(dir)?;
        println!("{}", config.url);
        Ok(())
    })
}

fn create_inner(dir: &str) -> Result<(), String> {
    let config = resolve_config(dir)?;
    runtime::block_on(create_database(&config)).map_err(|e| e.to_string())?;
    println!("created {}", describe(&config));
    Ok(())
}

fn destroy_inner(dir: &str, yes: bool) -> Result<(), String> {
    let config = resolve_config(dir)?;
    if !yes {
        return Err(format!(
            "this permanently deletes {} — rerun with --yes to confirm",
            describe(&config)
        ));
    }
    runtime::block_on(drop_database(&config)).map_err(|e| e.to_string())?;
    println!("destroyed {}", describe(&config));
    Ok(())
}

fn migrate_inner(dir: &str) -> Result<(), String> {
    let config = resolve_config(dir)?;
    let pending = migrations::load(Path::new(dir))?;
    let ran = runtime::block_on(async {
        let db = Database::connect(&config).await?;
        db.migrate(&pending).await
    })
    .map_err(|e| e.to_string())?;
    println!("applied {ran} migration(s)");
    Ok(())
}

fn resolve_config(dir: &str) -> Result<Config, String> {
    manifest::parse(dir).map_err(|e| e.to_string())?.database_config()
}

fn describe(config: &Config) -> String {
    format!("the {:?} database at {}", config.driver, redact(&config.url))
}

fn redact(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let (scheme, rest) = url.split_at(scheme_end + 3);
    let Some(at) = rest.find('@') else {
        return url.to_string();
    };
    let (credentials, host_and_rest) = rest.split_at(at);
    let user = credentials.split(':').next().unwrap_or("");
    format!("{scheme}{user}:***{host_and_rest}")
}

fn run(command: impl FnOnce() -> Result<(), String>) {
    if let Err(message) = command() {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn redact_masks_the_password_in_a_credentialed_url() {
        assert_eq!(
            redact("postgres://admin:secret@localhost/shop"),
            "postgres://admin:***@localhost/shop"
        );
    }

    #[test]
    fn redact_leaves_urls_without_credentials_untouched() {
        assert_eq!(redact("sqlite:my-api.db"), "sqlite:my-api.db");
        assert_eq!(redact("postgres://localhost/shop"), "postgres://localhost/shop");
    }
}
