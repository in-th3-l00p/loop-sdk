use std::path::Path;

use lib::database::{Database, applied_versions};

use crate::{manifest, migrations, runtime};

const TEMPLATE: &str = "-- write your migration's SQL here\n";

pub fn create(dir: &str, name: &str) {
    if let Err(message) = create_inner(dir, name) {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

pub fn status(dir: &str) {
    if let Err(message) = status_inner(dir) {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn create_inner(dir: &str, name: &str) -> Result<(), String> {
    let slug = migrations::slugify(name);
    if slug.is_empty() {
        return Err("migration name must contain at least one letter or digit".to_string());
    }

    let project_dir = Path::new(dir);
    let migrations_dir = migrations::dir(project_dir);
    std::fs::create_dir_all(&migrations_dir).map_err(|e| format!("{}: {e}", migrations_dir.display()))?;

    let version = migrations::next_version(project_dir)?;
    let filename = format!("{version:04}_{slug}.sql");
    let path = migrations_dir.join(&filename);
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    std::fs::write(&path, TEMPLATE).map_err(|e| format!("{}: {e}", path.display()))?;

    println!("created migrations/{filename}");
    Ok(())
}

fn status_inner(dir: &str) -> Result<(), String> {
    let files = migrations::discover(Path::new(dir))?;
    let config = manifest::parse(dir).map_err(|e| e.to_string())?.database_config()?;

    let applied = runtime::block_on(async {
        let db = Database::connect(&config).await?;
        applied_versions(&db).await
    })
    .map_err(|e| e.to_string())?;

    for file in &files {
        let state = if applied.iter().any(|(version, _)| *version == file.version) {
            "applied"
        } else {
            "pending"
        };
        println!("{state:<7} {:04}_{}", file.version, file.name);
    }

    for (version, _) in &applied {
        if !files.iter().any(|file| file.version == *version) {
            println!("missing {version:04}_???  (applied in the database, no matching file on disk)");
        }
    }

    if files.is_empty() && applied.is_empty() {
        println!("no migrations yet — create one with `loop migration create <name>`");
    }

    Ok(())
}
