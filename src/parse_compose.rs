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
pub struct Compose {
    version: Option<String>,
    services: HashMap<String, Service>,
    networks: Option<HashMap<String, Network>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Service {
    image: Option<String>,
    container_name: Option<String>,
    command: Option<String>,
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
pub struct Healthcheck {
    test: Vec<String>,
    interval: Option<String>,
    timeout: Option<String>,
    retries: Option<i32>,
    start_period: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Services {
    services: HashMap<String, Service>,
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependsOn {
    List(Vec<String>),
    Map(HashMap<String, Condition>),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Condition {
    condition: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Logging {
    driver: String,
    options: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Network {
    enable_ipv6: Option<bool>,
    driver: Option<String>,
    ipam: Option<Ipam>,
    internal: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ipam {
    driver: Option<String>,
    config: Option<Vec<SubnetConfig>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubnetConfig {
    subnet: IpNetwork,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpNetwork {
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

pub fn parse_composefile(reader: Box<dyn Read>) -> Result<Compose> {
    let compose: Compose = serde_yaml::from_reader(BufReader::new(reader))?;
    match compose.validate(){
        Ok(()) => println!("Validation successful"),
        Err(err) => println!("Compose validation failed: {}", err),
    }
    println!("{:#?}", compose);
    Ok(compose)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_yaml_sample() -> String {
        r#"
        services:
          db:
            healthcheck:
              test: ['CMD-SHELL', 'mysqladmin ping -h 127.0.0.1 --password="$$(cat /run/secrets/db-password)" --silent']
              interval: 3s
          backend:
            depends_on:
              db:
                condition: service_healthy
          proxy:
            depends_on: 
              - backend
        "#.to_string()
    }
    

    fn get_yaml_elk() -> String {
        // Replace with the second YAML string
        r#"
services:
  elasticsearch:
    image: elasticsearch:7.16.1
    container_name: es
    environment:
      discovery.type: single-node
      ES_JAVA_OPTS: "-Xms512m -Xmx512m"
    ports:
      - "9200:9200"
      - "9300:9300"
    healthcheck:
      test: ["CMD-SHELL", "curl --silent --fail localhost:9200/_cluster/health || exit 1"]
      interval: 10s
      timeout: 10s
      retries: 3
    networks:
      - elastic
  logstash:
    image: logstash:7.16.1
    container_name: log
    environment:
      discovery.seed_hosts: logstash
      LS_JAVA_OPTS: "-Xms512m -Xmx512m"
    volumes:
      - ./logstash/pipeline/logstash-nginx.config:/usr/share/logstash/pipeline/logstash-nginx.config
      - ./logstash/nginx.log:/home/nginx.log
    ports:
      - "5000:5000/tcp"
      - "5000:5000/udp"
      - "5044:5044"
      - "9600:9600"
    depends_on:
      - elasticsearch
    networks:
      - elastic
    command: logstash -f /usr/share/logstash/pipeline/logstash-nginx.config
  kibana:
    image: kibana:7.16.1
    container_name: kib
    ports:
      - "5601:5601"
    depends_on:
      - elasticsearch
    networks:
      - elastic
networks:
  elastic:
    driver: bridge
        "#.to_string()

    }

    fn check_elasticsearch_service(service: &Service) {
        assert_eq!(service.image.as_ref().unwrap(), "elasticsearch:7.16.1");
        assert_eq!(service.container_name.as_ref().unwrap(), "es");
        let environment = service.environment.as_ref().unwrap();
        assert_eq!(environment.get("discovery.type").unwrap(), "single-node");
        assert_eq!(environment.get("ES_JAVA_OPTS").unwrap(), "-Xms512m -Xmx512m");
    
        let ports = service.ports.as_ref().unwrap();
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0], "9200:9200");
        assert_eq!(ports[1], "9300:9300");
    
        let healthcheck = service.healthcheck.as_ref().unwrap();
        assert_eq!(healthcheck.test, vec!["CMD-SHELL", "curl --silent --fail localhost:9200/_cluster/health || exit 1"]);
        assert_eq!(healthcheck.interval.as_ref().unwrap(), "10s");
        assert_eq!(healthcheck.timeout.as_ref().unwrap(), "10s");
        assert_eq!(healthcheck.retries.unwrap(), 3);
    
        let networks = service.networks.as_ref().unwrap();
        assert_eq!(networks.len(), 1);
        assert_eq!(networks[0], "elastic");
    }

    fn check_logstash_service(service: &Service) {
        assert_eq!(service.image.as_ref().unwrap(), "logstash:7.16.1");
        assert_eq!(service.container_name.as_ref().unwrap(), "log");
    
        let environment = service.environment.as_ref().unwrap();
        assert_eq!(environment.get("discovery.seed_hosts").unwrap(), "logstash");
        assert_eq!(environment.get("LS_JAVA_OPTS").unwrap(), "-Xms512m -Xmx512m");
    
        let volumes = service.volumes.as_ref().unwrap();
        assert_eq!(volumes.len(), 2);
        assert_eq!(volumes[0], "./logstash/pipeline/logstash-nginx.config:/usr/share/logstash/pipeline/logstash-nginx.config");
        assert_eq!(volumes[1], "./logstash/nginx.log:/home/nginx.log");
    
        let ports = service.ports.as_ref().unwrap();
        assert_eq!(ports.len(), 4);
        assert_eq!(ports[0], "5000:5000/tcp");
        assert_eq!(ports[1], "5000:5000/udp");
        assert_eq!(ports[2], "5044:5044");
        assert_eq!(ports[3], "9600:9600");
    
        let depends_on = service.depends_on.as_ref().unwrap();
        match depends_on {
            DependsOn::List(services) => {
                assert_eq!(services.len(), 1);
                assert_eq!(services[0], "elasticsearch");
            }
            _ => panic!("Unexpected DependsOn variant"),
        }
    
        let networks = service.networks.as_ref().unwrap();
        assert_eq!(networks.len(), 1);
        assert_eq!(networks[0], "elastic");
    
        let command = service.command.as_ref().unwrap();
        assert_eq!(command, "logstash -f /usr/share/logstash/pipeline/logstash-nginx.config");
    
        // Since no other properties are defined for the logstash service in the provided YAML,
        // we'll check that they are set to their default values (i.e., None or empty).
        assert!(service.restart.is_none());
        assert!(service.env_file.is_none());
        assert!(service.logging.is_none());
        assert!(service.dns.is_none());
        assert!(service.hostname.is_none());
        assert!(service.extra_hosts.is_none());
        assert!(service.healthcheck.is_none());
    }

    fn check_kibana_service(service: &Service) {
        assert_eq!(service.image.as_ref().unwrap(), "kibana:7.16.1");
        assert_eq!(service.container_name.as_ref().unwrap(), "kib");
        let ports = service.ports.as_ref().unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0], "5601:5601");

        let depends_on = service.depends_on.as_ref().unwrap();
        match depends_on {
            DependsOn::List(services) => {
                assert_eq!(services.len(), 1);
                assert_eq!(services[0], "elasticsearch");
            }
            _ => panic!("Unexpected DependsOn variant"),
        }

        let networks = service.networks.as_ref().unwrap();
        assert_eq!(networks.len(), 1);
        assert_eq!(networks[0], "elastic");

        // Since no other properties are defined for the kibana service in the provided YAML,
        // we'll check that they are set to their default values (i.e., None or empty).
        assert!(service.restart.is_none());
        assert!(service.env_file.is_none());
        assert!(service.logging.is_none());
        assert!(service.environment.is_none());
        assert!(service.extra_hosts.is_none());
        assert!(service.healthcheck.is_none());
        assert!(service.dns.is_none());
        assert!(service.hostname.is_none());
        assert!(service.volumes.is_none());
    }

    fn check_db_service(service: &Service) {
        let healthcheck = service.healthcheck.as_ref().unwrap();
        assert_eq!(healthcheck.test, vec!["CMD-SHELL", "mysqladmin ping -h 127.0.0.1 --password=\"$$(cat /run/secrets/db-password)\" --silent"]);
        assert_eq!(healthcheck.interval.as_ref().unwrap(), "3s");
        assert!(healthcheck.timeout.is_none());
        assert!(healthcheck.retries.is_none());
        assert!(healthcheck.start_period.is_none());
    
        // Since no other properties are defined for the db service in the provided YAML, 
        // we'll check that they are set to their default values (i.e., None or empty).
        assert!(service.image.is_none());
        assert!(service.restart.is_none());
        assert!(service.env_file.is_none());
        assert!(service.logging.is_none());
        assert!(service.ports.is_none());
        assert!(service.networks.is_none());
        assert!(service.volumes.is_none());
        assert!(service.depends_on.is_none());
        assert!(service.dns.is_none());
        assert!(service.hostname.is_none());
        assert!(service.environment.is_none());
        assert!(service.extra_hosts.is_none());
    }

    fn check_backend_service(service: &Service) {
        let depends_on = service.depends_on.as_ref().unwrap();
        match depends_on {
            DependsOn::Map(conditions) => {
                let db_condition = conditions.get("db").unwrap();
                assert_eq!(db_condition.condition, "service_healthy");
            }
            _ => panic!("Unexpected DependsOn variant"),
        }
    
        // Since no other properties are defined for the backend service in the provided YAML, 
        // we'll check that they are set to their default values (i.e., None or empty).
        assert!(service.image.is_none());
        assert!(service.restart.is_none());
        assert!(service.env_file.is_none());
        assert!(service.logging.is_none());
        assert!(service.ports.is_none());
        assert!(service.networks.is_none());
        assert!(service.volumes.is_none());
        assert!(service.dns.is_none());
        assert!(service.hostname.is_none());
        assert!(service.environment.is_none());
        assert!(service.extra_hosts.is_none());
        assert!(service.healthcheck.is_none());
    }
    
    fn check_proxy_service(service: &Service) {
        let depends_on = service.depends_on.as_ref().unwrap();
        match depends_on {
            DependsOn::List(services) => {
                assert_eq!(services.len(), 1);
                assert_eq!(services[0], "backend");
            }
            _ => panic!("Unexpected DependsOn variant"),
        }
    
        // Since no other properties are defined for the proxy service in the provided YAML,
        // we'll check that they are set to their default values (i.e., None or empty).
        assert!(service.image.is_none());
        assert!(service.restart.is_none());
        assert!(service.env_file.is_none());
        assert!(service.logging.is_none());
        assert!(service.ports.is_none());
        assert!(service.networks.is_none());
        assert!(service.volumes.is_none());
        assert!(service.dns.is_none());
        assert!(service.hostname.is_none());
        assert!(service.environment.is_none());
        assert!(service.extra_hosts.is_none());
        assert!(service.healthcheck.is_none());
    }

    #[test]
    fn test_deserialization_sample() {
        let yaml_str = get_yaml_sample();
        let compose: Compose = serde_yaml::from_str(&yaml_str).unwrap();
        let services = compose.services;

        check_db_service(services.get("db").unwrap());
        check_backend_service(services.get("backend").unwrap());
        check_proxy_service(services.get("proxy").unwrap());
    }

    #[test]
    fn test_deserialization_elk() {
        let yaml_str = get_yaml_elk();
        let compose: Compose = serde_yaml::from_str(&yaml_str).unwrap();
        let services = compose.services;

        check_elasticsearch_service(services.get("elasticsearch").unwrap());
        check_logstash_service(services.get("logstash").unwrap());
        check_kibana_service(services.get("kibana").unwrap());
    }
}
