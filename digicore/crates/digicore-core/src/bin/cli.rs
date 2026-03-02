//! CLI proof-of-concept: load and list snippets from JSON library.
//!
//! Usage: cli <path-to-text_expansion_library.json>

use digicore_core::adapters::persistence::JsonLibraryAdapter;
use digicore_core::domain::ports::SnippetRepository;
use std::env;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let path = if args.len() >= 2 {
        args[1].as_str()
    } else {
        eprintln!("Usage: cli <path-to-text_expansion_library.json>");
        eprintln!("Example: cli ../ACTIVE-Prod-LIVE-Apps/Text-Expansion/text_expansion_library.json");
        std::process::exit(1);
    };

    let path = Path::new(path);
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }

    let repo = JsonLibraryAdapter;
    let library = repo.load(&path)?;

    let total_snippets: usize = library.values().map(|v| v.len()).sum();
    println!("DigiCore - Library Summary");
    println!("=========================");
    println!("Categories: {}", library.len());
    println!("Total snippets: {}", total_snippets);
    println!();

    for (category, snippets) in &library {
        println!("  {} ({} snippets)", category, snippets.len());
        for (i, snip) in snippets.iter().take(5).enumerate() {
            let preview = if snip.content.len() > 50 {
                format!("{}...", &snip.content[..47])
            } else {
                snip.content.clone()
            };
            println!("    {}. [{}] -> {}", i + 1, snip.trigger, preview);
        }
        if snippets.len() > 5 {
            println!("    ... and {} more", snippets.len() - 5);
        }
        println!();
    }

    Ok(())
}
