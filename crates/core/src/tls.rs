//! Shared certificate loading and per-destination client identity selection.
use crate::{Collection, NetworkSettings, engine::interpolate};
use anyhow::{Context, Result, ensure};
use rustls::pki_types::CertificateDer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};
use url::Url;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClientCertificate {
    pub domain: String,
    pub certificate_file: PathBuf,
    pub private_key_file: PathBuf,
    pub disabled: bool,
}
fn pattern(domain: &str) -> Result<(String, Option<u16>, bool)> {
    let wildcard = domain.starts_with("*.");
    let raw = domain.strip_prefix("*.").unwrap_or(domain);
    ensure!(
        !raw.is_empty() && !raw.contains(['/', '?', '#', '@', '*']),
        "客户端证书域名无效：{domain}"
    );
    let url = Url::parse(&format!("https://{raw}")).context("客户端证书域名无效")?;
    let host = url.host_str().context("客户端证书缺少域名")?.to_owned();
    // url::Url canonicalizes an explicit :443 away; retain the user's port constraint.
    let port = if raw.starts_with('[') {
        raw.split_once("]:").map(|(_, port)| port)
    } else {
        raw.rsplit_once(':').map(|(_, port)| port)
    }
    .map(str::parse::<u16>)
    .transpose()?;
    Ok((host, port, wildcard))
}
impl ClientCertificate {
    pub fn validate(&self) -> Result<()> {
        if self.disabled {
            return Ok(());
        }
        pattern(&self.domain)?;
        ensure!(
            self.certificate_file.is_absolute() && self.private_key_file.is_absolute(),
            "应用客户端证书和私钥路径必须是绝对路径"
        );
        Ok(())
    }
    pub fn matches(&self, url: &Url) -> Result<bool> {
        if self.disabled {
            return Ok(false);
        }
        let (host, port, wildcard) = pattern(&self.domain)?;
        let target = url.host_str().unwrap_or("");
        let host_match = if wildcard {
            target
                .strip_suffix(&host)
                .is_some_and(|prefix| prefix.ends_with('.') && prefix.len() > 1)
        } else {
            target == host
        };
        Ok(host_match && port.is_none_or(|port| url.port_or_known_default() == Some(port)))
    }
}
fn file(path: &Path) -> Result<Vec<u8>> {
    ensure!(
        std::fs::metadata(path)
            .with_context(|| format!("无法读取证书文件：{}", path.display()))?
            .len()
            <= 4 * 1024 * 1024,
        "证书文件不能超过 4 MiB"
    );
    let bytes = std::fs::read(path)?;
    ensure!(bytes.len() <= 4 * 1024 * 1024, "证书文件不能超过 4 MiB");
    Ok(bytes)
}
fn roots(settings: &NetworkSettings) -> Result<Vec<CertificateDer<'static>>> {
    let Some(path) = &settings.custom_ca_path else {
        return Ok(vec![]);
    };
    let bytes = file(path)?;
    let certificates =
        rustls_pemfile::certs(&mut Cursor::new(&bytes)).collect::<std::io::Result<Vec<_>>>()?;
    let certificates = if certificates.is_empty() {
        vec![CertificateDer::from(bytes)]
    } else {
        certificates
    };
    let mut check = rustls::RootCertStore::empty();
    for cert in &certificates {
        check.add(cert.clone()).context("自定义 CA 文件无效")?;
    }
    Ok(certificates)
}
fn identity(settings: &NetworkSettings, url: &Url) -> Result<Option<Vec<u8>>> {
    for certificate in &settings.client_certificates {
        if !certificate.matches(url)? {
            continue;
        }
        let mut bytes = file(&certificate.certificate_file)?;
        let key = file(&certificate.private_key_file)?;
        ensure!(
            !String::from_utf8_lossy(&key).contains("ENCRYPTED"),
            "仅支持未加密 PEM 私钥"
        );
        bytes.push(b'\n');
        bytes.extend(key);
        return Ok(Some(bytes));
    }
    Ok(None)
}
pub(crate) fn configure_http(
    mut builder: reqwest::ClientBuilder,
    settings: &NetworkSettings,
    url: &Url,
) -> Result<reqwest::ClientBuilder> {
    if url.scheme() != "https" {
        return Ok(builder);
    }
    for certificate in roots(settings)? {
        builder =
            builder.add_root_certificate(reqwest::Certificate::from_der(certificate.as_ref())?);
    }
    if let Some(pem) = identity(settings, url)? {
        builder = builder
            .identity(reqwest::Identity::from_pem(&pem).context("客户端证书或 PEM 私钥无效")?);
        // The execution engine follows redirects one hop at a time and reselects the identity.
        builder = builder.redirect(reqwest::redirect::Policy::none());
    }
    Ok(builder)
}
pub(crate) fn websocket_connector(
    settings: &NetworkSettings,
    url: &Url,
) -> Result<Option<tokio_tungstenite::Connector>> {
    if url.scheme() != "wss" {
        return Ok(None);
    }
    let mut store = rustls::RootCertStore::empty();
    store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for certificate in roots(settings)? {
        store.add(certificate)?;
    }
    let builder = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(store);
    let config = if let Some(pem) = identity(settings, url)? {
        let certificates =
            rustls_pemfile::certs(&mut Cursor::new(&pem)).collect::<std::io::Result<Vec<_>>>()?;
        let key =
            rustls_pemfile::private_key(&mut Cursor::new(&pem))?.context("PEM 中没有未加密私钥")?;
        builder
            .with_client_auth_cert(certificates, key)
            .context("客户端证书和私钥不匹配")?
    } else {
        builder.with_no_client_auth()
    };
    Ok(Some(tokio_tungstenite::Connector::Rustls(Arc::new(config))))
}
fn environment_certificates(
    collection: &Collection,
    name: &str,
    seen: &mut Vec<String>,
) -> Result<Vec<Value>> {
    ensure!(
        seen.len() < 64 && !seen.iter().any(|value| value == name),
        "环境证书继承存在循环或过深"
    );
    seen.push(name.into());
    let env = collection
        .documents()
        .iter()
        .find(|doc| doc.kind() == crate::DocumentKind::Environment && doc.name() == name)
        .map(|doc| doc.value())
        .or_else(|| {
            collection.project().value()["config"]["environments"]
                .as_array()
                .and_then(|envs| envs.iter().find(|env| env["name"] == name))
        })
        .context("证书环境不存在")?;
    let mut certificates = env["clientCertificates"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(parent) = env["extends"].as_str() {
        certificates.extend(environment_certificates(collection, parent, seen)?);
    }
    seen.pop();
    Ok(certificates)
}
pub(crate) fn resolve_settings(
    collection: &Collection,
    environment: Option<&str>,
    variables: &Value,
    settings: &mut NetworkSettings,
) -> Result<()> {
    let root = collection.root();
    let mut sources = if let Some(name) = environment {
        environment_certificates(collection, name, &mut vec![])?
    } else {
        vec![]
    };
    sources.extend(
        collection.project().value()["config"]["clientCertificates"]
            .as_array()
            .into_iter()
            .flatten()
            .cloned(),
    );
    let resolve_path = |value: &str| -> Result<PathBuf> {
        let path = PathBuf::from(interpolate(value, variables)?);
        Ok(if path.is_absolute() {
            path
        } else {
            root.join(path)
        })
    };
    let mut certificates = vec![];
    for value in sources {
        if value["disabled"] == true {
            continue;
        }
        ensure!(
            value["type"] == "pem",
            "仅支持 PEM 客户端证书；请禁用或转换 PKCS#12 证书"
        );
        ensure!(
            value["passphrase"].as_str().is_none_or(str::is_empty),
            "仅支持未加密 PEM 私钥"
        );
        certificates.push(ClientCertificate {
            domain: interpolate(
                value["domain"].as_str().context("客户端证书缺少 domain")?,
                variables,
            )?,
            certificate_file: resolve_path(
                value["certificateFilePath"]
                    .as_str()
                    .context("客户端证书缺少 certificateFilePath")?,
            )?,
            private_key_file: resolve_path(
                value["privateKeyFilePath"]
                    .as_str()
                    .context("客户端证书缺少 privateKeyFilePath")?,
            )?,
            disabled: false,
        });
    }
    certificates.extend(settings.client_certificates.clone());
    settings.client_certificates = certificates;
    if let Some(path) =
        collection.project().value()["extensions"]["apifuck"]["customCaCertificate"].as_str()
    {
        settings.custom_ca_path = if path.is_empty() {
            None
        } else {
            Some(resolve_path(path)?)
        };
    }
    settings.validate()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identities_are_matched_by_host_and_optional_port() {
        let mut cert = ClientCertificate {
            domain: "*.example.com:443".into(),
            ..Default::default()
        };
        assert!(
            cert.matches(&Url::parse("https://api.example.com/").unwrap())
                .unwrap()
        );
        assert!(
            !cert
                .matches(&Url::parse("https://example.com/").unwrap())
                .unwrap()
        );
        assert!(
            !cert
                .matches(&Url::parse("https://badexample.com/").unwrap())
                .unwrap()
        );
        assert!(
            !cert
                .matches(&Url::parse("https://api.example.com:444/").unwrap())
                .unwrap()
        );
        cert.disabled = true;
        assert!(
            !cert
                .matches(&Url::parse("https://api.example.com/").unwrap())
                .unwrap()
        );
    }
}
