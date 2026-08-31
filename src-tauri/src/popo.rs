//! Direct PoPo integration over its own wire protocol (LocalSend v2). No PoPo
//! HTTP API, no crate coupling — Mosaic discovers PoPo on the LAN via the
//! `/register` handshake, then sends a file with prepare-upload + upload. Field
//! names match PoPo's `http::dto` (note: `token`, protocol `"HTTP"`).

use crate::model::PopoPeer;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::sync::mpsc;
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 53317;
const PROTOCOL_VERSION: &str = "2.1";

fn agent(timeout_ms: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
}

fn register_body(alias: &str, fingerprint: &str) -> String {
    serde_json::json!({
        "alias": alias,
        "version": PROTOCOL_VERSION,
        "deviceType": "DESKTOP",
        "token": fingerprint,
        "port": DEFAULT_PORT,
        "protocol": "HTTP",
    })
    .to_string()
}

/// Primary local IPv4, found via a UDP connect trick (no packet is actually sent).
fn local_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v4) => Some(v4),
        IpAddr::V6(_) => None,
    }
}

/// Subnet-scan the local /24 over LocalSend `/register` to find PoPo peers. This
/// is PoPo's own VPN-resilient discovery path (multicast is often dropped).
pub fn scan(alias: &str, fingerprint: &str) -> Vec<PopoPeer> {
    let base = match local_ipv4() {
        Some(v) => v,
        None => return vec![],
    };
    let o = base.octets();
    let body = register_body(alias, fingerprint);
    let (tx, rx) = mpsc::channel::<PopoPeer>();

    let hosts: Vec<u8> = (1..=254u8)
        .filter(|h| Ipv4Addr::new(o[0], o[1], o[2], *h) != base)
        .collect();
    // Bounded concurrency: probe in batches so we don't spawn 254 threads at once.
    for chunk in hosts.chunks(40) {
        let mut batch = vec![];
        for &h in chunk {
            let ip = Ipv4Addr::new(o[0], o[1], o[2], h);
            let body = body.clone();
            let own = fingerprint.to_string();
            let tx = tx.clone();
            batch.push(std::thread::spawn(move || {
                if let Some(p) = probe(ip, &body, &own) {
                    let _ = tx.send(p);
                }
            }));
        }
        for h in batch {
            let _ = h.join();
        }
    }
    drop(tx);

    let mut peers: Vec<PopoPeer> = rx.iter().collect();
    peers.sort_by(|a, b| a.ip.cmp(&b.ip));
    peers.dedup_by(|a, b| a.fingerprint == b.fingerprint);
    peers
}

fn probe(ip: Ipv4Addr, body: &str, own_fp: &str) -> Option<PopoPeer> {
    let url = format!("http://{}:{}/api/localsend/v2/register", ip, DEFAULT_PORT);
    let resp = agent(1200)
        .post(&url)
        .set("content-type", "application/json")
        .send_string(body)
        .ok()?;
    let v: serde_json::Value = resp.into_json().ok()?;
    let alias = v.get("alias")?.as_str()?.to_string();
    let fp = v
        .get("token")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("fingerprint").and_then(|x| x.as_str()))
        .unwrap_or("")
        .to_string();
    if fp.is_empty() || fp == own_fp {
        return None;
    }
    Some(PopoPeer {
        ip: ip.to_string(),
        port: DEFAULT_PORT,
        alias,
        fingerprint: fp,
    })
}

/// Send one file to a peer via LocalSend v2 prepare-upload + upload.
pub fn send_file(
    peer: &PopoPeer,
    alias: &str,
    fingerprint: &str,
    file_path: &str,
) -> Result<(), String> {
    let bytes = std::fs::read(file_path).map_err(|e| format!("读取文件失败: {}", e))?;
    let total = bytes.len() as u64;
    let sha = sha256_hex(&bytes);
    let file_name = std::path::Path::new(file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let base = format!("http://{}:{}/api/localsend/v2", peer.ip, peer.port);

    let prepare = serde_json::json!({
        "info": {
            "alias": alias,
            "version": PROTOCOL_VERSION,
            "deviceType": "DESKTOP",
            "token": fingerprint,
            "port": DEFAULT_PORT,
            "protocol": "HTTP",
        },
        "files": {
            "file-1": {
                "id": "file-1",
                "fileName": file_name,
                "size": total,
                "fileType": "application/octet-stream",
                "sha256": sha,
            }
        }
    });

    let prep: serde_json::Value = agent(8000)
        .post(&format!("{}/prepare-upload", base))
        .send_json(prepare)
        .map_err(popo_err)?
        .into_json()
        .map_err(|e| e.to_string())?;

    let session = prep
        .get("sessionId")
        .and_then(|x| x.as_str())
        .ok_or("PoPo 未返回 sessionId（可能在 PoPo 里拒绝了接收）")?;
    let token = prep
        .pointer("/files/file-1")
        .and_then(|x| x.as_str())
        .ok_or("PoPo 未接受该文件")?;

    let url = format!(
        "{}/upload?sessionId={}&fileId=file-1&token={}",
        base, session, token
    );
    agent(60000)
        .post(&url)
        .send_bytes(&bytes)
        .map_err(popo_err)?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

fn popo_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, _) => format!("PoPo 返回 {}（在 PoPo 里确认允许接收）", code),
        _ => format!("连不上 PoPo：{}（确认 PoPo 在运行、与本机同一局域网）", e),
    }
}
