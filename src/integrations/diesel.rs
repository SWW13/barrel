//!

use diesel::migration::{Migration, RunMigrationsError};
use diesel::connection::SimpleConnection;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::io::prelude::*;
use std::fs::*;
use std::fs;

/// Represents a migration run inside Diesel
///
/// 1. Path
/// 2. Version
/// 3. Up
/// 4. Down
pub struct BarrelMigration(PathBuf, String, String, String);

impl Migration for BarrelMigration {
    fn file_path(&self) -> Option<&Path> {
        Some(self.0.as_path())
    }

    fn version(&self) -> &str {
        &self.1
    }

    fn run(&self, conn: &SimpleConnection) -> Result<(), RunMigrationsError> {
        try!(conn.batch_execute(&self.2));
        Ok(())
    }

    fn revert(&self, conn: &SimpleConnection) -> Result<(), RunMigrationsError> {
        try!(conn.batch_execute(&self.3));
        Ok(())
    }
}

/// Generate migration files using the barrel schema builder
pub fn generate_initial(path: &PathBuf) {
    let migr_path = path.join("mod.rs");
    println!("Creating {}", migr_path.display());

    let mut barrel_migr = fs::File::create(migr_path).unwrap();
    barrel_migr.write(b"/// Handle up migrations \n").unwrap();
    barrel_migr
        .write(b"fn up(migr: &mut Migration) {} \n\n")
        .unwrap();

    barrel_migr.write(b"/// Handle down migrations \n").unwrap();
    barrel_migr
        .write(b"fn down(migr: &mut Migration) {} \n")
        .unwrap();
}

/// Generate a Migration from the provided path
pub fn migration_from(path: &Path) -> Option<Box<Migration>> {
    return match path.join("mod.rs").exists() {
        true => Some(run_barrel_migration_wrapper(&path.join("mod.rs"))),
        false => None,
    };
}

fn version_from_path(path: &Path) -> Result<String, ()> {
    path.file_name()
        .expect(&format!("Can't get file name from path `{:?}`", path))
        .to_string_lossy()
        .split('_')
        .nth(0)
        .map(|s| Ok(s.replace('-', "")))
        .unwrap_or_else(|| Err(()))
}

fn run_barrel_migration_wrapper(path: &Path) -> Box<Migration> {
    let (up, down) = run_barrel_migration(&path);
    let version = version_from_path(path).unwrap();
    return Box::new(BarrelMigration(path.to_path_buf(), version, up, down));
}

fn run_barrel_migration(migration: &Path) -> (String, String) {
    /* Create a tmp dir with src/ child */
    use tempdir::TempDir;

    let dir = TempDir::new("barrel").unwrap();
    fs::create_dir_all(&dir.path().join("src")).unwrap();

    /* Add a Cargo.toml file */
    let ct = dir.path().join("Cargo.toml");
    let mut cargo_toml = File::create(&ct).unwrap();
    cargo_toml
        .write_all(
            b"# This file is auto generated by barrel
[package]
name = \"tmp-generator\"
description = \"Doing nasty things with cargo\"
version = \"0.0.0\"
authors = [\"Katharina Fey <kookie@spacekookie.de>\"]

# TODO: Use same `barrel` dependency as crate
[dependencies]
barrel = { git = \"https://github.com/spacekookie/barrel\", features = [\"pg\"] }",
        )
        .unwrap();

    /* Generate main.rs based on user migration */
    let main_file_path = &dir.path().join("src").join("main.rs");
    let mut main_file = File::create(&main_file_path).unwrap();

    let user_migration = migration.as_os_str().to_os_string().into_string().unwrap();
    main_file
        .write_all(
            format!(
                "//! This file is auto generated by barrel
extern crate barrel;
use barrel::*;

// FIXME: Make the backend configurable
use barrel::backend::Pg;

include!(\"{}\");

fn main() {{
    let mut m_up = Migration::new();
    up(&mut m_up);
    println!(\"{{}}\", m_up.make::<Pg>());

    let mut m_down = Migration::new();
    down(&mut m_down);
    println!(\"{{}}\", m_down.make::<Pg>());
}}
",
                user_migration
            ).as_bytes(),
        )
        .unwrap();

    let output = if cfg!(target_os = "windows") {
        Command::new("cargo")
            .current_dir(dir.path())
            .arg("run")
            .output()
            .expect("failed to execute cargo!")
    } else {
        Command::new("sh")
            .current_dir(dir.path())
            .arg("-c")
            .arg("cargo run")
            .output()
            .expect("failed to execute cargo!")
    };

    let output = String::from_utf8_lossy(&output.stdout);
    let vec: Vec<&str> = output.split("\n").collect();
    let up = String::from(vec[0]);
    let down = String::from(vec[1]);

    return (up, down);
}
