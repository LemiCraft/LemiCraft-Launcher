use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit};
use sha2::{Digest, Sha256};

// Encrypts account/session files where DPAPI doesn't exist. The key is derived from machine + user, not stored:
// no keyring daemon needed (absent in WSL/minimal desktops) and no key file to leak. A copied or backed-up
// file is useless elsewhere; like DPAPI it doesn't stop other code running as the same user.
const MAGIC: &[u8; 4] = b"LCE1";
const NONCE_LEN: usize = 12;

fn machine_key() -> [u8; 32] {
    let machine_id = ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();

    // The home directory always exists and belongs to the user, unlike the launcher's own data
    // dir on the very first save — a key that changed between seal and open would log the user out
    #[cfg(unix)]
    let uid = {
        use std::os::unix::fs::MetadataExt;
        dirs::home_dir().and_then(|home| std::fs::metadata(home).ok()).map(|m| m.uid()).unwrap_or(0)
    };
    #[cfg(not(unix))]
    let uid = 0u32;

    let mut hasher = Sha256::new();
    hasher.update(b"LemiCraftLauncher-account-key-v1");
    hasher.update(machine_id.trim().as_bytes());
    hasher.update(uid.to_le_bytes());
    hasher.finalize().into()
}

pub fn seal(plain: &[u8]) -> Option<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(&machine_key().into());
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).ok()?;
    let encrypted = cipher.encrypt(&nonce.into(), plain).ok()?;

    let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + encrypted.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&encrypted);
    Some(out)
}

/// Files written before this existed are plain JSON without the magic prefix — still readable,
/// and get sealed on the next save.
pub fn open(data: &[u8]) -> Option<Vec<u8>> {
    let Some(rest) = data.strip_prefix(MAGIC.as_slice()) else {
        return Some(data.to_vec());
    };
    if rest.len() <= NONCE_LEN {
        return None;
    }
    let (nonce, encrypted) = rest.split_at(NONCE_LEN);
    let cipher = ChaCha20Poly1305::new(&machine_key().into());
    let nonce: [u8; NONCE_LEN] = nonce.try_into().ok()?;
    cipher.decrypt(&nonce.into(), encrypted).ok()
}
