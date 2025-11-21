use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair};
use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "macos")]
fn trust_cert_impl(cert_path: &str) {
    let _ = Command::new("sudo")
        .arg("security")
        .arg("add-trusted-cert")
        .arg("-d")
        .arg("-r")
        .arg("trustRoot")
        .arg("-k")
        .arg("/Library/Keychains/System.keychain")
        .arg(cert_path)
        .status()
        .expect("failed to add trusted cert on macOS");
}

#[cfg(target_os = "windows")]
fn trust_cert_impl(cert_path: &str) {
    let _ = Command::new("certutil")
        .arg("-addstore")
        .arg("-f")
        .arg("Root")
        .arg(cert_path)
        .status()
        .expect("failed to add trusted cert on Windows");
}

#[cfg(target_os = "linux")]
fn trust_cert_impl(cert_path: &str) {
    let _ = Command::new("sudo")
        .args([
            "cp",
            cert_path,
            "/usr/local/share/ca-certificates/my_root_ca.crt",
        ])
        .status()
        .expect("failed to copy cert on Linux");
    let _ = Command::new("sudo")
        .arg("update-ca-certificates")
        .status()
        .expect("failed to update ca-certificates on Linux");
}

/// 信任根证书
pub fn trust_root_ca() {
    let ca_cert_path = Path::new("./tmp/root_ca.pem");
    if !ca_cert_path.exists() {
        println!("Root CA not found, generating...");
        generate_root_ca();
    }
    trust_cert_impl(ca_cert_path.to_str().unwrap());
    println!("Root CA has been trusted by the system.");
}

/// 生成根证书 (只生成一次)
pub fn generate_root_ca() {
    fs::create_dir_all("./tmp").unwrap_or_default();
    let ca_cert_path = Path::new("./tmp/root_ca.pem");
    let ca_key_path = Path::new("./tmp/root_ca_key.pem");
    if ca_cert_path.exists() && ca_key_path.exists() {
        println!("Root CA already exists.");
        return;
    }
    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "Rust Local Root CA");
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_cert = Certificate::from_params(params).unwrap();
    fs::write(ca_cert_path, ca_cert.serialize_pem().unwrap()).unwrap();
    fs::write(ca_key_path, ca_cert.serialize_private_key_pem()).unwrap();
    println!("Root CA generated.");
}

pub fn generate_cert_for_domains(domains: &Vec<String>) {
    let ca_cert_path = Path::new("./tmp/root_ca.pem");
    let ca_key_path = Path::new("./tmp/root_ca_key.pem");
    if !ca_cert_path.exists() || !ca_key_path.exists() {
        panic!("Root CA not found, please generate root CA first. \n run `cargo run -- trust_root_ca` to generate root CA");
    }

    let root_ca = {
        let key_pem = fs::read_to_string(ca_key_path).unwrap();
        let key_pair = KeyPair::from_pem(&key_pem).unwrap();
        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "Rust Local Root CA");
        params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.key_pair = Some(key_pair);
        Certificate::from_params(params).unwrap()
    };
    if domains.is_empty() {
        println!("No domains to generate certificate for.");
        return;
    }

    let params = CertificateParams::new(domains.clone());
    let cert = Certificate::from_params(params).unwrap();
    let signed_cert = cert.serialize_pem_with_signer(&root_ca).unwrap();
    let key = cert.serialize_private_key_pem();
    fs::create_dir_all("./tmp").unwrap_or_default();

    fs::write("./tmp/local.crt", signed_cert).unwrap();
    fs::write("./tmp/local.key", key).unwrap();
    let domain_str = domains
        .iter()
        .map(|domain| format!("{}", domain))
        .collect::<Vec<String>>()
        .join("\n");
    println!("generate certificate for domains: \n{} \n", domain_str);
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn get_domains_from_hosts_impl() -> Vec<String> {
    let hosts_path = Path::new("/etc/hosts");
    let hosts = fs::read_to_string(hosts_path).unwrap();
    let domains = hosts
        .lines()
        .filter(|line| line.starts_with("127.0.0.1"))
        .map(|line| line.split_whitespace().nth(1).unwrap().to_string())
        .collect();
    domains
}

#[cfg(target_os = "windows")]
fn get_domains_from_hosts_impl() -> Vec<String> {
    let hosts_path = Path::new("C:\\Windows\\System32\\drivers\\etc\\hosts");
    let hosts = fs::read_to_string(hosts_path).unwrap();
    let domains = hosts
        .lines()
        .filter(|line| line.starts_with("127.0.0.1"))
        .map(|line| line.split_whitespace().nth(1).unwrap().to_string())
        .collect();
    domains
}

pub fn get_domains_from_hosts() -> Vec<String> {
    get_domains_from_hosts_impl()
}
