//! HTTPS for the asset service without a certificate authority
//! (docs/plans/active/asset-service.md, "Playtest delivery"): the server
//! makes its own self-signed certificate once (`Identity`, Ed25519,
//! `<server store>/server/tls/`), and a client trusts exactly that
//! certificate by its SHA-256 fingerprint (`fingerprint`), which a game
//! server hands out at join and the client pins. Names, dates and
//! authorities don't enter into it: a public IP that changes keeps the same
//! certificate, and no other certificate (an attacker's, a real CA's) is
//! accepted where a fingerprint is pinned.
//!
//! - **The server side** (`Front`): a TLS listener (rustls) in front of the
//!   plain HTTP service on 127.0.0.1. Each accepted connection does its
//!   handshake (10 s at most) and is then relayed, decrypted, to the
//!   service over a loopback connection; `Peers` maps that loopback
//!   connection back to the client's address, for the access log and the
//!   per-address limits (`FrontLimits`: connections at once per address
//!   and in all, an idle timeout).
//! - **The client side** (`PinnedTls`): a ureq connector that wraps the
//!   TCP connection in rustls with a verifier that accepts only the pinned
//!   certificate (the handshake's signature is still checked against that
//!   certificate's key, so a copy of the certificate without the key gets
//!   nowhere).
//!
//! The certificate is a minimal X.509 v3 written here (`self_signed`):
//! version, serial, Ed25519 algorithm, a fixed name, a validity from 2025
//! to 9999, the public key; no extensions (no name to check).

use std::{
    collections::HashMap,
    fmt,
    io::{self, Read, Write},
    net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use ed25519_dalek::{Signer, SigningKey};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, ServerConfig, ServerConnection, SignatureScheme,
    StreamOwned,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::CryptoProvider,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
};
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, Either, LazyBuffers, NextTimeout, Transport, TransportAdapter,
};

use crate::store::{hex, sha256_hex, unhex, write_atomic};

/// The certificate's file in a server's TLS folder (DER).
pub const CERT_FILE: &str = "cert.der";
/// The private key's file (PKCS#8 DER; mode 0600 on Unix).
pub const KEY_FILE: &str = "key.der";

/// The server's certificate and key.
#[derive(Clone)]
pub struct Identity {
    pub cert: Vec<u8>,
    key: Vec<u8>,
}

impl fmt::Debug for Identity {
    // Never the key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

impl Identity {
    /// The certificate in `dir` (`CERT_FILE`, `KEY_FILE`), made there on
    /// first use.
    pub fn load_or_create(dir: &Path) -> io::Result<Self> {
        let (cert_path, key_path) = (dir.join(CERT_FILE), dir.join(KEY_FILE));
        if let (Ok(cert), Ok(key)) = (std::fs::read(&cert_path), std::fs::read(&key_path)) {
            return Ok(Self { cert, key });
        }
        let mut seed = [0u8; 32];
        let mut serial = [0u8; 16];
        getrandom::fill(&mut seed).map_err(|e| io::Error::other(format!("no randomness: {e}")))?;
        getrandom::fill(&mut serial).map_err(|e| io::Error::other(format!("no randomness: {e}")))?;
        let identity = Self::from_seed(&seed, serial);
        std::fs::create_dir_all(dir)?;
        write_atomic(&key_path, &identity.key)?;
        crate::sign::restrict(&key_path);
        write_atomic(&cert_path, &identity.cert)?;
        Ok(identity)
    }

    /// The certificate of the Ed25519 key `seed` (tests: a fixed one).
    pub fn from_seed(seed: &[u8; 32], serial: [u8; 16]) -> Self {
        let key = SigningKey::from_bytes(seed);
        Self {
            cert: self_signed(&key, serial),
            key: pkcs8(seed),
        }
    }

