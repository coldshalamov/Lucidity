//! UPnP Port Mapping and Public IP Discovery
//!
//! Enables zero-config remote access by:
//! 1. Automatically opening an external port via UPnP
//! 2. Discovering the public IP address
//! 3. Providing connection info for remote clients

use anyhow::{Context, Result};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use stun::message::{BINDING_REQUEST, Message};
use stun::xoraddr::XorMappedAddress;
use tokio::net::UdpSocket;

/// External connection info for remote access
#[derive(Debug, Clone)]
pub struct ExternalConnectionInfo {
    /// Local IP address on LAN
    pub local_ip: Ipv4Addr,
    /// Public IP address
    pub public_ip: Ipv4Addr,
    /// External port mapped via UPnP
    pub external_port: u16,
    /// Local port being forwarded to
    pub local_port: u16,
    /// Whether UPnP mapping is active
    pub upnp_active: bool,
}

impl ExternalConnectionInfo {
    pub fn socket_addr(&self) -> SocketAddrV4 {
        SocketAddrV4::new(self.public_ip, self.external_port)
    }
    
    pub fn lan_addr(&self) -> SocketAddrV4 {
        SocketAddrV4::new(self.local_ip, self.local_port)
    }
}

/// Manages UPnP port mapping and public IP discovery
pub struct P2PConnectivity {
    local_port: u16,
    external_info: Arc<RwLock<Option<ExternalConnectionInfo>>>,
    gateway: Option<igd::Gateway>,
}

impl P2PConnectivity {
    /// Create a new P2P connectivity manager
    pub fn new(local_port: u16) -> Self {
        Self {
            local_port,
            external_info: Arc::new(RwLock::new(None)),
            gateway: None,
        }
    }

    /// Initialize UPnP and discover public IP
    /// Call this once at startup
    pub fn initialize(&mut self) -> Result<ExternalConnectionInfo> {
        log::info!("Initializing P2P connectivity...");

        // Step 1: Discover UPnP gateway
        let gateway = self.discover_gateway()?;
        self.gateway = Some(gateway.clone());

        // Step 2: Get local IP
        let local_ip = self.get_local_ip()?;

        // Step 3: Request port mapping
        let external_port = self.request_port_mapping(&gateway, local_ip)?;

        // Step 4: Discover public IP (Try STUN first, fallback to HTTP)
        let public_addr = self.discover_public_addr_via_stun().ok();
        
        let (public_ip, external_port) = if let Some(addr) = public_addr {
            (match addr.ip() {
                IpAddr::V4(ip) => ip,
                IpAddr::V6(_) => self.discover_public_ip()?, // Fallback to HTTP for IP
            }, addr.port())
        } else {
            (self.discover_public_ip()?, external_port)
        };

        let info = ExternalConnectionInfo {
            local_ip,
            public_ip,
            external_port,
            local_port: self.local_port,
            upnp_active: true,
        };

        log::info!(
            "P2P connectivity ready: {}:{} -> local:{}",
            public_ip,
            external_port,
            self.local_port
        );

        *self.external_info.write().unwrap() = Some(info.clone());
        Ok(info)
    }

    /// Get current external connection info
    pub fn get_external_info(&self) -> Option<ExternalConnectionInfo> {
        self.external_info.read().unwrap().clone()
    }

    /// Discover the UPnP gateway (router)
    fn discover_gateway(&self) -> Result<igd::Gateway> {
        log::debug!("Searching for UPnP gateway...");
        
        let options = igd::SearchOptions {
            timeout: Some(Duration::from_secs(5)),
            ..Default::default()
        };

        igd::search_gateway(options)
            .context("Failed to discover UPnP gateway. Your router may not support UPnP or it may be disabled.")
    }

    /// Get local IP address
    fn get_local_ip(&self) -> Result<Ipv4Addr> {
        // Use a UDP socket trick to get the local IP
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.connect("8.8.8.8:80")?;
        let local_addr = socket.local_addr()?;
        
        match local_addr.ip() {
            IpAddr::V4(ip) => Ok(ip),
            IpAddr::V6(_) => anyhow::bail!("IPv6 not supported yet"),
        }
    }

