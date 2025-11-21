use std::{fs::File, io::BufReader};

use serde::{Deserialize, Deserializer};

#[derive(Debug)]
pub enum ProxyPass {
    Port(u16),
    Url(String),
}

impl<'de> Deserialize<'de> for ProxyPass {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Number(n) => {
                if let Some(port) = n.as_u64() {
                    if port <= u16::MAX as u64 {
                        Ok(ProxyPass::Port(port as u16))
                    } else {
                        Err(D::Error::custom("Port number too large"))
                    }
                } else {
                    Err(D::Error::custom("Invalid port number"))
                }
            }
            serde_json::Value::String(s) => Ok(ProxyPass::Url(s)),
            _ => Err(D::Error::custom("proxy_pass must be a number or string")),
        }
    }
}

impl ProxyPass {
    pub fn to_url(&self) -> String {
        match self {
            ProxyPass::Port(port) => format!("http://127.0.0.1:{}", port),
            ProxyPass::Url(url) => url.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub location: String,
    #[serde(deserialize_with = "ProxyPass::deserialize")]
    pub proxy_pass: ProxyPass,
}

#[derive(Debug, Deserialize)]
pub struct ProxyConfig {
    port: u16,
    rules: Vec<Rule>,
}

impl ProxyConfig {
    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn get_rules(&self) -> &Vec<Rule> {
        &self.rules
    }
}

pub fn load_config() -> ProxyConfig {
    let config: Option<ProxyConfig> = File::open("./config.json").ok().and_then(|file| {
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).ok()
    });

    let config = match config {
        Some(config) => config,
        None => {
            panic!("config.json is not valid JSON or file not found");
        }
    };
    config
}