    /// SHA-256 of the certificate, hex: what clients pin.
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.cert)
    }

    /// rustls' server configuration for this certificate.
    pub fn server_config(&self) -> io::Result<Arc<ServerConfig>> {
        let config = ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(io::Error::other)?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(self.cert.clone())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone())),
            )
            .map_err(io::Error::other)?;
        Ok(Arc::new(config))
    }
}

/// The fingerprint of the certificate in a server's TLS folder, if it has
/// one (a game server on the same machine as the asset service reads it).
pub fn fingerprint_in(dir: &Path) -> Option<String> {
    std::fs::read(dir.join(CERT_FILE)).ok().map(|c| fingerprint(&c))
}

/// SHA-256 of a certificate's DER bytes, hex (lower case).
pub fn fingerprint(cert: &[u8]) -> String {
    sha256_hex(cert)
}

/// A fingerprint as people may write it (hex, any case, with or without
/// `:` between bytes, a `sha256:` prefix) in our form; None if it isn't
/// 32 bytes of hex.
pub fn normalize_fingerprint(text: &str) -> Option<String> {
    let text = text.trim();
    let text = text
        .strip_prefix("sha256:")
        .or_else(|| text.strip_prefix("SHA256:"))
        .unwrap_or(text);
    let hexed: String = text
        .chars()
        .filter(|c| *c != ':')
        .collect::<String>()
        .to_ascii_lowercase();
    unhex::<32>(&hexed).map(|b| hex(&b))
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

// --- The certificate, in DER.

fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let n = content.len();
    if n < 0x80 {
        out.push(n as u8);
    } else if n < 0x100 {
        out.extend([0x81, n as u8]);
    } else {
        out.extend([0x82, (n >> 8) as u8, n as u8]);
    }
    out.extend_from_slice(content);
    out
}

fn seq(parts: &[&[u8]]) -> Vec<u8> {
    der(0x30, &parts.concat())
}

/// Ed25519's algorithm identifier (1.3.101.112).
fn ed25519_alg() -> Vec<u8> {
    seq(&[&der(0x06, &[0x2b, 0x65, 0x70])])
}

/// A minimal self-signed X.509 v3 certificate of `key`.
fn self_signed(key: &SigningKey, mut serial: [u8; 16]) -> Vec<u8> {
    // Positive and minimal: the top bit clear, the first byte nonzero.
    serial[0] = (serial[0] & 0x7f) | 0x40;
    let name = seq(&[&der(
        0x31,
        &seq(&[&der(0x06, &[0x55, 0x04, 0x03]), &der(0x0c, b"chimera asset service")]),
    )]);
    let validity = seq(&[&der(0x17, b"250101000000Z"), &der(0x18, b"99991231235959Z")]);
    let mut public = vec![0u8];
    public.extend_from_slice(key.verifying_key().as_bytes());
    let spki = seq(&[&ed25519_alg(), &der(0x03, &public)]);
    let tbs = seq(&[
        &der(0xa0, &der(0x02, &[2])),
        &der(0x02, &serial),
        &ed25519_alg(),
        &name,
        &validity,
        &name,
        &spki,
    ]);
    let mut signature = vec![0u8];
    signature.extend_from_slice(&key.sign(&tbs).to_bytes());
    seq(&[&tbs, &ed25519_alg(), &der(0x03, &signature)])
}

/// The PKCS#8 (v1, RFC 8410) form of an Ed25519 key.
fn pkcs8(seed: &[u8; 32]) -> Vec<u8> {
    let mut out = vec![
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
    ];
    out.extend_from_slice(seed);
    out
}

// --- The client side.

/// Accepts the one certificate whose SHA-256 is pinned.
#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let got = fingerprint(end_entity.as_ref());
        if got == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(format!(
                "the asset service's certificate isn't the one the game server named (fingerprint {} expected, {} \
                 offered)",
                short(&self.fingerprint),
                short(&got)
            )))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// The first 16 hex digits of a fingerprint (for messages).
pub fn short(fingerprint: &str) -> &str {
    &fingerprint[..fingerprint.len().min(16)]
}

