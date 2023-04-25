use crate::parse_dockerfile;

// Generate an Activity Diagram representation of a dockerfile
pub fn generate_sysml_ad(block: &parse_dockerfile::DockerfileBlock) -> String {
    let mut ad = format!(
        "package {} {{\n\
            import Activities::*;\n\
            activity build_{} {{\n",
           block.name, block.name
       );
       
    for (index, line) in block.containerfile.iter().enumerate() {
        ad += &format!(
            "\n\
             action Command{} {{\n\
                 value command: String = \"{}\";\n\
             }};",
            index, line
        );
    }

    for index in 0..(block.containerfile.len() - 1) {
        ad += &format!(
            "\n\
             control Command{}_to_Command{}: Command{};",
            index, index + 1, index + 1
        );
    }

    ad += "\n}"; // Close Activity
    ad += "\n}"; // Close package
    ad

}
/// Generate a SysML Block Definition Diagram for the parsed dockerfile
pub fn generate_sysml_bdd(block: &parse_dockerfile::DockerfileBlock) -> String {
    let mut bdd = format!(
        "package {} {{\n\
            import ScalarValues::*;\n\
            import Base::*;\n\n\
            block «Container» {} {{\n",
           block.name, block.name
       );


    for (index, exposed_port) in block.exposed_ports.iter().enumerate() {
        bdd += &format!(
            "\n\
             port port{}: Port {{\n\
                 in protocol: String = \"{}\";\n\
                 in portNumber: Integer = {};\n\
             }};\n",
            index, exposed_port.protocol, exposed_port.port_number
        );
    }

    for (index, volume) in block.volumes.iter().enumerate() {
        bdd += &format!(
            "\n\
             port volume{}: «Data Volume» Port {{ \nin mount: String = \"{}\" \n }};\n",
            index, volume.mount_point
        );
    }

    for (index, volume) in block.volumes.iter().enumerate() {
        bdd += &format!(
            "\n\
             block «Data Volume» Volume{} {{\n\
                 value mount: String = \"{}\";\n\
            }};\n",
            index, volume.mount_point
        );
    }

    for (index, _) in block.volumes.iter().enumerate() {
        bdd += &format!(
            "\n\
             connect {}::volume{} to Volume{};",
            block.name, index, index
        );
    }
    bdd += &format!("allocate «allocate» build_{} to {}",block.name,block.name);
    bdd += "\n}"; // Close Block
    bdd += "\n}"; // Close Package
    bdd
}