    /// Request a port mapping from the gateway
    fn request_port_mapping(&self, gateway: &igd::Gateway, local_ip: Ipv4Addr) -> Result<u16> {
        let local_addr = SocketAddrV4::new(local_ip, self.local_port);
        
        // Try the same external port as local first
        let mut external_port = self.local_port;
        
        // Try up to 10 different ports if the preferred one is taken
        for attempt in 0..10 {
            let try_port = if attempt == 0 {
                external_port
            } else {
                // Pick a random high port
                49152 + (rand_u16() % 16383)
            };

            log::debug!("Attempting UPnP mapping: external:{} -> {}:{}", 
                try_port, local_ip, self.local_port);

            match gateway.add_port(
                igd::PortMappingProtocol::TCP,
                try_port,
                local_addr,
                3600, // 1 hour lease
                "Lucidity Terminal",
            ) {
                Ok(()) => {
                    log::info!("UPnP port mapping created: external:{} -> local:{}", 
                        try_port, self.local_port);
                    return Ok(try_port);
                }
                Err(igd::AddPortError::PortInUse) => {
                    log::debug!("Port {} in use, trying another...", try_port);
                    continue;
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("UPnP port mapping failed: {}", e));
                }
            }
        }

        anyhow::bail!("Failed to find available external port after 10 attempts")
    }

    /// Discover public IP and port via STUN
    #[tokio::main(flavor = "current_thread")]
    async fn discover_public_addr_via_stun(&self) -> Result<SocketAddr> {
        log::debug!("Discovering public address via STUN...");
        
        let stun_server = "stun.l.google.com:19302";
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(stun_server).await?;

        let mut msg = Message::new();
        msg.build(&[Box::new(BINDING_REQUEST)])?;

        socket.send(&msg.raw).await?;

        let mut buf = [0u8; 1024];
        let (n, _) = tokio::time::timeout(Duration::from_secs(3), socket.recv_from(&mut buf)).await??;

        let mut response = Message::new();
        response.raw = buf[..n].to_vec();
        response.decode()?;

        let mut xor_addr = XorMappedAddress::default();
        xor_addr.get_from_as(&response, stun::attributes::ATTR_XORMAPPED_ADDRESS)?;

        let socket_addr = SocketAddr::new(xor_addr.ip, xor_addr.port);
        log::info!("Discovered public address via STUN: {}", socket_addr);
        Ok(socket_addr)
    }

    /// Discover public IP address
    fn discover_public_ip(&self) -> Result<Ipv4Addr> {
        log::debug!("Discovering public IP...");

        // Try multiple services for reliability
        let services = [
            "https://api.ipify.org",
            "https://ifconfig.me/ip",
            "https://icanhazip.com",
        ];

        for service in services {
            match self.fetch_public_ip(service) {
                Ok(ip) => {
                    log::info!("Public IP: {} (via {})", ip, service);
                    return Ok(ip);
                }
                Err(e) => {
                    log::debug!("Failed to get IP from {}: {}", service, e);
                    continue;
                }
            }
        }

        anyhow::bail!("Failed to discover public IP from any service")
    }

    fn fetch_public_ip(&self, url: &str) -> Result<Ipv4Addr> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let response = client.get(url).send()?.text()?;
        let ip_str = response.trim();
        
        ip_str
            .parse::<Ipv4Addr>()
            .context(format!("Invalid IP response: {}", ip_str))
    }

    /// Refresh the port mapping and check for IP changes
    pub fn refresh_mapping(&mut self) -> Result<()> {
        if let Some(gateway) = &self.gateway {
            if let Some(mut info) = self.get_external_info() {
                // Check if public IP changed
                match self.discover_public_ip() {
                    Ok(new_ip) => {
                        if new_ip != info.public_ip {
                            log::info!("Public IP changed: {} -> {}", info.public_ip, new_ip);
                            info.public_ip = new_ip;
                            *self.external_info.write().unwrap() = Some(info.clone());
                        }
                    }
                    Err(e) => log::warn!("Failed to check public IP during refresh: {}", e),
                }

                let local_ip = self.get_local_ip()?;
                let local_addr = SocketAddrV4::new(local_ip, self.local_port);

                gateway.add_port(
                    igd::PortMappingProtocol::TCP,
                    info.external_port,
                    local_addr,
                    3600,
                    "Lucidity Terminal",
                )?;

                log::debug!("Refreshed UPnP mapping");
            }
        }
        Ok(())
    }

    /// Remove the port mapping (call on shutdown)
    pub fn cleanup(&self) {
        if let Some(gateway) = &self.gateway {
            if let Some(info) = self.get_external_info() {
                if let Err(e) = gateway.remove_port(igd::PortMappingProtocol::TCP, info.external_port) {
                    log::warn!("Failed to remove UPnP mapping: {}", e);
                } else {
                    log::info!("Removed UPnP port mapping");
                }
            }
        }
    }
}

impl Drop for P2PConnectivity {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Simple random u16 for port selection
fn rand_u16() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos % 65536) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_ip_discovery() {
        let p2p = P2PConnectivity::new(9797);
        let ip = p2p.get_local_ip();
        // This should work on any machine with network access
        assert!(ip.is_ok(), "Failed to get local IP: {:?}", ip);
        let ip = ip.unwrap();
        assert!(!ip.is_loopback(), "Got loopback address");
    }
}
