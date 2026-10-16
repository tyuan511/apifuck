use apifuck_core::{
    Collection, Document, NetworkSettings, execute, prepare, tls::ClientCertificate, websocket,
};
use futures_util::{SinkExt, StreamExt};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use rustls::{RootCertStore, ServerConfig, pki_types::PrivateKeyDer, server::WebPkiClientVerifier};
use serde_json::json;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct Certificates {
    dir: tempfile::TempDir,
    ca: Certificate,
    server: Certificate,
    server_key: KeyPair,
}
impl Certificates {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(vec![]).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];
        let ca = params.self_signed(&key).unwrap();
        let server_key = KeyPair::generate().unwrap();
        let mut params =
            CertificateParams::new(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let server = params.signed_by(&server_key, &ca, &key).unwrap();
        let client_key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(vec![]).unwrap();
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        let client = params.signed_by(&client_key, &ca, &key).unwrap();
        std::fs::write(dir.path().join("ca.pem"), ca.pem()).unwrap();
        std::fs::write(dir.path().join("client.pem"), client.pem()).unwrap();
        std::fs::write(
            dir.path().join("client-key.pem"),
            client_key.serialize_pem(),
        )
        .unwrap();
        Self {
            dir,
            ca,
            server,
            server_key,
        }
    }
    fn config(&self, required: bool) -> Arc<ServerConfig> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut roots = RootCertStore::empty();
        roots.add(self.ca.der().clone()).unwrap();
        let verifier =
            WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider.clone());
        let verifier = if required {
            verifier
        } else {
            verifier.allow_unauthenticated()
        }
        .build()
        .unwrap();
        Arc::new(
            ServerConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_client_cert_verifier(verifier)
                .with_single_cert(
                    vec![self.server.der().clone()],
                    PrivateKeyDer::try_from(self.server_key.serialize_der()).unwrap(),
                )
                .unwrap(),
        )
    }
    fn network(&self) -> NetworkSettings {
        let mut settings = NetworkSettings::default();
        settings.proxy = Some(String::new());
        settings.timeout_ms = 5000;
        settings.connect_timeout_ms = 2000;
        settings.custom_ca_path = Some(self.dir.path().join("ca.pem"));
        settings.client_certificates = vec![ClientCertificate {
            domain: "localhost".into(),
            certificate_file: self.dir.path().join("client.pem"),
            private_key_file: self.dir.path().join("client-key.pem"),
            disabled: false,
        }];
        settings
    }
}
struct LocalServer {
    port: u16,
    peers: Arc<Mutex<Vec<bool>>>,
    job: tokio::task::JoinHandle<()>,
}
impl Drop for LocalServer {
    fn drop(&mut self) {
        self.job.abort();
    }
}
async fn http_server(config: Arc<ServerConfig>, response: String) -> LocalServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let peers = Arc::new(Mutex::new(vec![]));
    let captured = peers.clone();
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    let job = tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                break;
            };
            let acceptor = acceptor.clone();
            let response = response.clone();
            let peers = captured.clone();
            tokio::spawn(async move {
                let Ok(mut stream) = acceptor.accept(socket).await else {
                    return;
                };
                let authenticated = stream
                    .get_ref()
                    .1
                    .peer_certificates()
                    .is_some_and(|certs| !certs.is_empty());
                let mut bytes = vec![];
                let mut chunk = [0; 1024];
                loop {
                    let Ok(count) = stream.read(&mut chunk).await else {
                        return;
                    };
                    if count == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                        break;
                    }
                    if bytes.len() > 16384 {
                        return;
                    }
                }
                peers.lock().unwrap().push(authenticated);
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    LocalServer { port, peers, job }
}
fn request(c: &Collection, url: &str) -> Document {
    c.add(
        Path::new("test.yml"),
        &serde_yaml::to_string(
            &json!({"info":{"name":"TLS","type":"http"},"http":{"method":"GET","url":url}}),
        )
        .unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn custom_ca_and_client_identity_are_required_and_domain_scoped() {
    let certs = Certificates::new();
    let server = http_server(
        certs.config(true),
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".into(),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "TLS").unwrap();
    let doc = request(&c, &format!("https://localhost:{}/", server.port));
    let mut network = certs.network();
    network.custom_ca_path = None;
    assert!(
        execute(prepare(&c, &doc, None, network).unwrap(), None)
            .await
            .is_err()
    );
    let mut network = certs.network();
    network.client_certificates.clear();
    assert!(
        execute(prepare(&c, &doc, None, network).unwrap(), None)
            .await
            .is_err()
    );
    let mut network = certs.network();
    network.client_certificates[0].domain = "elsewhere.test".into();
    assert!(
        execute(prepare(&c, &doc, None, network).unwrap(), None)
            .await
            .is_err()
    );
    let response = execute(prepare(&c, &doc, None, certs.network()).unwrap(), None)
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(*server.peers.lock().unwrap(), vec![true]);
}
#[tokio::test]
async fn redirect_reselects_identity_without_sending_it_to_another_host() {
    let certs = Certificates::new();
    let target = http_server(
        certs.config(false),
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".into(),
    )
    .await;
    let first=http_server(certs.config(true),format!("HTTP/1.1 302 Found\r\nLocation: https://127.0.0.1:{}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",target.port)).await;
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "TLS").unwrap();
    let doc = request(&c, &format!("https://localhost:{}/", first.port));
    let response = execute(prepare(&c, &doc, None, certs.network()).unwrap(), None)
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(*first.peers.lock().unwrap(), vec![true]);
    assert_eq!(*target.peers.lock().unwrap(), vec![false]);
}
#[tokio::test]
async fn collection_relative_certificates_and_custom_ca_work_for_secure_websockets() {
    let certs = Certificates::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let acceptor = tokio_rustls::TlsAcceptor::from(certs.config(true));
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let stream = acceptor.accept(socket).await.unwrap();
        assert!(!stream.get_ref().1.peer_certificates().unwrap().is_empty());
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                "secure".into(),
            ))
            .await
            .unwrap();
        while let Some(Ok(message)) = socket.next().await {
            if message.is_close() {
                break;
            }
        }
    });
    let dir = tempfile::tempdir().unwrap();
    for name in ["ca.pem", "client.pem", "client-key.pem"] {
        std::fs::copy(certs.dir.path().join(name), dir.path().join(name)).unwrap();
    }
    let c = Collection::create(dir.path(), "TLS").unwrap();
    let mut root = c.project().value().clone();
    root["config"] = json!({"clientCertificates":[{"domain":"localhost","type":"pem","certificateFilePath":"client.pem","privateKeyFilePath":"client-key.pem"}]});
    root["extensions"]["apifuck"]["customCaCertificate"] = json!("ca.pem");
    c.save(c.project(), &serde_yaml::to_string(&root).unwrap())
        .unwrap();
    let c = Collection::open(c.root()).unwrap();
    let doc=c.add(Path::new("socket.yml"),&serde_yaml::to_string(&json!({"info":{"name":"Secure socket","type":"websocket"},"websocket":{"url":format!("wss://localhost:{port}/")}})).unwrap()).unwrap();
    let mut network = NetworkSettings::default();
    network.proxy = Some(String::new());
    let execution = prepare(&c, &doc, None, network).unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let (events, mut incoming) = tokio::sync::mpsc::unbounded_channel();
    let job = tokio::spawn(websocket::connect(execution, rx, events));
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = incoming.recv().await {
            if let apifuck_core::Event::Message { text, .. } = event {
                assert_eq!(text, "secure");
                tx.send(websocket::Command::Close).unwrap();
                return;
            }
        }
        panic!("missing message");
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), job)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    server.await.unwrap();
}

