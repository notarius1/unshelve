use std::env;
use std::collections::HashMap;
use anyhow::{Context, Result};
use tokio::time::{sleep, Duration};
use clap::{Parser, Subcommand};
use openstack::compute::ServerAddress;
// use openstack::waiter::Waiter;
// use clap::builder::TypedValueParser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to config file with OpenStack credentials. Empty for default .env file
    #[arg(short, long, default_value = ".env")]
    config: String,

    /// Command to execute
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show list of all servers
    ServerList,
    /// Display detailed server information.
    /// Add <SERVER_NAME_OR_UUID> e.g. ./bin_file server-info ServerName or set SERVER_NAME var in .env or config
    ServerInfo {
        /// Server name or UUID
        #[arg(value_name = "SERVER_NAME")]
        server_identifier: Option<String>,
    },
    /// Manual unshelve server.
    /// Add <SERVER_NAME_OR_UUID> e.g. ./bin_file unshelve ServerName  or set SERVER_NAME var in .env or config
    Unshelve {
        /// Server name or UUID
        #[arg(value_name = "SERVER_NAME")]
        server_identifier: Option<String>,
    },
    /// Monitor server with auto-unshelve
    Start {
        /// raw - for sudo user, dgram - for unprivileged user
        #[arg(default_value = "dgram")]
        socket_type: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load environment variables from file
    dotenv::from_filename(&args.config).context(format!(
        "Failed to load environment from file: {}",
        args.config
    ))?;

    match args.command {
        Command::ServerList => {
            let cloud = init_cloud().await;
            list_servers(&cloud).await
        },
        Command::ServerInfo { server_identifier } => {
            let identifier = match server_identifier {
                Some(id) => id,
                None => {
                    env::var("SERVER_NAME")
                        .context("No server identifier provided and SERVER_NAME env var not set")?
                }
            };
            let cloud = init_cloud().await;
            server_info(&cloud, &identifier).await
        },
        Command::Unshelve { server_identifier } => {
            let identifier = match server_identifier {
                Some(id) => id,
                None => {
                    env::var("SERVER_NAME")
                        .context("No server identifier provided and SERVER_NAME env var not set")?
                }
            };
            let cloud = init_cloud().await;
            unshelve_manual(&cloud, &identifier).await
        },
        Command::Start { socket_type } => {
            let s = socket_type.unwrap().to_lowercase();
            let lower = s.to_lowercase();
            let use_dgram_socket: bool = if lower == "raw" {
                if is_sudo::check() != is_sudo::RunningAs::Root {
                    anyhow::bail!("For 'raw' socket type need privileged user");
                }
                false
            } else if lower == "dgram" {
                true
            } else {
                anyhow::bail!("Invalid socket type: '{}'. Allowed values: 'raw', 'dgram' (Case insensitive)", s);
            };
            println!("Socket type: {}", lower.to_uppercase());
            let cloud = init_cloud().await;
            start_monitoring(&cloud, use_dgram_socket).await
        },
    }
}

async fn init_cloud() -> openstack::Cloud {
    let cloud = openstack::Cloud::from_env()
        .await
        .context("Failed to authenticate with OpenStack")
        .unwrap();

    println!("Connected to OpenStack successfully!");
    cloud
}

/// List all servers in the project
async fn list_servers(cloud: &openstack::Cloud) -> Result<()> {
    println!("Fetching list of servers...");
    println!("{}", "-".repeat(90));

    let servers = cloud
        .list_servers()
        .await
        .context("Failed to fetch server list")?;

    println!("{:<10} | {:<40} | {:<15} | {:<20}",
             "NAME", "ID", "STATUS", "POWER");
    println!("{}", "=".repeat(90));

    for server in servers.clone() {

        let details = server.details().await?;

        println!("{:<10} | {:<40} | {:<15} | {:<20?}",
                 server.name(),
                 server.id(),
                 details.status().to_string(),
                 details.power_state()
        );
        println!("{}", "-".repeat(90));

        let addresses = details.addresses();
        let address_strings = get_server_addresses_string(&addresses);
        address_strings.iter().for_each(|s| println!("{:<12} {}", " ", s));
        println!("{}", "-".repeat(90));
    }

    println!("Total servers: {}", servers.len());
    Ok(())
}

fn get_server_addresses_string(addresses: &HashMap<String, Vec<ServerAddress>>) -> Vec<String> {
    let mut address_strings: Vec<String> = vec![];
    for net in addresses {
        let (net_name, ips) = net;
        let mut ip_attrib: Vec<String> = vec![];
        for ip in ips {
            let ip_type = match ip.addr_type {
                Some(ip_type) => ip_type.to_string(),
                None => "None".to_string(),
            };
            ip_attrib.push(format!("{} - {} ", ip.addr.to_string(), ip_type));
        }
        address_strings.push(format!("[{}] {}", net_name, ip_attrib.join(", ")));
    }
    address_strings
}

