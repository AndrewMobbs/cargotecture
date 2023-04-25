pub mod parse_dockerfile;
pub mod gen_sysml;
use std::fs;
use crate::parse_dockerfile::parse_dockerfile;
use anyhow::{Result,anyhow};

#[allow(dead_code)]
fn debug_dump_dockerfile_struct(block: &parse_dockerfile::DockerfileBlock) {
    let json = serde_json::to_string_pretty(&block).unwrap();
    println!("{}", json);
}

pub fn demo(path: &str) -> Result<()> {
    let block = parse_dockerfile(path)?;

    let bdd = gen_sysml::generate_sysml_bdd(&block);
    let ad = gen_sysml::generate_sysml_ad(&block);
    print!("------------------------------\n{}\n", bdd);
    print!("------------------------------\n{}\n", ad);
    
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

fn main() {
    match run() {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("An error occurred: {}", e);
            std::process::exit(1);
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