/// rustls' client configuration trusting only the certificate `fingerprint`
/// (normalized).
pub fn pinned_client_config(fingerprint: &str) -> Result<Arc<ClientConfig>, String> {
    let fingerprint = normalize_fingerprint(fingerprint).ok_or("a certificate fingerprint is 64 hex digits")?;
    let provider = provider();
    let mut config = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned { fingerprint, provider }))
        .with_no_client_auth();
    // The name is no part of the check: send none.
    config.enable_sni = false;
    Ok(Arc::new(config))
}

/// A ureq connector: HTTPS to a server whose certificate is pinned
/// (`pinned_client_config`); plain HTTP passes through.
pub struct PinnedTls {
    config: Arc<ClientConfig>,
}

impl fmt::Debug for PinnedTls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PinnedTls")
    }
}

impl PinnedTls {
    pub fn new(fingerprint: &str) -> Result<Self, String> {
        Ok(Self {
            config: pinned_client_config(fingerprint)?,
        })
    }
}

impl<In: Transport> Connector<In> for PinnedTls {
    type Out = Either<In, PinnedTransport>;

    fn connect(&self, details: &ConnectionDetails, chained: Option<In>) -> Result<Option<Self::Out>, ureq::Error> {
        let Some(transport) = chained else {
            return Ok(None);
        };
        if !details.needs_tls() || transport.is_tls() {
            return Ok(Some(Either::A(transport)));
        }
        // Any name: the verifier checks the certificate alone, and SNI is off.
        let name = ServerName::try_from("chimera-asset-service").expect("a DNS name");
        let mut conn = ClientConnection::new(self.config.clone(), name)
            .map_err(|e| ureq::Error::Io(io::Error::other(e.to_string())))?;
        let mut sock = TransportAdapter::new(transport.boxed(), details.current_time.clone());
        sock.set_timeout(details.timeout);
        while conn.is_handshaking() {
            conn.complete_io(&mut sock)?;
        }
        let buffers = LazyBuffers::new(details.config.input_buffer_size(), details.config.output_buffer_size());
        Ok(Some(Either::B(PinnedTransport {
            buffers,
            stream: StreamOwned::new(conn, sock),
        })))
    }
}

/// A connection made by `PinnedTls`.
pub struct PinnedTransport {
    buffers: LazyBuffers,
    stream: StreamOwned<ClientConnection, TransportAdapter>,
}

impl fmt::Debug for PinnedTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PinnedTransport")
    }
}

impl Transport for PinnedTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.stream.get_mut().set_timeout(timeout);
        let output = &self.buffers.output()[..amount];
        self.stream.write_all(output)?;
        Ok(())
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.stream.get_mut().set_timeout(timeout);
        let input = self.buffers.input_append_buf();
        let amount = self.stream.read(input)?;
        self.buffers.input_appended(amount);
        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        self.stream.get_mut().get_mut().is_open()
    }

    fn is_tls(&self) -> bool {
        true
    }
}

// --- The server side.

/// Loopback connection (the front's end, as the service sees it) -> the
/// client's address.
pub type Peers = Arc<Mutex<HashMap<SocketAddr, IpAddr>>>;

/// What the TLS front lets in.
#[derive(Clone, Copy, Debug)]
pub struct FrontLimits {
    /// Connections at once from one address.
    pub per_address: usize,
    /// Connections at once in all.
    pub total: usize,
    /// A handshake takes at most this long.
    pub handshake: Duration,
    /// A connection quiet this long is closed.
    pub idle: Duration,
}

impl Default for FrontLimits {
    fn default() -> Self {
        Self {
            // Testers behind one home address share it, and one game
            // holds several (the join's four, the prefetch, files on
            // demand, kept open between requests): 16 refused the third of
            // three games joining from one address (2026-11-11 rehearsal).
            per_address: 48,
            total: 256,
            handshake: Duration::from_secs(10),
            idle: Duration::from_secs(120),
        }
    }
}

