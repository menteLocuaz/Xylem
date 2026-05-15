use std::path::{PathBuf};
use std::fs;
use duct::cmd;
use walkdir::WalkDir;
use directories::ProjectDirs;
use crate::runtime::list::GRAMMARS;
use crate::logger;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::{Semaphore, mpsc};

pub struct SyncResult {
    pub lang: String,
    pub success: bool,
    pub path: String,
    pub message: String,
}

pub struct ProgressEvent {
    pub lang: String,
    pub done: u32,
    pub total: u32,
}

pub async fn sync_one(lang: &str) -> SyncResult {
    let grammar = GRAMMARS.iter().find(|g| g.name == lang);
    let grammar = match grammar {
        Some(g) => g,
        None => {
            return SyncResult {
                lang: lang.to_string(),
                success: false,
                path: String::new(),
                message: format!("Language '{}' not found in grammar list", lang),
            };
        }
    };

    let proj_dirs = match ProjectDirs::from("com", "xylem", "xylem") {
        Some(d) => d,
        None => {
            return SyncResult {
                lang: lang.to_string(),
                success: false,
                path: String::new(),
                message: "Could not determine project directories".to_string(),
            };
        }
    };
    let base_dir = proj_dirs.data_dir().to_path_buf();

    let tmp_dir = std::env::temp_dir().join("xylem-sync");
    if !tmp_dir.exists() {
        let _ = fs::create_dir_all(&tmp_dir);
    }

    let name = &grammar.name;
    let url = &grammar.url;
    let revision = &grammar.revision;
    let repo_path = tmp_dir.join(name);

    let result = (|| -> anyhow::Result<String> {
        if repo_path.exists() {
            let _ = cmd!("git", "-C", &repo_path, "fetch", "--depth", "1", "origin", revision).run();
            let _ = cmd!("git", "-C", &repo_path, "checkout", revision).run();
        } else {
            cmd!("git", "clone", "--depth", "1", url, &repo_path).run()?;
        }

        let queries_dest = base_dir.join("queries").join(name);
        if queries_dest.exists() {
            fs::remove_dir_all(&queries_dest)?;
        }
        fs::create_dir_all(&queries_dest)?;

        let queries_src = repo_path.join("queries");
        if queries_src.exists() {
            for entry in WalkDir::new(&queries_src) {
                let entry = entry?;
                if entry.file_type().is_file() && entry.path().extension().and_then(|s| s.to_str()) == Some("scm") {
                    let rel_path = entry.path().strip_prefix(&queries_src)?;
                    let dest_path = queries_dest.join(rel_path);
                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(entry.path(), dest_path)?;
                }
            }
        }

        Ok(queries_dest.display().to_string())
    })();

    match result {
        Ok(path) => {
            logger::info(&format!("Synced {} to {}", lang, path));
            SyncResult {
                lang: lang.to_string(),
                success: true,
                path,
                message: "Success".to_string(),
            }
        }
        Err(e) => {
            logger::error(&format!("Failed to sync {}: {}", lang, e));
            SyncResult {
                lang: lang.to_string(),
                success: false,
                path: String::new(),
                message: e.to_string(),
            }
        }
    }
}

pub async fn sync_all(progress_tx: mpsc::Sender<ProgressEvent>) -> (u32, Vec<String>) {
    let sem = Arc::new(Semaphore::new(4));
    let total = GRAMMARS.len() as u32;
    let failed = Arc::new(parking_lot::Mutex::new(vec![]));
    let done = Arc::new(AtomicU32::new(0));

    let tasks: Vec<_> = GRAMMARS.iter().map(|g| {
        let sem = Arc::clone(&sem);
        let tx = progress_tx.clone();
        let failed = Arc::clone(&failed);
        let done = Arc::clone(&done);
        let lang = g.name.to_string();
        let name = g.name.to_string();

        tokio::spawn(async move {
            let _permit = sem.acquire().await.map_err(|e| anyhow::anyhow!(e)).unwrap();
            let result = sync_one(&lang).await;
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;

            if !result.success {
                failed.lock().push(name.clone());
            }

            let _ = tx.send(ProgressEvent { lang: name, done: n, total }).await;
        })
    }).collect();

    for handle in tasks {
        let _ = handle.await;
    }

    let failed_langs = failed.lock().clone();
    (total, failed_langs)
}

pub struct SyncManager {
    base_dir: PathBuf,
}

impl SyncManager {
    pub fn new() -> anyhow::Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "xylem", "xylem")
            .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;
        let base_dir = proj_dirs.data_dir().to_path_buf();
        
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)?;
        }

        Ok(Self { base_dir })
    }

    pub async fn sync_all(&self) -> anyhow::Result<()> {
        let (tx, mut rx) = mpsc::channel::<ProgressEvent>(32);

        let handle = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                println!("Synced {} ({}/{})", event.lang, event.done, event.total);
            }
        });

        let (total, failed_langs) = sync_all(tx).await;
        drop(handle);

        println!("Sync complete! {} total, {} failed", total, failed_langs.len());
        for lang in &failed_langs {
            println!("  Failed: {}", lang);
        }
        Ok(())
    }

    pub fn get_query_path(&self, lang: &str, query_name: &str) -> PathBuf {
        self.base_dir.join("queries").join(lang).join(format!("{}.scm", query_name))
    }
}
