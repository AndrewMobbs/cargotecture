// Copyright Andrew Mobbs 2023
use std::{
    fs::File,
    path::Path,
    collections::HashMap,
    fmt::{self, Display, Formatter},
};
use dockerfile_parser::{Result, Dockerfile, Instruction};
use serde::{Deserialize, Serialize};
use escape_string;

#[derive(Debug, Deserialize,Serialize,PartialEq)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol::Tcp
    }
}

impl Display for Protocol {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "TCP"),
            Protocol::Udp => write!(f, "UDP"),
        }
    }
}


#[derive(Debug, Deserialize,Serialize,PartialEq)]
pub struct ExposedPort {
    pub port_number: u16,
    pub protocol: Protocol
}
#[derive(Debug, Deserialize,Serialize,PartialEq)]
pub struct VolumeMount {
    pub mount_point: String
}
#[derive(Debug, Deserialize,PartialEq)]
enum Port {
    Network(ExposedPort),
    Volume(Vec<VolumeMount>),
    None
}
impl Default for Port {
    fn default() -> Self {
        Port::None
    }
}
/// A type representing a container as specified by a Docker-style Containerfile
#[derive(Debug, Serialize)]
pub struct ParsedContainer {
    pub name: String,
    pub base_image: String,
    pub labels: HashMap<String, String>,
    pub exposed_ports: Vec<ExposedPort>,
    pub volumes: Vec<VolumeMount>,
    pub containerfile: Vec<String>,
}

fn parse_exposed_port(input: &str) -> Port {
    let parts: Vec<&str>= input.split('/').collect();
    let port:u16=parts[0].trim().parse().unwrap_or(0);
    let protocol = match parts.get(1) {
        Some(s) if s.to_lowercase() == "tcp" => Protocol::Tcp,
        Some(s) if s.to_lowercase() == "udp" => Protocol::Udp,
        _ => Protocol::default(),
    };
    if port == 0 {
        Port::None
    } else {
        Port::Network(ExposedPort{port_number: port,protocol,})
    }
}

fn parse_json_volume(json_str: &str) -> Port {
    let unix_paths:Result<Vec<String>,_> = serde_json::from_str(json_str);
    match unix_paths {
        Ok(paths) => {
            let volume_ports = paths
                .into_iter()
                .map(|mount_point| VolumeMount { mount_point })
                .collect();
            Port::Volume(volume_ports)
        },
        Err(_) => Port::None,
    }
}

fn parse_string_volume(input_str: &str) -> Port {
    let unix_paths = match escape_string::split(input_str) {
        Some(paths) => paths,
        None => return Port::None,
    };
    
    let volume_ports: Vec<VolumeMount> = unix_paths
        .into_iter()
        .map(|mount_point| VolumeMount {
            mount_point: mount_point.into_owned(),
        })
        .collect();
    Port::Volume(volume_ports)

}

fn parse_volume(input: &str) -> Port {
    match input.trim().chars().next() {
        Some('[') => parse_json_volume(input),
        Some('/') => parse_string_volume(input),
        _ => Port::None,
    }
}

fn parse_misc_instruction(inst: &dockerfile_parser::MiscInstruction) -> Port {
    let in_str = inst.instruction.to_string();
    match in_str.as_str() {
        "EXPOSE" => {
            parse_exposed_port(inst.arguments.to_string().as_str())
        },
        "VOLUME" => {
            parse_volume(inst.arguments.to_string().as_str())
        },
        _ => {Port::None}
    }
}

