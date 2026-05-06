//! Minimal TCP reachability check.
//!
//! NOT a NAT-traversal implementation -- we don't have one. This binary
//! just verifies whether two endpoints can reach each other over TCP,
//! independent of the game logic. Run it with a friend before kicking
//! off a real PvP match to confirm the network path works.
//!
//! ```text
//! # On the listening side:
//! cargo run -p antcolony-net --release --bin net_diag -- listen --port 17001
//!
//! # On the dialing side (use the listener's reachable address):
//! cargo run -p antcolony-net --release --bin net_diag -- dial 100.64.x.x:17001
//! ```
//!
//! The two sides exchange a 16-byte banner ("antcolony-diag\0\0") and
//! report timing. Failure messages are categorized so you know what's
//! actually broken: connection refused (no listener), timed out (NAT or
//! firewall ate the SYN), DNS failure (bad host), etc.

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

const BANNER: &[u8; 16] = b"antcolony-diag\0\0";
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: net_diag listen --port <PORT>");
        eprintln!("       net_diag dial <HOST:PORT>");
        eprintln!("       net_diag firewall --port <PORT>   # print Windows firewall rule for that port");
        return std::process::ExitCode::from(2);
    }
    match args[0].as_str() {
        "listen" => {
            let port = parse_port(&args).unwrap_or(17001);
            run_listen(port)
        }
        "dial" => {
            if args.len() < 2 {
                eprintln!("dial: expected target HOST:PORT");
                return std::process::ExitCode::from(2);
            }
            run_dial(&args[1])
        }
        "firewall" => {
            let port = parse_port(&args).unwrap_or(17001);
            run_firewall(port)
        }
        other => {
            eprintln!("unknown subcommand `{other}` (expected listen, dial, or firewall)");
            std::process::ExitCode::from(2)
        }
    }
}

fn parse_port(args: &[String]) -> Option<u16> {
    let mut i = 1;
    while i + 1 < args.len() {
        if args[i] == "--port" {
            return args[i + 1].parse().ok();
        }
        i += 1;
    }
    None
}

fn run_listen(port: u16) -> std::process::ExitCode {
    let bind = format!("0.0.0.0:{port}");
    let listener = match TcpListener::bind(&bind) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("FAIL: bind {bind}: {e}");
            eprintln!("hint: another process may already have port {port}, or you lack permission to bind it");
            return std::process::ExitCode::from(1);
        }
    };
    println!("[net_diag] listening on {bind}");
    println!("[net_diag] tell your peer to connect with one of:");
    print_local_hints(port);
    println!("[net_diag] waiting for one connection (Ctrl-C to abort)...");
    let (mut stream, peer) = match listener.accept() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("FAIL: accept: {e}");
            return std::process::ExitCode::from(1);
        }
    };
    println!("[net_diag] accepted from {peer}");
    if let Err(e) = stream.set_read_timeout(Some(IO_TIMEOUT)) {
        eprintln!("warn: set_read_timeout: {e}");
    }
    let started = Instant::now();
    let mut buf = [0u8; 16];
    if let Err(e) = stream.read_exact(&mut buf) {
        eprintln!("FAIL: read banner from peer: {e}");
        return std::process::ExitCode::from(1);
    }
    if &buf != BANNER {
        eprintln!("FAIL: banner mismatch (got {:?})", &buf);
        return std::process::ExitCode::from(1);
    }
    if let Err(e) = stream.write_all(BANNER) {
        eprintln!("FAIL: write banner back: {e}");
        return std::process::ExitCode::from(1);
    }
    println!(
        "[net_diag] OK -- banner exchange completed in {:.1}ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    std::process::ExitCode::SUCCESS
}

