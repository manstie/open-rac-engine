//! Opening the host's port on its router (D8), by whichever of PCP, NAT-PMP and UPnP IGD it
//! speaks, asked in that order: PCP and NAT-PMP of the default route's gateway
//! ([`crate::pcp`]), which answers within moments or not at all, then UPnP of whichever
//! router answers its search ([`crate::upnp`]). Routers with all three turned off, and
//! networks behind a carrier's NAT, don't map; the join code's STUN address and punching
//! are the way in then.
//!
//! Every call waits on the network for up to several seconds, so make them off the frame
//! loop.

use std::{
    fmt,
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use crate::{pcp, upnp};

/// How long a mapping lasts unless removed first, so one left by a crash doesn't linger.
pub const LEASE: Duration = Duration::from_secs(2 * 60 * 60);

/// How a router was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Pcp,
    NatPmp,
    Upnp,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Protocol::Pcp => "PCP",
            Protocol::NatPmp => "NAT-PMP",
            Protocol::Upnp => "UPnP",
        })
    }
}

/// A UDP port the router forwards to this machine.
pub struct Mapping {
    how: How,
}

enum How {
    Pcp(pcp::PcpMapping),
    Upnp(upnp::Mapping),
}

/// Why no router forwards the port: what each way of asking met.
#[derive(Debug, thiserror::Error)]
#[error("{}", .0.join("; "))]
pub struct Unmapped(pub Vec<String>);

#[derive(Debug, thiserror::Error)]
pub enum UnmapError {
    #[error(transparent)]
    Pcp(#[from] pcp::PcpError),
    #[error(transparent)]
    Upnp(#[from] igd_next::RemovePortError),
}

/// Asks the router to forward UDP `local`'s port, on its own address, to `local`: this
/// machine's address on the network ([`crate::code::local_address`]).
pub fn map_port(local: SocketAddr) -> Result<Mapping, Unmapped> {
    let mut failures = Vec::new();
    match default_gateway() {
        Some(gateway) => {
            let server = SocketAddr::from((gateway, pcp::SERVER_PORT));
            match pcp::map(server, local, LEASE) {
                Ok(mapped) => {
                    return Ok(Mapping {
                        how: How::Pcp(mapped),
                    });
                }
                Err(error) => failures.push(format!("PCP and NAT-PMP: {error}")),
            }
        }
        None => failures.push("PCP and NAT-PMP: the default gateway isn't known".to_owned()),
    }
    match upnp::map_port(local) {
        Ok(mapped) => Ok(Mapping {
            how: How::Upnp(mapped),
        }),
        Err(error) => {
            failures.push(format!("UPnP: {error}"));
            Err(Unmapped(failures))
        }
    }
}

impl Mapping {
    /// Where the other peer reaches this machine through the router.
    pub fn external(&self) -> SocketAddr {
        match &self.how {
            How::Pcp(mapped) => mapped.external,
            How::Upnp(mapped) => mapped.external,
        }
    }

    /// The router's own address.
    pub fn router(&self) -> SocketAddr {
        match &self.how {
            How::Pcp(mapped) => mapped.server,
            How::Upnp(mapped) => mapped.router(),
        }
    }

    pub fn protocol(&self) -> Protocol {
        match &self.how {
            How::Pcp(mapped) => mapped.protocol(),
            How::Upnp(_) => Protocol::Upnp,
        }
    }

    /// Asks the router to stop forwarding the port.
    pub fn remove(self) -> Result<(), UnmapError> {
        match self.how {
            How::Pcp(mapped) => Ok(mapped.remove()?),
            How::Upnp(mapped) => Ok(mapped.remove()?),
        }
    }
}

/// The default route's gateway: the router PCP and NAT-PMP are asked of. Only Linux and
/// Windows, the platforms the game ships on, are asked; elsewhere UPnP's search finds the
/// router.
pub fn default_gateway() -> Option<Ipv4Addr> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/net/route")
            .ok()
            .and_then(|table| gateway_in_route_table(&table))
    }
    #[cfg(windows)]
    {
        windows_gateway()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        None
    }
}

/// The gateway of the default route with the lowest metric in Linux's `/proc/net/route`,
/// whose addresses are the kernel's 32-bit words in hexadecimal, in the machine's order.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn gateway_in_route_table(table: &str) -> Option<Ipv4Addr> {
    const GATEWAY_FLAG: u32 = 0x2;
    table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let hex = |at: usize| u32::from_str_radix(fields.get(at)?, 16).ok();
            let (destination, gateway, flags) = (hex(1)?, hex(2)?, hex(3)?);
            let metric: u32 = fields.get(6)?.parse().ok()?;
            (destination == 0 && flags & GATEWAY_FLAG != 0 && gateway != 0)
                .then(|| (metric, Ipv4Addr::from(gateway.to_ne_bytes())))
        })
        .min_by_key(|&(metric, _)| metric)
        .map(|(_, gateway)| gateway)
}

/// The next hop of Windows' best route to a public address.
#[cfg(windows)]
fn windows_gateway() -> Option<Ipv4Addr> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetBestRoute, MIB_IPFORWARDROW};
    let mut row = MIB_IPFORWARDROW::default();
    // Addresses are in network order, as their bytes lie in memory.
    let public = u32::from_ne_bytes([8, 8, 8, 8]);
    // SAFETY: `row` is a live, writable `MIB_IPFORWARDROW`, all the call writes.
    let status = unsafe { GetBestRoute(public, 0, &mut row) };
    (status == 0 && row.dwForwardNextHop != 0)
        .then(|| Ipv4Addr::from(row.dwForwardNextHop.to_ne_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_route_with_the_lowest_metric_gives_the_gateway() {
        let word = |ip: [u8; 4]| format!("{:08X}", u32::from_ne_bytes(ip));
        let table = format!(
            "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
             wlan0\t00000000\t{}\t0003\t0\t0\t600\t00000000\t0\t0\t0\n\
             eth0\t00000000\t{}\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
             eth0\t{}\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n",
            word([10, 0, 0, 1]),
            word([192, 168, 1, 1]),
            word([192, 168, 1, 0]),
        );
        assert_eq!(
            gateway_in_route_table(&table),
            Some(Ipv4Addr::new(192, 168, 1, 1))
        );
        assert_eq!(gateway_in_route_table("Iface\tDestination\n"), None);
    }
}
