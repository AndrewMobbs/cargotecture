use crate::parse_dockerfile;

static PACKAGE_HEADER:&str = r#" {
    import ScalarValues::*;
    
    attribute def image;
    attribute def label;
    attribute def maintainer;
    attribute def mountPoint;

    // Part Definition: Container
    part def Container {
        attribute image: String;
        attribute label: String[0..*];
        attribute maintainer: String[0..*];

        port networkPorts: NetworkPort[0..*];
        port volumePorts: VolumePort[0..*];
    }

    part def BaseImage {
        attribute imageName: String;
    }

    // Port Definition: NetworkPort
    port def NetworkPort {
        enum def Protocol {
            enum UDP;
            enum TCP;
        }

        attribute protocol: Protocol;
        attribute portNumber: Integer;
    }

    // Port Definition: VolumePort
    port def VolumePort {
        attribute mountPoint: String;
    }
    "#;
/// Generate a SysMLv2 Package for the parsed dockerfile
pub fn sysml_cargotecture_package(container: &parse_dockerfile::ParsedContainer) -> String {

    let mut package=format!("package {}Model",container.name);
    package.push_str(PACKAGE_HEADER);
    package.push_str(&format!("part {}System {{\n", container.name));
    package.push_str(&format!("        part {}Base: BaseImage {{\n",container.name));
    package.push_str(&format!("                attribute imageName redefines imageName = \"{}\";\n", container.base_image));
    package.push_str("            }\n");

    package.push_str(&format!("        part {}: Container {{\n", container.name));
    
    for (key, value) in &container.labels {
        package.push_str(&format!("            attribute {} redefines label = \"{}\";\n", key, value));
    }

    for (index, exposed_port) in container.exposed_ports.iter().enumerate() {
        package.push_str(&format!("            port port{}: NetworkPort {{\n", index));
        package.push_str(&format!("                protocol redefines protocol = Protocol::{};\n", exposed_port.protocol));
        package.push_str(&format!("                portNumber redefines portNumber = {};\n", exposed_port.port_number));
        package.push_str("            }\n");
    }

    for (index, volume) in container.volumes.iter().enumerate() {
        package.push_str(&format!("            port volume{}: VolumePort {{\n", index));
        package.push_str(&format!("                mountPoint redefines mountPoint = \"{}\";\n", volume.mount_point));
        package.push_str("            }\n");
    }

    package.push_str("        }\n"); // Close Container part
    package.push_str("    }\n"); // Close System Part
    package.push_str("}\n");// Close Package

    package
    }

