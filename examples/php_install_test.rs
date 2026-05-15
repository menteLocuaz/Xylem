use xylem::parser::registry::GrammarSpec;
use xylem::parser::installer::ParserInstaller;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let spec = GrammarSpec {
        name: "php".to_string(),
        repo: "https://github.com/tree-sitter/tree-sitter-php".to_string(),
        revision: "master".to_string(),
        queries: vec![],
    };

    println!("Installing PHP parser...");
    match ParserInstaller::install(spec).await {
        Ok(path) => println!("Success! PHP installed at: {:?}", path),
        Err(e) => eprintln!("Failed: {:?}", e),
    }

    Ok(())
}
