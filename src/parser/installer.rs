use crate::parser::registry::{GrammarRegistry, GrammarSpec};
use std::path::{Path, PathBuf};
use std::io::Cursor;
use flate2::read::GzDecoder;
use tar::Archive;

pub struct ParserInstaller;

impl ParserInstaller {
    pub async fn install(spec: GrammarSpec) -> anyhow::Result<PathBuf> {
        let install_dir = GrammarRegistry::get_install_dir()?;
        let temp_dir = std::env::temp_dir().join(format!("xylem-{}", spec.name));
        
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        std::fs::create_dir_all(&temp_dir)?;

        // 1. Fetch source via tarball (GitHub releases/archive)
        let url = format!("{}/archive/{}.tar.gz", spec.repo, spec.revision);
        let response = reqwest::get(url).await?.bytes().await?;
        
        let tar = GzDecoder::new(Cursor::new(response));
        let mut archive = Archive::new(tar);
        archive.unpack(&temp_dir)?;
        
        // Find the root folder (usually repo-revision)
        let root = std::fs::read_dir(&temp_dir)?.next().unwrap()?.path();

        // 2. Compile
        let lib_path = Self::compile_grammar(&root, &spec.name, &install_dir)?;
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
        
        Ok(lib_path)
    }

    fn compile_grammar(src_dir: &Path, name: &str, output_dir: &Path) -> anyhow::Result<PathBuf> {
        // Find the src/ directory inside the repo
        let src_path = walkdir::WalkDir::new(src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .find(|e| e.file_name() == "src" && e.path().join("parser.c").exists())
            .map(|e| e.path().to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Could not find src/parser.c"))?;

        let parser_c = src_path.join("parser.c");
        let scanner_c = src_path.join("scanner.c");
        
        let mut build = cc::Build::new();
        // Force TARGET and HOST environment variables if missing
        if std::env::var("TARGET").is_err() {
            unsafe { std::env::set_var("TARGET", "x86_64-unknown-linux-gnu"); }
        }
        if std::env::var("HOST").is_err() {
            unsafe { std::env::set_var("HOST", "x86_64-unknown-linux-gnu"); }
        }
        build.include(&src_path)
             .file(&parser_c)
             .pic(true)
             .opt_level(3);
        
        if scanner_c.exists() {
            build.file(&scanner_c);
        }

        let output_name = format!("tree-sitter-{}", name);
        let lib_file = output_dir.join(format!("{}.so", output_name));
        
        // Manual shared library generation
        let compiler = build.get_compiler();
        let mut cmd = compiler.to_command();
        
        cmd.arg("-shared")
           .arg("-fPIC")
           .arg("-O3")
           .arg("-o").arg(&lib_file);

        cmd.arg(parser_c);
        if scanner_c.exists() {
            cmd.arg(scanner_c);
        }
        cmd.arg("-I").arg(src_path);

        let status = cmd.status()?;
        if !status.success() {
            return Err(anyhow::anyhow!("Compilation failed"));
        }

        Ok(lib_file)
    }
}