/// Display detailed information about a specific server
async fn server_info(cloud: &openstack::Cloud, server_identifier: &str) -> Result<()> {
    println!("Getting information for server: {}", server_identifier);
    println!("{}", "-".repeat(80));

    // Try to find server by name or ID
    let server: openstack::compute::Server = match cloud.get_server(server_identifier).await {
        Ok(server) => server,
        Err(_) => {
            // If not found by exact match, search in the list
            println!("Failed to get server: {}, try get identifier from server list...", server_identifier);
            let servers = cloud
                .list_servers()
                .await
                .context("Failed to fetch server list")?;

            let found = servers
                .into_iter()
                .find(|s| s.name().contains(server_identifier));

            match found {
                Some(server) => server.details().await?,
                None => anyhow::bail!("Server '{}' not found", server_identifier),
            }
        }
    };

    print_server_info(&server)?;
    Ok(())
}

/// Print detailed server information
fn print_server_info(server: &openstack::compute::Server) -> Result<()> {

    println!("{:<25} : {}", "ID", server.id());
    println!("{:<25} : {}", "Name", server.name());
    // println!("{:<25} : {}", "Status", server.status());

    if server.status().to_string() == "ACTIVE" {
        println!("{:<25} : ✅ {}", "Status", server.status());
    } else if server.status().to_string() == "SHELVED_OFFLOADED" {
        println!("{:<25} : ❄️ {}", "Status", server.status());
    } else {
        println!("{:<25} : ⚠️ {}", "Status", server.status());
    }

    println!("{:<25} : {:?}", "Power state", server.power_state());

    let addresses = server.addresses();
    let address_strings: Vec<String> = get_server_addresses_string(&addresses);
    address_strings.iter().for_each(|s| println!("{:<25} {} {}", "Network", ":", s));

    println!("{}", "=".repeat(80));
    // println!("SERVER STATUS: {}", server.status());

    Ok(())
}

async fn unshelve_manual(cloud: &openstack::Cloud, server_identifier: &str) -> Result<()> {

    match cloud.get_server(&server_identifier).await {
        Ok(mut server) => {
            println!("Server status: {}", server.status());

            match server.action(openstack::compute::ServerAction::Unshelve).await {
                Ok(_) => {
                    println!("✓ Unshelve command sent successfully");

                }
                Err(e) => {
                    println!("✗ Failed to unshelve server: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to get server info: {}", e);
        }
    }

    Ok(())
}

fn ping_server(ip: &str, timeout_secs: u64, use_dgram_socket: bool) -> bool {
    let timeout = Duration::from_secs(timeout_secs);
    let socket_type = if use_dgram_socket { ping::DGRAM } else { ping::RAW };

    match ping::new(ip.parse().unwrap())
        .socket_type(socket_type)
        .timeout(timeout)
        // .ttl(128)
        // .seq_cnt(3)
        .send()
    {
        Ok(r) => {
            println!("[{}] {} Ping successful {:?}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), r.target, r.rtt);
            true
        },
        Err(_e) => {
            println!("[{}] {} Ping failed", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), ip);
            false
        },
    }
}

// need sudo sysctl -w net.ipv4.ping_group_range="0 1000" for Ubuntu (check sysctl net.ipv4.ping_group_range | default "1 0")
async fn start_monitoring(cloud: &openstack::Cloud, use_dgram_socket: bool) -> Result<()> {
    // Get configuration from environment
    let server_name = env::var("SERVER_NAME")
        .context("SERVER_NAME not set in environment")?;

    let ping_ip = env::var("PING_IP")
        .context("PING_IP not set in environment")?;

    let ping_interval_minutes: u64 = env::var("PING_INTERVAL_MINUTES")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .context("PING_INTERVAL_MINUTES must be a number")?;

    let ping_timeout_secs: u64 = env::var("PING_TIMEOUT_SECONDS")
        .unwrap_or_else(|_| "3".to_string())
        .parse()
        .context("PING_TIMEOUT_SECONDS must be a number")?;

    println!("Starting monitoring for server '{}'", server_name);
    println!("Ping target: {}", ping_ip);
    println!("Check interval: {} minutes", ping_interval_minutes);
    println!("Ping timeout: {} seconds", ping_timeout_secs);
    println!("{}", "=".repeat(80));

    // let mut interval = Duration::from_secs(ping_interval_minutes * 60);

    loop {
        let mut interval = Duration::from_secs(ping_interval_minutes * 60);

        let is_ping_successful = ping_server(&ping_ip, ping_timeout_secs, use_dgram_socket);

        if is_ping_successful {
        } else {
            println!("checking OpenStack status...");

            // 2. Get server status from OpenStack
            match cloud.get_server(&server_name).await {
                Ok(mut server) => {
                    let status = server.status();
                    println!("Server status in OpenStack: {}", status);

                    // 3. Check if server is shelved_offloaded
                    if status.to_string() == "SHELVED_OFFLOADED" {
                        println!("Server is shelved_offloaded - attempting to unshelve...");

                        match server.action(openstack::compute::ServerAction::Unshelve).await {
                            Ok(_) => {
                                println!("✓ Unshelve command sent successfully");

                                // Wait for server to become active
                                println!("Waiting for server to become ACTIVE...");
                                interval = Duration::from_secs(1 * 60);

                            }
                            Err(e) => {
                                println!("✗ Failed to unshelve server: {}", e);
                            }
                        }
                    } else {
                        println!("Server status is '{}' - no action required", status);
                    }
                }
                Err(e) => {
                    println!("✗ Failed to get server info: {}", e);
                }
            }
        }

        // println!("Next check in {} minutes...", ping_interval_minutes);
        sleep(interval).await
    }
}