//! Opening the host's port on its router (D8): UPnP IGD's port mapping, asked of the
//! router that answers on the network. Routers with UPnP turned off, and networks behind
//! a carrier's NAT, don't map; the join code's STUN address and punching are the way in
//! then.
//!
//! Every call waits on the network for up to a few seconds, so make them off the frame
//! loop.

use igd_next::{Gateway, PortMappingProtocol, SearchOptions, search_gateway};
use std::{net::SocketAddr, time::Duration};

/// How long the search for a router waits for answers.
pub const SEARCH: Duration = Duration::from_secs(3);
/// How long a mapping lasts unless removed first, so one left by a crash doesn't linger.
pub const LEASE: Duration = Duration::from_secs(2 * 60 * 60);
const DESCRIPTION: &str = "Open Ratchet & Clank Engine co-op";

#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    #[error("no router with UPnP answered: {0}")]
    Search(#[from] igd_next::SearchError),
    #[error("the router didn't map the port: {0}")]
    Add(#[from] igd_next::AddPortError),
    #[error("the router didn't say its address: {0}")]
    ExternalIp(#[from] igd_next::GetExternalIpError),
}

/// A UDP port the router forwards to this machine.
pub struct Mapping {
    gateway: Gateway,
    /// Where the other peer reaches this machine through the router.
    pub external: SocketAddr,
}

/// Asks the router to forward UDP `local`'s port, on its own address, to `local`: this
/// machine's address on the network ([`crate::code::local_address`]).
pub fn map_port(local: SocketAddr) -> Result<Mapping, MappingError> {
    let mut options = SearchOptions::default();
    options.timeout = Some(SEARCH);
    options.single_search_timeout = Some(SEARCH);
    let gateway = search_gateway(options)?;
    gateway.add_port(
        PortMappingProtocol::UDP,
        local.port(),
        local,
        LEASE.as_secs() as u32,
        DESCRIPTION,
    )?;
    let external = SocketAddr::new(gateway.get_external_ip()?, local.port());
    Ok(Mapping { gateway, external })
}

impl Mapping {
    /// The router's own address.
    pub fn router(&self) -> SocketAddr {
        self.gateway.addr
    }

    /// Asks the router to stop forwarding the port.
    pub fn remove(self) -> Result<(), igd_next::RemovePortError> {
        self.gateway
            .remove_port(PortMappingProtocol::UDP, self.external.port())
    }
}
