use serde::{
    de::{self,Deserializer},
    Deserialize, Serialize,
};
use std::{
    collections::{HashMap, HashSet},
    io::{BufReader,Read},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    fmt,
};
use anyhow::Result;

fn deserialize_socket_addrs<'de, D>(deserializer: D) -> Result<Option<Vec<SocketAddr>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SocketAddrVisitor;

    impl<'de> serde::de::Visitor<'de> for SocketAddrVisitor {
        type Value = Vec<SocketAddr>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a list of socket addresses")
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: serde::de::SeqAccess<'de>,
        {
            let mut addresses = vec![];

            while let Some(addr_str) = seq.next_element::<String>()? {
                let addr: SocketAddr = addr_str.parse().map_err(serde::de::Error::custom)?;
                addresses.push(addr);
            }

            Ok(addresses)
        }
    }

    let addresses = deserializer.deserialize_option(SocketAddrVisitor)?;
    Ok(Some(addresses))
}

fn deserialize_ports<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct VecStringVisitor;

    impl<'de> de::Visitor<'de> for VecStringVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a sequence of strings or integers")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Vec<String>, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();

            while let Some(value) = seq.next_element::<serde_yaml::Value>()? {
                let as_string = match (value.as_str(), value.as_u64()) {
                    (Some(s), _) => s.to_string(),
                    (_, Some(i)) => i.to_string(),
                    _ => return Err(de::Error::custom("unexpected value type")),
                };
                vec.push(as_string);
            }

            Ok(vec)
        }
    }

    deserializer.deserialize_seq(VecStringVisitor).map(Some)
}

#[derive(Debug, Serialize, Deserialize)]
struct Compose {
    version: Option<String>,
    services: HashMap<String, Service>,
    networks: Option<HashMap<String, Network>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Service {
    image: Option<String>,
    restart: Option<String>,
    env_file: Option<String>,
    logging: Option<Logging>,
    #[serde(default, deserialize_with = "deserialize_ports")]
    ports: Option<Vec<String>>,
    networks: Option<Vec<String>>,
    volumes: Option<Vec<String>>,
    #[serde(rename = "depends_on")]
    depends_on: Option<DependsOn>,
    #[serde(default,deserialize_with = "deserialize_socket_addrs")]
    dns: Option<Vec<SocketAddr>>,
    hostname: Option<String>,
    environment: Option<HashMap<String,String>>,
    extra_hosts: Option<Vec<String>>,
    healthcheck: Option<Healthcheck>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Healthcheck {
    test: Vec<String>,
    interval: Option<String>,
    timeout: Option<String>,
    retries: Option<i32>,
    start_period: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Services {
    services: HashMap<String, Service>,
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum DependsOn {
    List(Vec<String>),
    Map(HashMap<String, Condition>),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Condition {
    condition: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Logging {
    driver: String,
    options: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Network {
    enable_ipv6: Option<bool>,
    driver: Option<String>,
    ipam: Option<Ipam>,
    internal: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Ipam {
    driver: Option<String>,
    config: Option<Vec<SubnetConfig>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SubnetConfig {
    subnet: IpNetwork,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum IpNetwork {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

// #[derive(Debug, Serialize)]
// #[serde(untagged)]
// enum SocketAddr {
//     #[serde(deserialize_with = "deserialize_socket_addr")]
//     V4(SocketAddr),
//     #[serde(deserialize_with = "deserialize_socket_addr")]
//     V6(SocketAddr),
// }

impl Compose {
    pub fn validate(&self) -> Result<(), String> {
        let service_names: HashSet<&String> = self.services.keys().collect();
        let networks = &self.networks;
        let t=&HashMap::new();
        let network_names: HashSet<&String> = networks.as_ref().unwrap_or(t).keys().collect();

        for (name, service) in &self.services {
            // Validate restart values
            if let Some(ref restart) = service.restart {
                if !["no", "always", "on-failure", "unless-stopped"]
                    .contains(&restart.as_str())
                {
                    return Err(format!(
                        "Invalid restart value '{}' for service '{}'",
                        restart, name
                    ));
                }
            }

            // Validate referenced networks
            if let Some(ref networks) = service.networks {
                for network in networks {
                    if !network_names.contains(network) {
                        return Err(format!(
                            "Referenced network '{}' not found for service '{}'",
                            network, name
                        ));
                    }
                }
            }

            // Validate depends_on services
            if let Some(ref depends_on) = service.depends_on {
                let s=match depends_on {
                    DependsOn::List(l) => l.clone(),
                    DependsOn::Map(k) => k.keys().cloned().collect(),
                };
                for dependency in s {
                    if !service_names.contains(&dependency) {
                        return Err(format!(
                            "Referenced service '{}' in depends_on not found for service '{}'",
                            dependency, name
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

pub fn parse_composefile(reader: Box<dyn Read>) -> Result<()> {
    let compose: Compose = serde_yaml::from_reader(BufReader::new(reader))?;
    match compose.validate(){
        Ok(()) => println!("Validation successful"),
        Err(err) => println!("Compose validation failed: {}", err),
    }
    println!("{:#?}", compose);
    Ok(())
}