fn run_dial(target: &str) -> std::process::ExitCode {
    println!("[net_diag] dialing {target}");
    let addrs: Vec<SocketAddr> = match target.to_socket_addrs() {
        Ok(it) => it.collect(),
        Err(e) => {
            eprintln!("FAIL: name resolution for {target}: {e}");
            eprintln!("hint: bad hostname / DNS issue. Try a literal IP:port.");
            return std::process::ExitCode::from(1);
        }
    };
    if addrs.is_empty() {
        eprintln!("FAIL: name resolution for {target} returned no addresses");
        return std::process::ExitCode::from(1);
    }
    let addr = addrs[0];
    let started = Instant::now();
    let mut stream = match TcpStream::connect_timeout(&addr, DIAL_TIMEOUT) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FAIL: connect {addr}: {e} ({:?})", e.kind());
            print_connect_hint(e.kind(), addr);
            return std::process::ExitCode::from(1);
        }
    };
    let connect_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!("[net_diag] connected to {addr} in {connect_ms:.1}ms");
    if let Err(e) = stream.set_read_timeout(Some(IO_TIMEOUT)) {
        eprintln!("warn: set_read_timeout: {e}");
    }
    if let Err(e) = stream.write_all(BANNER) {
        eprintln!("FAIL: write banner: {e}");
        return std::process::ExitCode::from(1);
    }
    let mut buf = [0u8; 16];
    if let Err(e) = stream.read_exact(&mut buf) {
        eprintln!("FAIL: read banner echo: {e}");
        eprintln!("hint: peer connected but never responded -- different protocol, half-broken NAT, or process crashed");
        return std::process::ExitCode::from(1);
    }
    if &buf != BANNER {
        eprintln!("FAIL: banner mismatch (got {:?})", &buf);
        return std::process::ExitCode::from(1);
    }
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!("[net_diag] OK -- round-trip banner exchange in {total_ms:.1}ms (connect {connect_ms:.1}ms)");
    std::process::ExitCode::SUCCESS
}

/// Print the Windows firewall rule needed to allow inbound on the
/// chosen port, plus the Linux equivalent for Proton-GE / nix peers.
/// Doesn't actually run anything -- just echoes the commands so the
/// user can decide whether to apply them.
fn run_firewall(port: u16) -> std::process::ExitCode {
    println!("# Windows -- run once in PowerShell (UAC-elevated if UAC is on):");
    println!("New-NetFirewallRule -DisplayName \"antcolony PvP TCP {port}\" -Direction Inbound -Protocol TCP -LocalPort {port} -Action Allow -Profile Any");
    println!();
    println!("# Undo:");
    println!("Remove-NetFirewallRule -DisplayName \"antcolony PvP TCP {port}\"");
    println!();
    println!("# Linux (ufw):");
    println!("sudo ufw allow {port}/tcp");
    println!();
    println!("# Linux (iptables):");
    println!("sudo iptables -A INPUT -p tcp --dport {port} -j ACCEPT");
    println!();
    println!("Note: only the HOST needs this. Joiners just outbound-connect, which is");
    println!("almost never blocked by host firewalls.");
    std::process::ExitCode::SUCCESS
}

fn print_connect_hint(kind: std::io::ErrorKind, addr: SocketAddr) {
    use std::io::ErrorKind::*;
    match kind {
        ConnectionRefused => {
            eprintln!("hint: nothing is listening on {addr}. Did the host start net_diag listen first?");
        }
        TimedOut => {
            eprintln!("hint: SYN reached the network but no SYN-ACK came back. Likely causes:");
            eprintln!("       - host's router NAT/firewall blocking inbound on the port");
            eprintln!("       - wrong public IP (you need the host's WAN IP, not their LAN IP)");
            eprintln!("       - ISP-level CGNAT (some mobile / cell-modem ISPs)");
            eprintln!("       Workaround: have the host port-forward, OR use Tailscale / ZeroTier / Hamachi");
            eprintln!("       and connect to the VPN-issued address instead.");
        }
        HostUnreachable | NetworkUnreachable => {
            eprintln!("hint: route to {addr} doesn't exist. Bad IP, or VPN tunnel down.");
        }
        AddrNotAvailable => {
            eprintln!("hint: the local interface can't reach that target. Multi-homed config issue?");
        }
        _ => {
            eprintln!("hint: unmapped error kind. The OS message above is the best clue.");
        }
    }
}

/// Print useful local addresses without an external crate. We can't
/// enumerate interfaces from std, so we resolve our own hostname --
/// captures the primary LAN IP on most setups. Add Tailscale tip.
fn print_local_hints(port: u16) {
    let host = std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok());
    if let Some(h) = host {
        match (h.as_str(), port).to_socket_addrs() {
            Ok(it) => {
                let v: Vec<SocketAddr> = it.collect();
                if v.is_empty() {
                    println!("    (could not resolve own hostname `{h}`)");
                } else {
                    for a in v {
                        println!("    - {a}    (resolved from hostname `{h}`)");
                    }
                }
            }
            Err(e) => {
                println!("    (resolve {h} failed: {e})");
            }
        }
    }
    println!("    - <LAN IP>:{port}    (run `ipconfig` on Windows or `ip addr` on Linux to find)");
    println!("    - <Tailscale IP>:{port}    (run `tailscale ip` -- recommended for cross-internet play)");
    println!("    - <WAN IP>:{port}    (only works if router port-forwards {port} to this machine)");
    let _ = IpAddr::from([0, 0, 0, 0]); // keep IpAddr import meaningful
}
