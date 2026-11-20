//! Escreve as mudanças com backup em `.alltheani/backups/<n>/` e desfaz a última aplicação.

use crate::codemod::Change;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
struct Manifest {
    files: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    path: String,
    existed: bool,
}

fn backups_dir(root: &Path) -> PathBuf {
    root.join(".alltheani").join("backups")
}

/// Escreve tudo e devolve a pasta do backup.
pub fn write(root: &Path, changes: &[Change]) -> anyhow::Result<PathBuf> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let dir = backups_dir(root).join(stamp.to_string());
    std::fs::create_dir_all(&dir)?;
    let mut manifest = Manifest { files: vec![] };
    for c in changes {
        let target = root.join(&c.path);
        let existed = target.exists();
        if existed {
            let b = dir.join("files").join(&c.path);
            std::fs::create_dir_all(b.parent().unwrap())?;
            std::fs::copy(&target, &b)?;
        }
        manifest.files.push(Entry {
            path: c.path.clone(),
            existed,
        });
    }
    std::fs::write(dir.join("manifest.json"), serde_json::to_string_pretty(&manifest)?)?;
    for c in changes {
        let target = root.join(&c.path);
        if let Some(p) = target.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&target, &c.new)?;
    }
    Ok(dir)
}

/// Desfaz a última aplicação. Devolve quantos arquivos voltaram.
pub fn undo(root: &Path) -> anyhow::Result<usize> {
    let base = backups_dir(root);
    let mut stamps: Vec<u128> = std::fs::read_dir(&base)
        .map_err(|_| anyhow::anyhow!("Nada para desfazer."))?
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().parse().ok())
        .collect();
    stamps.sort();
    let Some(last) = stamps.last() else {
        anyhow::bail!("Nada para desfazer.")
    };
    let dir = base.join(last.to_string());
    let manifest: Manifest = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json"))?)?;
    for e in &manifest.files {
        let target = root.join(&e.path);
        if e.existed {
            std::fs::copy(dir.join("files").join(&e.path), &target)?;
        } else if target.exists() {
            std::fs::remove_file(&target)?;
            // Remove pastas que ficaram vazias (ex.: src/alltheani).
            let mut p = target.parent();
            while let Some(d) = p {
                if d == root || std::fs::remove_dir(d).is_err() {
                    break;
                }
                p = d.parent();
            }
        }
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(manifest.files.len())
}
