/// Wallet helpers: address + signing for AWP chain operations.
///
/// For community-skill the agent authenticates via api_key (from
/// `register`), not wallet signatures. Wallet signing is only needed
/// for the chain-level registration (awp_register.rs). We keep this
/// minimal — read AWP_ADDRESS / AWP_PRIVATE_KEY from env and sign
/// EIP-712 when required.

use anyhow::{bail, Context, Result};
use k256::ecdsa::{signature::Signer, Signature, SigningKey};
use sha3::{Digest, Keccak256};

/// Resolve the agent's EVM address from env: COMMUNITY_AWP_ADDRESS
/// (preferred) or AWP_ADDRESS (compat with other skills).
pub fn resolve_address(explicit: Option<&str>) -> Result<String> {
    if let Some(a) = explicit {
        return Ok(normalize_hex_address(a)?);
    }
    if let Ok(a) = std::env::var("COMMUNITY_AWP_ADDRESS") {
        return Ok(normalize_hex_address(&a)?);
    }
    if let Ok(a) = std::env::var("AWP_ADDRESS") {
        return Ok(normalize_hex_address(&a)?);
    }
    bail!("no AWP address — set COMMUNITY_AWP_ADDRESS or pass --address")
}

pub fn resolve_private_key() -> Result<SigningKey> {
    let hex = std::env::var("COMMUNITY_AWP_PRIVATE_KEY")
        .or_else(|_| std::env::var("AWP_PRIVATE_KEY"))
        .context("no private key — set COMMUNITY_AWP_PRIVATE_KEY or AWP_PRIVATE_KEY")?;
    let bytes = hex::decode(hex.trim_start_matches("0x")).context("decode private key hex")?;
    SigningKey::from_slice(&bytes).context("invalid private key")
}

fn normalize_hex_address(a: &str) -> Result<String> {
    let s = a.trim().to_lowercase();
    let s = s.strip_prefix("0x").unwrap_or(&s);
    if s.len() != 40 {
        bail!("address must be 40 hex chars (got {})", s.len())
    }
    if !s.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("address contains non-hex chars")
    }
    Ok(format!("0x{s}"))
}

/// Keccak256 digest as 0x-prefixed lowercase hex.
pub fn keccak256_hex(data: &[u8]) -> String {
    let mut h = Keccak256::new();
    h.update(data);
    let out = h.finalize();
    format!("0x{}", hex::encode(out))
}

/// Sign a 32-byte digest, return 65-byte 0x-prefixed r||s||v.
pub fn sign_digest(key: &SigningKey, digest: &[u8; 32]) -> Result<String> {
    let (signature, recovery_id): (Signature, _) = key
        .sign_prehash_recoverable(digest)
        .context("sign prehash")?;
    let mut bytes = signature.to_bytes().to_vec();
    bytes.push(27 + recovery_id.to_byte()); // v
    Ok(format!("0x{}", hex::encode(bytes)))
}
