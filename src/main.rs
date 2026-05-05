mod build;
mod config;
mod content;
mod feed;
mod markdown;
mod new;
mod render;
mod scanner;
mod sitemap;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "ssg", version, about = "minimal personal blog SSG")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 정적 사이트를 public/ 에 빌드한다
    Build,
    /// 새 글 파일을 content/posts/ 에 생성한다
    New {
        /// 글 제목
        title: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = std::env::current_dir()?;
    match cli.cmd {
        Cmd::Build => {
            let start = Instant::now();
            build::run(&root)?;
            println!(
                "built site → {} ({:?})",
                root.join("public").display(),
                start.elapsed()
            );
        }
        Cmd::New { title } => {
            let path = new::run(&root, &title)?;
            println!("created {}", path.display());
        }
    }
    Ok(())
}
