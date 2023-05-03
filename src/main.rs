mod parse_dockerfile;
mod parse_podfile;
mod parse_compose;
mod util;
mod gen_sysml;

use std::fs;
use crate::{
    parse_dockerfile::{parse_dockerfile,parse_containerfile},
    parse_compose::parse_composefile,
    parse_podfile::parse_podfile,
    util::get_basename,
};

use anyhow::{Result,anyhow};
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::{self, BufReader, Read};

#[allow(dead_code)]
fn debug_dump_dockerfile_struct(block: &parse_dockerfile::ParsedContainer) {
    let json = serde_json::to_string_pretty(&block).unwrap();
    println!("{}", json);
}

pub fn demo(path: &str) -> Result<()> {
    let container = parse_dockerfile(path)?;
    let parts=gen_sysml::sysml_cargotecture_package(&container);
    print!("{}",parts);
    Ok(())
}

pub fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("a path to a Dockerfile is required");
    // Check if the file exists
    fs::metadata(path).map_err(|e| {
        anyhow!("Failed to read file metadata for {}: {}", path, e)
    })?;
    
    demo(path)
}

// fn main() {
//     match run() {
//         Ok(()) => std::process::exit(0),
//         Err(e) => {
//             eprintln!("An error occurred: {}", e);
//             std::process::exit(1);
//         }
//     }
// }

fn create_reader(filename: Option<&str>) -> Box<dyn Read> {
    match filename {
        Some(file) => {
            let file = File::open(file).expect("Unable to open the file");
            Box::new(BufReader::new(file))
        }
        None => Box::new(BufReader::new(io::stdin())),
    }
}

#[derive(Parser)]
#[clap(version = "0.1", author = "Andrew Mobbs <andrew.mobbs@gmail.com>", about = "Generate SysML version 2 representations of container files")]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[clap(about = "Parses container files", alias = "cf")]
    Containerfile {
        #[clap(help = "The input file. If not provided, stdin will be used")]
        filename: Option<String>,
    },
    #[clap(about = "Parses compose files", alias = "cmp")]
    Compose {
        #[clap(help = "The input file. If not provided, stdin will be used")]
        filename: Option<String>,
    },
    #[clap(about = "Parses pod files")]
    Pod {
        #[clap(help = "The input file. If not provided, stdin will be used")]
        filename: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Containerfile{ filename }) => {
            let reader = create_reader(filename.as_deref());
            let basename = get_basename(filename.as_deref().unwrap_or("Unknown"));
            let block=parse_containerfile(reader, &basename);
            match block {
                Ok(_)=> println!("Parse successful"),
                Err(err)=> println!("Parse failed: {}", err),
            };
        }
        Some(Commands::Compose{ filename }) => {
            let reader = create_reader(filename.as_deref());
            let block=parse_composefile(reader);
            match block{
                Ok(block) => println!("Parse successful"),
                Err(err) => println!("Parse failed: {}", err),
            };
        }
        Some(Commands::Pod{ filename }) => {
            let reader = create_reader(filename.as_deref());
            let block=parse_podfile(reader);
            match block{
                Ok(()) => println!("Parse successful"),
                Err(err) => println!("Parse failed: {}", err),
            };
        }
        None => {
            println!("Default subcommand");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::prelude::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_demo() -> Result<()> {
        let dockerfile_content = r#"
            FROM rust:1.55
            LABEL version="1.0"
            EXPOSE 8080/tcp
            VOLUME /data
        "#;

        let mut temp_dockerfile = NamedTempFile::new()?;
        writeln!(temp_dockerfile, "{}", dockerfile_content)?;

        let temp_dockerfile_path = temp_dockerfile.path().to_str().unwrap();
        demo(temp_dockerfile_path)?;

        // Add assertions for expected output or side effects

        Ok(())
    }
}