fn extract_dockerblock(dockerfile: &dockerfile_parser::Dockerfile) -> Result<ParsedContainer> {
    let mut name = String::new();
    let mut base_image = String::new();
    let mut labels = HashMap::new();
    let mut exposed_ports = Vec::new();
    let mut volumes = Vec::new();
    let mut containerfile = Vec::new();

    for stage in dockerfile.iter_stages() {
        name=stage.name.unwrap_or("".to_string());
        for ins in stage.instructions {
            let ins_str=format!("{}",&dockerfile.content[ins.span().start..ins.span().end]);
            containerfile.push(ins_str);
            match ins {
// TODO - Parse ARG (& ENV?) Instructions to provide expansion of others below
                Instruction::From(from) => {
                    base_image = from.image.clone().to_string();
                }
                Instruction::Label(label) => {

                    for item in &label.labels {
                        labels.insert(item.name.to_string(), item.value.to_string());
                    }
                }
                Instruction::Misc(misc) => {
                    
                    match parse_misc_instruction(misc) {
                        Port::Network(exposed) => {
                            exposed_ports.push(exposed);
                        }
                        Port::Volume(mut vol) => {
                            volumes.append(&mut vol);
                        }
                        Port::None => {
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let block = ParsedContainer {
        name,
        base_image,
        labels,
        exposed_ports,
        volumes,
        containerfile
    };

    Ok(block)
}

#[allow(dead_code)]
fn debug_dockerfile_parse(dockerfile: &dockerfile_parser::Dockerfile) {
    for stage in dockerfile.iter_stages() {
        println!(
          "stage #{} (parent: {:?}, root: {:?})",
          stage.index, stage.parent, stage.root
        );
        for ins in stage.instructions {
            println!("  {:?}", ins);
        }
      }  
}
/// A function to parse a dockerfile into a DockerfileBlock structure
/// Uses https://github.com/HewlettPackard/dockerfile-parser-rs/ for basic parsing
///  
pub fn parse_dockerfile(path: &str) -> Result<ParsedContainer> {
    let f = File::open(path).expect("file must be readable");
  
    let dockerfile = Dockerfile::from_reader(f)?;
    //debug_dockerfile_parse(&dockerfile);
    let mut block=extract_dockerblock(&dockerfile)?;
    if block.name == "" {
        block.name =  Path::new(path).file_name().unwrap().to_os_string().into_string().unwrap();
    }
    Ok(block)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::prelude::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_dockerfile() {
        // Create a temporary file with a sample Dockerfile content
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            "\
            FROM rust:latest
            LABEL version=\"1.0\"
            EXPOSE 8080
            VOLUME /data"
        )
        .unwrap();

        // Get the path of the temporary file
        let temp_path = temp_file.path().to_str().unwrap().to_string();

        // Call the parse_dockerfile function
        let dockerfile_block = parse_dockerfile(&temp_path).unwrap();

        // Check if the name of the DockerfileBlock is set to the file name
        assert_eq!(
            dockerfile_block.name,
            temp_file.path().file_name().unwrap().to_string_lossy()
        );

        // Check if the parsed DockerfileBlock has the expected base_image
        assert_eq!(dockerfile_block.base_image, "rust:latest");

        // Check if the parsed DockerfileBlock has the expected labels
        assert_eq!(
            dockerfile_block.labels.get("version"),
            Some(&String::from("1.0"))
        );

        // Check if the parsed DockerfileBlock has the expected exposed_ports
        assert_eq!(dockerfile_block.exposed_ports.len(), 1);
        assert_eq!(dockerfile_block.exposed_ports[0].port_number, 8080);

        // Check if the parsed DockerfileBlock has the expected volumes
        assert_eq!(dockerfile_block.volumes.len(), 1);
        assert_eq!(dockerfile_block.volumes[0].mount_point, "/data");
    }
    #[test]
    fn test_parse_volume() {
        // Test with JSON input
        let json_input = r#"[ "/data" , "/app" ]"#;
        let parsed_json_volume = parse_volume(json_input);
        if let Port::Volume(volume_mounts) = parsed_json_volume {
            assert_eq!(volume_mounts.len(), 2);
            assert_eq!(volume_mounts[0].mount_point, "/data");
            assert_eq!(volume_mounts[1].mount_point, "/app");
        } else {
            panic!("Expected Port::Volume, got {:?}", parsed_json_volume);
        }

        // Test with string input
        let string_input = "/data";
        let parsed_string_volume = parse_volume(string_input);
        if let Port::Volume(volume_mounts) = parsed_string_volume {
            assert_eq!(volume_mounts.len(), 1);
            assert_eq!(volume_mounts[0].mount_point, "/data");
        } else {
            panic!("Expected Port::Volume, got {:?}", parsed_string_volume);
        }

        // Test with invalid input
        let invalid_input = "invalid";
        let parsed_invalid_volume = parse_volume(invalid_input);
        assert_eq!(parsed_invalid_volume, Port::None);
    }

    use dockerfile_parser::{MiscInstruction,BreakableString, BreakableStringComponent, Span, SpannedString};

    fn create_misc_instruction(instruction: &str, arguments: Vec<BreakableStringComponent>) -> MiscInstruction {
        MiscInstruction {
            span: Span { start: 0, end: 0 },
            instruction: SpannedString {
                span: Span { start: 0, end: 0 },
                content: instruction.to_string(),
            },
            arguments: BreakableString {
                span: Span { start: 0, end: 0 },
                components: arguments,
            },
        }
    }

    #[test]
    fn test_parse_misc_instruction() {
        // Test with EXPOSE instruction
        let expose_instruction = create_misc_instruction("EXPOSE", vec![BreakableStringComponent::String(SpannedString {
            span: Span { start: 0, end: 0 },
            content: "8080/tcp".to_string(),
        })]);
        let parsed_expose = parse_misc_instruction(&expose_instruction);
        if let Port::Network(exposed_port) = parsed_expose {
            assert_eq!(exposed_port.port_number, 8080);
            assert_eq!(exposed_port.protocol, Protocol::Tcp);
        } else {
            panic!("Expected Port::Network, got {:?}", parsed_expose);
        }

        // Test with VOLUME instruction (string input)
        let volume_instruction_string = create_misc_instruction("VOLUME", vec![BreakableStringComponent::String(SpannedString {
            span: Span { start: 0, end: 0 },
            content: "/data".to_string(),
        })]);
        let parsed_volume_string = parse_misc_instruction(&volume_instruction_string);
        if let Port::Volume(volume_mounts) = parsed_volume_string {
            assert_eq!(volume_mounts.len(), 1);
            assert_eq!(volume_mounts[0].mount_point, "/data");
        } else {
            panic!("Expected Port::Volume, got {:?}", parsed_volume_string);
        }

        // Test with VOLUME instruction (JSON input)
        let volume_instruction_json = create_misc_instruction("VOLUME", vec![BreakableStringComponent::String(SpannedString {
            span: Span { start: 0, end: 0 },
            content: r#"[ "/data" , "/app" ]"#.to_string(),
        })]);
        let parsed_volume_json = parse_misc_instruction(&volume_instruction_json);
        if let Port::Volume(volume_mounts) = parsed_volume_json {
            assert_eq!(volume_mounts.len(), 2);
            assert_eq!(volume_mounts[0].mount_point, "/data");
            assert_eq!(volume_mounts[1].mount_point, "/app");
        } else {
            panic!("Expected Port::Volume, got {:?}", parsed_volume_json);
        }

        // Test with an unsupported instruction
        let unsupported_instruction = create_misc_instruction("MAINTAINER", vec![BreakableStringComponent::String(SpannedString {
            span: Span { start: 0, end: 0 },
            content: "John Doe <john@example.com>".to_string(),
        })]);
        let parsed_unsupported = parse_misc_instruction(&unsupported_instruction);
        assert_eq!(parsed_unsupported, Port::None);
    }
    #[test]
    fn test_parse_exposed_port() {
        // Test with a valid TCP port
        let tcp_input = "8080/tcp";
        let parsed_tcp_port = parse_exposed_port(tcp_input);
        if let Port::Network(exposed_port) = parsed_tcp_port {
            assert_eq!(exposed_port.port_number, 8080);
            assert_eq!(exposed_port.protocol, Protocol::Tcp);
        } else {
            panic!("Expected Port::Network, got {:?}", parsed_tcp_port);
        }

        // Test with a valid UDP port
        let udp_input = "8080/udp";
        let parsed_udp_port = parse_exposed_port(udp_input);
        if let Port::Network(exposed_port) = parsed_udp_port {
            assert_eq!(exposed_port.port_number, 8080);
            assert_eq!(exposed_port.protocol, Protocol::Udp);
        } else {
            panic!("Expected Port::Network, got {:?}", parsed_udp_port);
        }

        // Test with an invalid port number
        let invalid_input = "invalid/tcp";
        let parsed_invalid_port = parse_exposed_port(invalid_input);
        assert_eq!(parsed_invalid_port, Port::None);

        // Test with an unsupported protocol
        let unsupported_input = "8080/unsupported";
        let parsed_unsupported_protocol = parse_exposed_port(unsupported_input);
        if let Port::Network(exposed_port) = parsed_unsupported_protocol {
            assert_eq!(exposed_port.port_number, 8080);
            assert_eq!(exposed_port.protocol, Protocol::default());
        } else {
            panic!("Expected Port::Network, got {:?}", parsed_unsupported_protocol);
        }
    }
}
