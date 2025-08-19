#![allow(clippy::manual_async_fn)]

pub mod prelude;
pub mod common;
pub mod core;
pub mod components;
pub mod delays;
pub use strum;
pub use strum_macros;
pub mod nexosim {
    extern crate nexosim;
    pub use nexosim::model::*;
    pub use nexosim::time::*;
    pub use nexosim::simulation::*;
    pub use nexosim::ports::*;
    pub use nexosim::registry::*;
    pub use nexosim::server;
}

use std::{fs, path::{Path, PathBuf}};

// pub fn inject_boilerplate(path: PathBuf) -> anyhow::Result<()> {
//     let mut src = fs::read_to_string(&path)?;
//     if src.contains("=== BOILERPLATE START ===") {
//         eprintln!("already injected!");
//         return Ok(());
//     }
//     let boilerplate = r#"
// // === BOILERPLATE START ===
// use my_crate::prelude::*;
// // … your boilerplate …
    
// // === BOILERPLATE END ===
// "#;
//     src = format!("{}{}", boilerplate, src);
//     fs::write(&path, src)?;
//     println!("✅ injected into {}", path.display());
//     Ok(())
// }

use anyhow::{bail, Context, Result};

pub fn inject_boilerplate(path: &Path) -> Result<()> {
    // 1. read the existing file
    let src = fs::read_to_string(path)
        .with_context(|| format!("failed to read `{}`", path.display()))?;

    // 2. define your snippets
    const SNIPPET_ENUMS: &str = r#"
define_model_enums! {
    pub enum ComponentModel {}
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {}
    pub enum ScheduledEvent {}
}
"#;

    const SNIPPET_CUSTOM_COMPONENT: &str = r#"
impl CustomComponentConnection for ComponentModel {
    fn connect_components(
        a: &mut Self,
        b: &mut Self,
        n: Option<usize>
    ) -> Result<(), Box<dyn std::error::Error>> {
        match (a, b) {
            (a, b) => Err(format!(
                "No component connection defined from {} to {} (n={:?})",
                a, b, n
            )
            .into()),
        }
    }
}
"#;

    const SNIPPET_CUSTOM_LOGGER: &str = r#"
impl CustomLoggerConnection for ComponentLogger {
    type ComponentType = ComponentModel;

    fn connect_logger(
        a: &mut Self,
        b: &mut Self::ComponentType,
        n: Option<usize>
    ) -> Result<(), Box<dyn std::error::Error>> {
        match (a, b, n) {
            (a, b, _) => Err(format!(
                "No logger connection defined from {} to {} (n={:?})",
                a, b, n
            )
            .into()),
        }
    }
}
"#;

    // 3. collect only the missing ones
    let mut to_insert = String::new();

    if !src.contains("define_model_enums!") {
        to_insert.push_str("\n");
        to_insert.push_str(SNIPPET_ENUMS);
    }
    if !src.contains("impl CustomComponentConnection for ComponentModel") {
        to_insert.push_str("\n");
        to_insert.push_str(SNIPPET_CUSTOM_COMPONENT);
    }
    if !src.contains("impl CustomLoggerConnection for ComponentLogger") {
        to_insert.push_str("\n");
        to_insert.push_str(SNIPPET_CUSTOM_LOGGER);
    }

    // 4. nothing to do?
    if to_insert.is_empty() {
        println!("⛔ All boilerplate already present in `{}`", path.display());
        return Ok(());
    }

    // 5. split into lines and find `fn main`
    let lines: Vec<&str> = src.lines().collect();
    let main_idx = lines.iter()
        .position(|l| l.trim_start().starts_with("fn main"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "`fn main()` not found in `{}`, cannot inject boilerplate",
                path.display()
            )
        })?;

    // 6. rebuild the source, injecting right before `fn main`
    let mut new_src = String::new();
    for (i, &line) in lines.iter().enumerate() {
        if i == main_idx {
            new_src.push_str(&to_insert);
        }
        new_src.push_str(line);
        new_src.push('\n');
    }

    // 7. write it back
    fs::write(path, &new_src)
        .with_context(|| format!("failed to write `{}`", path.display()))?;

    println!("✅ Injected boilerplate into `{}`", path.display());
    Ok(())
}