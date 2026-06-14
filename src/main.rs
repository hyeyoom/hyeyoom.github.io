mod build;
mod config;
mod content;
mod feed;
mod markdown;
mod new;
mod render;
mod scanner;
mod search;
mod sitemap;

use anyhow::Result;
use clap::{Parser, Subcommand};
use new::ContentSection;
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
    /// 새 블로그 draft 파일을 content/drafts/blog/ 에 생성한다
    New {
        /// 글 제목
        title: String,
    },
    /// 블로그 draft를 만들거나 발행한다
    Blog {
        #[command(subcommand)]
        cmd: ContentCmd,
    },
    /// 번역 draft를 만들거나 발행한다
    Translation {
        #[command(subcommand)]
        cmd: ContentCmd,
    },
}

#[derive(Subcommand)]
enum ContentCmd {
    /// 새 draft 파일을 생성한다
    Draft {
        /// 글 제목
        title: String,
    },
    /// draft 파일을 공개 content 디렉토리로 이동한다
    Publish {
        /// draft 파일 경로, 파일명, 또는 slug
        draft: String,
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
        Cmd::Blog { cmd } => {
            run_content_cmd(&root, ContentSection::Blog, cmd)?;
        }
        Cmd::Translation { cmd } => {
            run_content_cmd(&root, ContentSection::Translation, cmd)?;
        }
    }
    Ok(())
}

fn run_content_cmd(root: &std::path::Path, section: ContentSection, cmd: ContentCmd) -> Result<()> {
    match cmd {
        ContentCmd::Draft { title } => {
            let path = new::draft(root, section, &title)?;
            println!("created {}", path.display());
        }
        ContentCmd::Publish { draft } => {
            let path = new::publish(root, section, &draft)?;
            println!("published {}", path.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blog_and_translation_draft_commands() {
        assert!(Cli::try_parse_from(["ssg", "blog", "draft", "Hello"]).is_ok());
        assert!(Cli::try_parse_from(["ssg", "translation", "draft", "Hello"]).is_ok());
    }

    #[test]
    fn parses_blog_and_translation_publish_commands() {
        assert!(Cli::try_parse_from(["ssg", "blog", "publish", "hello"]).is_ok());
        assert!(Cli::try_parse_from(["ssg", "translation", "publish", "hello"]).is_ok());
    }
}
