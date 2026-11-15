//! Per-client TLS configuration without modifying the host's crypto provider.
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime, pem::PemObject};
use rustls::{ClientConfig, DigitallySignedStruct, Error, SignatureScheme};

pub(crate) fn client_config(
    ca_bundle: Option<&[u8]>,
    insecure: bool,
) -> Result<ClientConfig, Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = Vec::new();
    if let Some(bytes) = ca_bundle {
        if bytes.windows(10).any(|window| window == b"-----BEGIN") {
            roots = CertificateDer::pem_slice_iter(bytes)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| Error::General(format!("invalid CA bundle: {error}")))?;
            if roots.is_empty() {
                return Err(Error::General("CA bundle contains no certificates".into()));
            }
        } else {
            roots.push(CertificateDer::from(bytes.to_vec()));
        }
        // Validate DER even on platforms whose native verifier defers parsing.
        let mut store = rustls::RootCertStore::empty();
        for root in &roots {
            store.add(root.clone())?;
        }
    }
    let verifier: Arc<dyn ServerCertVerifier> = if insecure {
        Arc::new(InsecureVerifier {
            provider: Arc::clone(&provider),
        })
    } else {
        Arc::new(rustls_platform_verifier::Verifier::new_with_extra_roots(
            roots,
            Arc::clone(&provider),
        )?)
    };
    Ok(ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth())
}

#[derive(Debug)]
struct InsecureVerifier {
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
