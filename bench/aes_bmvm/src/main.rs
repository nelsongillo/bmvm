#![no_std]
#![no_main]

extern crate core;

use bmvm_guest::{ForeignBuf, SharedBuf, expose};

use aes_gcm::aead::Buffer;
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::AeadInOut, aead::KeyInit};

struct FixedBuf<'a> {
    buf: &'a mut [u8],
    idx: usize,
}

impl<'a> FixedBuf<'a> {
    fn from(buf: &'a mut [u8]) -> Self {
        Self { buf, idx: 0 }
    }

    fn try_extend_from_slice(&mut self, other: &[u8]) -> core::result::Result<(), usize> {
        if self.idx + other.len() > self.buf.len() {
            return Err(self.idx + other.len() - self.buf.len());
        }

        self.buf[self.idx..(self.idx + other.len())].copy_from_slice(&other);
        Ok(())
    }

    fn truncate(&mut self, _len: usize) {}
}

impl<'a> AsRef<[u8]> for FixedBuf<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.buf
    }
}

impl<'a> AsMut<[u8]> for FixedBuf<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}

impl<'a> Buffer for FixedBuf<'a> {
    fn extend_from_slice(&mut self, other: &[u8]) -> aes_gcm::aead::Result<()> {
        self.try_extend_from_slice(other)
            .map_err(|_| aes_gcm::aead::Error)
    }

    fn truncate(&mut self, len: usize) {
        self.truncate(len);
    }
}

#[expose]
fn encrypt(key: ForeignBuf, msg: ForeignBuf, nonce: ForeignBuf) -> SharedBuf {
    let mut owned = msg.owned();

    let key = Key::<Aes256Gcm>::try_from(key.as_ref()).unwrap();
    let nonce = Nonce::try_from(nonce.as_ref()).unwrap();
    let aes = Aes256Gcm::new(&key);

    let mut buf = FixedBuf::from(owned.as_mut());
    aes.encrypt_in_place(&nonce, b"", &mut buf).unwrap();
    owned.into_shared()
}

#[expose]
fn decrypt(key: ForeignBuf, msg: ForeignBuf, nonce: ForeignBuf) -> SharedBuf {
    let mut owned = msg.owned();

    let key = Key::<Aes256Gcm>::try_from(key.as_ref()).unwrap();
    let nonce = Nonce::try_from(nonce.as_ref()).unwrap();
    let aes = Aes256Gcm::new(&key);

    let mut buf = FixedBuf::from(owned.as_mut());
    aes.decrypt_in_place(&nonce, b"", &mut buf).unwrap();
    owned.into_shared()
}
