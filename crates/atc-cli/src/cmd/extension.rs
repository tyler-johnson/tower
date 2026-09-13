//! Declare what this machine describes. Dispatch itself requires no declaration.
use crate::{error::CliError, machine, manifest, registry};

pub fn run(json: bool, name: Option<&str>, delete: Option<&str>) -> Result<(), CliError> {
    if let Some(name) = delete {
        if !registry::forget(name)? {
            return Err(CliError::coded(
                "extension/not-declared",
                format!("nothing on this machine is declared under `{name}`"),
                vec!["atc extension".into(), format!("atc extension {name}")],
            ));
        }
        if json {
            println!(
                "{}",
                machine::emit(
                    "extension",
                    &serde_json::json!({"removed": name, "file": registry::path()})
                )
            );
        } else {
            println!(
                "removed {name}\natc-{name} still runs from a shell; tower says nothing about it now"
            );
        }
    } else if let Some(name) = name {
        let replaced = registry::read()
            .get(name)
            .map(|d| d.manifest.version.clone());
        let shook = manifest::handshake(name)?;
        registry::declare(&shook)?;
        if json {
            println!(
                "{}",
                machine::emit(
                    "extension",
                    &serde_json::json!({"declared": {"manifest": shook.manifest, "path": shook.path}, "replaced": replaced, "file": registry::path()})
                )
            );
        } else {
            println!(
                "{} {name} {} from {}",
                if replaced.is_some() {
                    "re-declared"
                } else {
                    "declared"
                },
                shook.manifest.version,
                shook.path.display()
            );
            println!("  its verbs: {}", verbs(&shook.manifest));
            println!("  atc help {name}; atc explain {name}/<id>");
            if shook.manifest.briefing.is_some() {
                println!("  its briefing line rides tower's trigger notice");
            }
            if !shook.manifest.skills.is_empty() {
                println!("  its skills install beside tower's on atc hook");
            }
            println!("undo: atc extension -d {name}");
        }
    } else {
        let registry = registry::read();
        if json {
            let mut data = serde_json::to_value(registry).expect("registry serializes");
            for (row, entry) in data["declared"]
                .as_array_mut()
                .expect("list")
                .iter_mut()
                .zip(registry.declared())
            {
                row["resolved"] = serde_json::to_value(entry.resolve()).expect("path serializes");
            }
            println!("{}", machine::emit("extension", &data));
        } else {
            if let Some(why) = &registry.unreadable {
                eprintln!("atc: the registry does not read as one: {why}");
            }
            if registry.declared().is_empty() {
                println!("nothing is declared on this machine\natc extension <name> declares one");
            }
            for entry in registry.declared() {
                println!(
                    "{}  {}  {}",
                    entry.name(),
                    entry.manifest.version,
                    verbs(&entry.manifest)
                );
                if entry.resolve().is_none() {
                    println!("  no atc-{} on PATH any more", entry.name());
                }
            }
            for stale in &registry.stale {
                println!(
                    "{}  contract {} — from a contract this tower does not speak",
                    stale.name, stale.contract
                );
            }
        }
    }
    Ok(())
}
fn verbs(manifest: &manifest::Manifest) -> String {
    manifest
        .verbs
        .iter()
        .map(|verb| verb.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