#[test]
fn environment_certificate_precedence_resolves_static_paths_and_preserves_global_fallbacks() {
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "TLS").unwrap();
    let certificate = |domain: &str| json!({"domain":domain,"type":"pem","certificateFilePath":"{{cert_path}}","privateKeyFilePath":"key.pem"});
    c.add(Path::new("environments/parent.yml"),&serde_yaml::to_string(&json!({"name":"parent","variables":[{"name":"cert_path","value":"client.pem"}],"clientCertificates":[certificate("parent.test")]})).unwrap()).unwrap();
    c.add(Path::new("environments/dev.yml"),&serde_yaml::to_string(&json!({"name":"dev","extends":"parent","clientCertificates":[certificate("dev.test")]})).unwrap()).unwrap();
    let doc = request(&c, "https://dev.test/");
    let c = Collection::open(c.root()).unwrap();
    let mut settings = NetworkSettings::default();
    settings.client_certificates.push(ClientCertificate {
        domain: "global.test".into(),
        certificate_file: dir.path().join("global.pem"),
        private_key_file: dir.path().join("key.pem"),
        disabled: false,
    });
    let execution = prepare(&c, &doc, Some("dev"), settings).unwrap();
    let certificates = &execution.network().client_certificates;
    assert_eq!(
        certificates
            .iter()
            .map(|cert| cert.domain.as_str())
            .collect::<Vec<_>>(),
        vec!["dev.test", "parent.test", "global.test"]
    );
    assert_eq!(
        certificates[0].certificate_file,
        c.root().join("client.pem")
    );
}
