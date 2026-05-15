use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarSpec {
    pub name: String,
    pub repo: String,
    pub revision: String,
    pub queries: Vec<String>,
}

pub struct GrammarRegistry;

impl GrammarRegistry {
    pub fn get_install_dir() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?;
        
        // Following Neovim standard: stdpath("data")/site/xylem/parsers
        let install_dir = data_dir.join("nvim").join("site").join("xylem").join("parsers");
        
        if !install_dir.exists() {
            std::fs::create_dir_all(&install_dir)?;
        }
        
        Ok(install_dir)
    }
}
