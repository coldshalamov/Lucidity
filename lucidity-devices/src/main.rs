//! CLI tool for managing paired mobile devices
//!
//! Usage:
//!   lucidity-devices list              # List all paired devices
//!   lucidity-devices revoke <pubkey>   # Revoke a device by public key
//!   lucidity-devices info <pubkey>     # Show device details

use anyhow::{Context, Result};
use lucidity_host::{list_trusted_devices, revoke_device};
use std::env;

fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "list" => cmd_list(),
        "revoke" => {
            if args.len() < 3 {
                eprintln!("Error: Missing public key argument");
                eprintln!("Usage: lucidity-devices revoke <public_key>");
                std::process::exit(1);
            }
            cmd_revoke(&args[2])
        }
        "info" => {
            if args.len() < 3 {
                eprintln!("Error: Missing public key argument");
                eprintln!("Usage: lucidity-devices info <public_key>");
                std::process::exit(1);
            }
            cmd_info(&args[2])
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => {
            eprintln!("Error: Unknown command '{}'", other);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!("Lucidity Device Management CLI");
    println!();
    println!("USAGE:");
    println!("    lucidity-devices <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    list                List all paired mobile devices");
    println!("    revoke <pubkey>     Revoke a device by public key");
    println!("    info <pubkey>       Show detailed device information");
    println!("    help                Show this help message");
}

fn cmd_list() -> Result<()> {
    let devices = list_trusted_devices().context("Failed to load trusted devices")?;

    if devices.is_empty() {
        println!("No paired devices found.");
        return Ok(());
    }

    println!("Paired Devices ({}):", devices.len());
    println!();

    for device in devices {
        let paired_date = chrono::DateTime::<chrono::Utc>::from_timestamp(device.paired_at, 0)
            .map(|dt: chrono::DateTime<chrono::Utc>| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let last_seen = device
            .last_seen
            .and_then(|ts| chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0))
            .map(|dt: chrono::DateTime<chrono::Utc>| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Never".to_string());

        println!("  Device: {}", device.device_name);
        println!("    Email:      {}", device.user_email);
        println!("    Public Key: {}", device.public_key.to_base64());
        println!("    Paired At:  {}", paired_date);
        println!("    Last Seen:  {}", last_seen);
        println!();
    }

    Ok(())
}

fn cmd_revoke(public_key: &str) -> Result<()> {
    println!("Revoking device with public key: {}", public_key);
    
    revoke_device(public_key).context("Failed to revoke device")?;
    
    println!("✓ Device revoked successfully");
    println!("The device will no longer be able to connect to this desktop.");
    
    Ok(())
}

fn cmd_info(public_key: &str) -> Result<()> {
    let devices = list_trusted_devices().context("Failed to load trusted devices")?;

    let device = devices
        .iter()
        .find(|d| d.public_key.to_base64() == public_key)
        .context("Device not found")?;

    let paired_date = chrono::DateTime::<chrono::Utc>::from_timestamp(device.paired_at, 0)
        .map(|dt: chrono::DateTime<chrono::Utc>| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let last_seen = device
        .last_seen
        .and_then(|ts| chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0))
        .map(|dt: chrono::DateTime<chrono::Utc>| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "Never".to_string());

    println!("Device Information:");
    println!();
    println!("  Name:       {}", device.device_name);
    println!("  Email:      {}", device.user_email);
    println!("  Public Key: {}", device.public_key.to_base64());
    println!("  Paired At:  {}", paired_date);
    println!("  Last Seen:  {}", last_seen);

    Ok(())
}
