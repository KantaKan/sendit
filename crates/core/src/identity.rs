//! Our TLS identity. LocalSend defines a device's fingerprint as SHA-256 of its cert.
use crate::error::Result;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub struct Identity {
    pub cert_der: Vec<u8>,
    /// PKCS#8
    pub key_der: Vec<u8>,
    /// Uppercase hex SHA-256 of `cert_der`.
    pub fingerprint: String,
}

pub fn fingerprint_of(cert_der: &[u8]) -> String {
    hex::encode_upper(Sha256::digest(cert_der))
}

impl Identity {
    pub fn generate() -> Result<Self> {
        let ck = rcgen::generate_simple_self_signed(vec!["sendit".to_string()])?;
        let cert_der = ck.cert.der().to_vec();
        Ok(Self {
            fingerprint: fingerprint_of(&cert_der),
            key_der: ck.key_pair.serialize_der(),
            cert_der,
        })
    }

    pub fn load_or_create(dir: &Path) -> Result<Self> {
        let cert_path = dir.join("cert.der");
        let key_path = dir.join("key.der");
        if cert_path.exists() && key_path.exists() {
            let cert_der = fs::read(cert_path)?;
            return Ok(Self {
                fingerprint: fingerprint_of(&cert_der),
                key_der: fs::read(key_path)?,
                cert_der,
            });
        }
        fs::create_dir_all(dir)?;
        let id = Self::generate()?;
        fs::write(&cert_path, &id.cert_der)?;
        fs::write(&key_path, &id.key_der)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_uppercase_sha256_hex() {
        let id = Identity::generate().unwrap();
        assert_eq!(id.fingerprint.len(), 64);
        assert!(id
            .fingerprint
            .chars()
            .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)));
        assert_eq!(id.fingerprint, fingerprint_of(&id.cert_der));
    }

    #[test]
    fn each_generate_is_unique() {
        assert_ne!(
            Identity::generate().unwrap().fingerprint,
            Identity::generate().unwrap().fingerprint
        );
    }

    #[test]
    fn load_or_create_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let a = Identity::load_or_create(dir.path()).unwrap();
        let b = Identity::load_or_create(dir.path()).unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.key_der, b.key_der);
    }
}
