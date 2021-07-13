//! ECDSA/secp256k1 support.

pub use k256::ecdsa::{
    recoverable::{Id as RecoveryId, Signature as RecoverableSignature},
    Signature, VerifyingKey,
};

use crate::{
    key::{ring::LoadPkcs8, store::GeneratePkcs8},
    Error, KeyHandle, Map, Result,
};
use alloc::boxed::Box;
use core::fmt;
use ecdsa::signature::Signer;
use pkcs8::{FromPrivateKey, ToPrivateKey};

/// ECDSA/secp256k1 key ring.
#[derive(Debug, Default)]
pub struct KeyRing {
    keys: Map<VerifyingKey, SigningKey>,
}

impl KeyRing {
    /// Create new ECDSA/secp256k1 keystore.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the [`SigningKey`] that corresponds to the provided [`VerifyingKey`]
    /// (i.e. public key)
    pub fn get(&self, verifying_key: &VerifyingKey) -> Option<&SigningKey> {
        self.keys.get(verifying_key)
    }

    /// Iterate over the keys in the keyring.
    pub fn iter(&self) -> impl Iterator<Item = &SigningKey> {
        self.keys.values()
    }
}

impl LoadPkcs8 for KeyRing {
    fn load_pkcs8(&mut self, private_key: pkcs8::PrivateKeyInfo<'_>) -> Result<KeyHandle> {
        let signing_key = SigningKey::from_pkcs8_private_key_info(private_key)?;
        let verifying_key = signing_key.verifying_key();

        if self.keys.contains_key(&verifying_key) {
            return Err(Error::DuplicateKey);
        }

        self.keys.insert(verifying_key, signing_key);
        Ok(KeyHandle::EcdsaSecp256k1(verifying_key))
    }
}

/// Transaction signing key (ECDSA/secp256k1)
pub struct SigningKey {
    inner: Box<dyn Secp256k1Signer>,
}

impl SigningKey {
    /// Initialize from a provided signer object.
    ///
    /// Use [`SigningKey::from_bytes`] to initialize from a raw private key.
    pub fn new(signer: Box<dyn Secp256k1Signer>) -> Self {
        Self { inner: signer }
    }

    /// Initialize from a raw scalar value (big endian).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let signing_key = k256::ecdsa::SigningKey::from_bytes(bytes)?;
        Ok(Self::new(Box::new(signing_key)))
    }

    /// Get the verifying key that corresponds to this signing key.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.inner.verifying_key()
    }
}

impl FromPrivateKey for SigningKey {
    fn from_pkcs8_private_key_info(private_key: pkcs8::PrivateKeyInfo<'_>) -> pkcs8::Result<Self> {
        let signing_key = k256::ecdsa::SigningKey::from_pkcs8_private_key_info(private_key)?;
        Ok(Self::new(Box::new(signing_key)))
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl GeneratePkcs8 for SigningKey {
    /// Randomly generate a new PKCS#8 private key.
    fn generate_pkcs8() -> pkcs8::PrivateKeyDocument {
        k256::SecretKey::random(&mut rand_core::OsRng)
            .to_pkcs8_der()
            .expect("DER error")
    }
}

impl Signer<Signature> for SigningKey {
    fn try_sign(&self, msg: &[u8]) -> signature::Result<Signature> {
        self.inner.try_sign(msg)
    }
}

impl Signer<RecoverableSignature> for SigningKey {
    fn try_sign(&self, msg: &[u8]) -> signature::Result<RecoverableSignature> {
        let sig: Signature = self.inner.try_sign(msg)?;

        // TODO(tarcieri): optimized support for `k256`
        RecoverableSignature::from_trial_recovery(&self.verifying_key(), msg, &sig)
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKey")
            .field("verifying_key", &self.verifying_key())
            .finish()
    }
}

/// ECDSA/secp256k1 signer
pub trait Secp256k1Signer: Signer<Signature> {
    /// Get the ECDSA verifying key for this signer
    fn verifying_key(&self) -> VerifyingKey;
}

impl<T> Secp256k1Signer for T
where
    T: Signer<Signature>,
    VerifyingKey: for<'a> From<&'a T>,
{
    fn verifying_key(&self) -> VerifyingKey {
        self.into()
    }
}
