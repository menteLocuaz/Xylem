use std::path::{PathBuf};
use std::fs;
use duct::cmd;
use walkdir::WalkDir;
use directories::ProjectDirs;
use crate::runtime::list::GRAMMARS;
use std::sync::Arc;
use tokio::sync::Semaphore;

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
        let tmp_dir = std::env::temp_dir().join("xylem-sync");
        if !tmp_dir.exists() {
            fs::create_dir_all(&tmp_dir)?;
        }

        let semaphore = Arc::new(Semaphore::new(4));
        let mut handles = Vec::new();

        for grammar in GRAMMARS {
            let name = grammar.name.to_string();
            let url = grammar.url.to_string();
            let revision = grammar.revision.to_string();
            let tmp_dir = tmp_dir.clone();
            let base_dir = self.base_dir.clone();
            let sem = Arc::clone(&semaphore);

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.map_err(|e| anyhow::anyhow!(e))?;
                println!("Syncing {}...", name);
                let repo_path = tmp_dir.join(&name);
                
                if repo_path.exists() {
                    // Fetch if exists
                    let _ = cmd!("git", "-C", &repo_path, "fetch", "--depth", "1", "origin", &revision).run();
                    let _ = cmd!("git", "-C", &repo_path, "checkout", &revision).run();
                } else {
                    // Clone if not exists
                    cmd!("git", "clone", "--depth", "1", &url, &repo_path).run()?;
                }
                
                // Extract queries
                let queries_dest = base_dir.join("queries").join(&name);
                if queries_dest.exists() {
                    fs::remove_dir_all(&queries_dest)?;
                }
                fs::create_dir_all(&queries_dest)?;

                let queries_src = repo_path.join("queries");
                if queries_src.exists() {
                    for entry in WalkDir::new(&queries_src) {
                        let entry = entry.map_err(|e| anyhow::anyhow!(e))?;
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
                Ok::<(), anyhow::Error>(())
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await??;
        }

        println!("Sync complete! Queries stored in {}", self.base_dir.join("queries").display());
        Ok(())
    }

    pub fn get_query_path(&self, lang: &str, query_name: &str) -> PathBuf {
        self.base_dir.join("queries").join(lang).join(format!("{}.scm", query_name))
    }
}