/// A running TLS front; stopped when dropped (connections under way end
/// on their own).
pub struct Front {
    pub addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Front {
    /// `https://<addr>`.
    pub fn url(&self) -> String {
        format!("https://{}", self.addr)
    }

    /// Serve until the process ends.
    pub fn wait(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Front {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Wake the accept loop.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Accept TLS on `listen` and relay each connection to the plain service
/// at `backend`, noting who each relayed connection is in `peers`.
pub fn front(
    listen: &str,
    backend: SocketAddr,
    identity: &Identity,
    peers: Peers,
    limits: FrontLimits,
) -> io::Result<Front> {
    let config = identity.server_config()?;
    let listener = TcpListener::bind(listen).map_err(|e| io::Error::new(e.kind(), format!("{listen}: {e}")))?;
    let addr = listener.local_addr()?;
    let stop = Arc::new(AtomicBool::new(false));
    let counts: Arc<Mutex<HashMap<IpAddr, usize>>> = Arc::default();
    let total = Arc::new(AtomicUsize::new(0));
    let stop2 = stop.clone();
    let thread = std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stop2.load(Ordering::Relaxed) {
                break;
            }
            let Ok(stream) = stream else { continue };
            let Ok(peer) = stream.peer_addr() else { continue };
            let ip = peer.ip();
            {
                let mut c = counts.lock().unwrap_or_else(|e| e.into_inner());
                let n = c.entry(ip).or_default();
                if *n >= limits.per_address || total.load(Ordering::Relaxed) >= limits.total {
                    eprintln!("tls: refused a connection from {ip}: too many at once");
                    continue;
                }
                *n += 1;
                total.fetch_add(1, Ordering::Relaxed);
            }
            let (config, peers, counts, total) = (config.clone(), peers.clone(), counts.clone(), total.clone());
            std::thread::spawn(move || {
                if let Err(e) = relay(stream, config, backend, &peers, limits)
                    && !matches!(
                        e.kind(),
                        io::ErrorKind::UnexpectedEof
                            | io::ErrorKind::ConnectionReset
                            // A connection quiet past the idle timeout.
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                    )
                {
                    eprintln!("tls: {ip}: {e}");
                }
                let mut c = counts.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(n) = c.get_mut(&ip) {
                    *n -= 1;
                    if *n == 0 {
                        c.remove(&ip);
                    }
                }
                total.fetch_sub(1, Ordering::Relaxed);
            });
        }
    });
    Ok(Front {
        addr,
        stop,
        thread: Some(thread),
    })
}

/// One client: the handshake, then bytes both ways until either side ends.
fn relay(
    mut client: TcpStream,
    config: Arc<ServerConfig>,
    backend: SocketAddr,
    peers: &Peers,
    limits: FrontLimits,
) -> io::Result<()> {
    let ip = client.peer_addr()?.ip();
    let _ = client.set_nodelay(true);
    client.set_read_timeout(Some(limits.handshake))?;
    client.set_write_timeout(Some(limits.handshake))?;
    let mut conn = ServerConnection::new(config).map_err(io::Error::other)?;
    while conn.is_handshaking() {
        conn.complete_io(&mut client)?;
    }
    client.set_read_timeout(Some(limits.idle))?;
    client.set_write_timeout(Some(limits.idle))?;
    // What the client sent right behind its handshake may be read already
    // (a request in the same segment as its Finished): forwarded first.
    let mut early = Vec::new();
    let state = conn
        .process_new_packets()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if state.plaintext_bytes_to_read() > 0 {
        early.resize(state.plaintext_bytes_to_read(), 0);
        conn.reader().read_exact(&mut early)?;
    }
    while conn.wants_write() {
        conn.write_tls(&mut client)?;
    }
    let mut back = TcpStream::connect(backend)?;
    let _ = back.set_nodelay(true);
    let local = back.local_addr()?;
    peers.lock().unwrap_or_else(|e| e.into_inner()).insert(local, ip);
    if !early.is_empty() {
        back.write_all(&early)?;
    }
    let result = pump(client, back, conn);
    peers.lock().unwrap_or_else(|e| e.into_inner()).remove(&local);
    result
}

fn pump(client: TcpStream, back: TcpStream, conn: ServerConnection) -> io::Result<()> {
    let conn = Arc::new(Mutex::new(conn));
    let (mut client_in, mut back_out) = (client.try_clone()?, back.try_clone()?);
    // The service's answers, encrypted, to the client.
    let down = {
        let (conn, mut client_out, mut back_in) = (conn.clone(), client.try_clone()?, back.try_clone()?);
        std::thread::spawn(move || -> io::Result<()> {
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = match back_in.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => 0,
                };
                let mut c = conn.lock().unwrap_or_else(|e| e.into_inner());
                if n == 0 {
                    c.send_close_notify();
                } else {
                    c.writer().write_all(&buf[..n])?;
                }
                while c.wants_write() {
                    c.write_tls(&mut client_out)?;
                }
                if n == 0 {
                    let _ = client_out.shutdown(Shutdown::Write);
                    return Ok(());
                }
            }
        })
    };
    // The client's requests, decrypted, to the service.
    let mut raw = vec![0u8; 64 * 1024];
    let mut plain = Vec::new();
    let up = (|| -> io::Result<()> {
        loop {
            let n = match client_in.read(&mut raw) {
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            if n == 0 {
                return Ok(());
            }
            let mut rest = &raw[..n];
            plain.clear();
            let mut closed = false;
            {
                let mut c = conn.lock().unwrap_or_else(|e| e.into_inner());
                while !rest.is_empty() {
                    c.read_tls(&mut rest)?;
                    let state = c.process_new_packets().map_err(|e| {
                        // Tell the client why before giving up.
                        let _ = c.write_tls(&mut client_in);
                        io::Error::new(io::ErrorKind::InvalidData, e)
                    })?;
                    let available = state.plaintext_bytes_to_read();
                    if available > 0 {
                        let at = plain.len();
                        plain.resize(at + available, 0);
                        c.reader().read_exact(&mut plain[at..])?;
                    }
                    closed |= state.peer_has_closed();
                }
                while c.wants_write() {
                    c.write_tls(&mut client_in)?;
                }
            }
            if !plain.is_empty() {
                back_out.write_all(&plain)?;
            }
            if closed {
                return Ok(());
            }
        }
    })();
    // The client is done sending: the service sees the end of its requests,
    // answers what is under way and closes.
    let _ = back_out.shutdown(Shutdown::Write);
    let down = down
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("relay thread panicked")));
    let _ = client.shutdown(Shutdown::Both);
    up.and(down)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_are_normalized() {
        let f = "ab".repeat(32);
        assert_eq!(normalize_fingerprint(&f).as_deref(), Some(f.as_str()));
        let colons = vec!["AB"; 32].join(":");
        assert_eq!(normalize_fingerprint(&colons).as_deref(), Some(f.as_str()));
        assert_eq!(
            normalize_fingerprint(&format!("sha256:{f}")).as_deref(),
            Some(f.as_str())
        );
        assert_eq!(normalize_fingerprint("abcd"), None);
        assert_eq!(normalize_fingerprint(&"zz".repeat(32)), None);
    }

    #[test]
    fn the_certificate_parses_and_its_key_matches() {
        let id = Identity::from_seed(&[7; 32], [0xff; 16]);
        // rustls takes the pair (it checks the key against the certificate).
        id.server_config().unwrap();
        assert_eq!(id.fingerprint().len(), 64);
        let again = Identity::from_seed(&[7; 32], [0xff; 16]);
        assert_eq!(id.cert, again.cert, "deterministic for a seed and serial");
    }
}
