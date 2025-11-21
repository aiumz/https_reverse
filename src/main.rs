mod cert;
mod config;
mod proxy;
use crate::proxy::start_proxy;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <command>", args[0]);
        println!("Commands:");
        println!("  trust_root_ca  - Generate and trust root CA certificate");
        println!("  proxy          - Start the HTTPS reverse proxy server");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "trust_root_ca" => {
            cert::trust_root_ca();
        }
        "proxy" => {
            let domains = cert::get_domains_from_hosts();
            cert::generate_cert_for_domains(&domains);
            let config = config::load_config();
            if let Err(e) = start_proxy(config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            eprintln!("Available commands: trust_root_ca, proxy");
            std::process::exit(1);
        }
    }
}
