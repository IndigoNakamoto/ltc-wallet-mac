//! aezeed cipher-seed decoding (lnd's seed scheme, used by Nexus).
//!
//! Port of the decipher half of lnd's `aezeed/cipherseed.go`. The 24 words
//! encode 33 bytes: `extVersion(1) || ciphertext(23) || salt(5) || crc32c(4)`.
//! The ciphertext deciphers (AEZ, tau=4, AD = version||salt, key from
//! scrypt(n=32768, r=8, p=1)) to 19 bytes:
//! `internalVersion(1) || birthday_be_u16(2) || entropy(16)`.
//!
//! The 16-byte entropy is used *directly* as the BIP32 master seed — this is
//! not BIP39, despite sharing the English wordlist.

use bdk_wallet::keys::bip39::Language;

use crate::error::WalletError;

/// External (enciphering) scheme version we understand (lnd version 0).
const CIPHER_SEED_VERSION: u8 = 0;
/// Bytes encoded by the 24 words (24 * 11 bits = 264 bits).
const ENCIPHERED_SIZE: usize = 33;
/// Deciphered plaintext: version(1) || birthday(2) || entropy(16).
const DECIPHERED_SIZE: usize = 19;
/// AEZ ciphertext expansion (acts as a 32-bit MAC).
const CIPHERTEXT_EXPANSION: u32 = 4;
/// Scrypt salt location within the 33 bytes.
const SALT_OFFSET: usize = ENCIPHERED_SIZE - CHECKSUM_SIZE - SALT_SIZE;
const SALT_SIZE: usize = 5;
const CHECKSUM_OFFSET: usize = ENCIPHERED_SIZE - CHECKSUM_SIZE;
const CHECKSUM_SIZE: usize = 4;
/// Passphrase used when the user did not set one.
pub const DEFAULT_PASSPHRASE: &str = "aezeed";

/// A successfully deciphered aezeed seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAezeed {
    /// 16 bytes used directly as the BIP32 master seed.
    pub entropy: [u8; 16],
    /// Days since 2013-01-01 (Bitcoin genesis date in lnd's scheme).
    pub birthday_days: u16,
    /// Plaintext version marking the wallet's derivation scheme (lnd uses 0,
    /// Nexus stamps 1). Informational: lnd itself never validates it, and the
    /// plaintext layout is identical either way.
    pub internal_version: u8,
}

/// Decipher a 24-word aezeed mnemonic with the given passphrase
/// (empty/None means the scheme default, the literal string `aezeed`).
pub fn decode(words: &[&str], passphrase: Option<&str>) -> Result<DecodedAezeed, WalletError> {
    if words.len() != 24 {
        return Err(WalletError::InvalidMnemonic(format!(
            "aezeed needs exactly 24 words, got {}",
            words.len()
        )));
    }

    let bytes = words_to_bytes(words)?;

    let ext_version = bytes[0];
    if ext_version != CIPHER_SEED_VERSION {
        return Err(WalletError::InvalidMnemonic(format!(
            "unsupported aezeed version {ext_version} (only version 0 is supported)"
        )));
    }

    // CRC-32 Castagnoli over everything before the checksum, big-endian.
    let expected = u32::from_be_bytes(bytes[CHECKSUM_OFFSET..].try_into().expect("4 bytes"));
    let actual = crc32c::crc32c(&bytes[..CHECKSUM_OFFSET]);
    if expected != actual {
        return Err(WalletError::InvalidMnemonic(
            "aezeed checksum mismatch — one or more words are wrong or out of order".into(),
        ));
    }

    let salt = &bytes[SALT_OFFSET..CHECKSUM_OFFSET];
    let pass = match passphrase {
        Some(p) if !p.is_empty() => p,
        _ => DEFAULT_PASSPHRASE,
    };

    // scrypt(n=32768, r=8, p=1) -> 32-byte AEZ key.
    let params = scrypt::Params::new(15, 8, 1)
        .map_err(|e| WalletError::InvalidMnemonic(format!("scrypt params: {e}")))?;
    let mut key = [0u8; 32];
    scrypt::scrypt(pass.as_bytes(), salt, &params, &mut key)
        .map_err(|e| WalletError::InvalidMnemonic(format!("scrypt: {e}")))?;

    let mut ad = [0u8; 1 + SALT_SIZE];
    ad[0] = ext_version;
    ad[1..].copy_from_slice(salt);

    let ciphertext = &bytes[1..SALT_OFFSET];
    let plaintext = zears::Aez::new(&key)
        .decrypt(&[], &[&ad], CIPHERTEXT_EXPANSION, ciphertext)
        .ok_or_else(|| {
            WalletError::InvalidMnemonic(
                "could not decipher aezeed seed — wrong cipher-seed passphrase?".into(),
            )
        })?;
    if plaintext.len() != DECIPHERED_SIZE {
        return Err(WalletError::InvalidMnemonic(format!(
            "aezeed plaintext has unexpected size {}",
            plaintext.len()
        )));
    }
    // The internal version marks the derivation scheme, not the format: lnd
    // decodes it without validation, and the authenticated decryption above
    // already guarantees the 19-byte layout. Nexus seeds carry version 1.
    let birthday_days = u16::from_be_bytes([plaintext[1], plaintext[2]]);
    let mut entropy = [0u8; 16];
    entropy.copy_from_slice(&plaintext[3..]);
    Ok(DecodedAezeed {
        entropy,
        birthday_days,
        internal_version: plaintext[0],
    })
}

/// Pack 24 words (11 bits each, MSB first) into 33 bytes.
fn words_to_bytes(words: &[&str]) -> Result<[u8; ENCIPHERED_SIZE], WalletError> {
    let mut bytes = [0u8; ENCIPHERED_SIZE];
    let mut bit_pos = 0usize;
    for word in words {
        let index = Language::English.find_word(word).ok_or_else(|| {
            WalletError::InvalidMnemonic(format!("\"{word}\" is not in the seed wordlist"))
        })?;
        for shift in (0..11).rev() {
            if (index >> shift) & 1 == 1 {
                bytes[bit_pos / 8] |= 1 << (7 - bit_pos % 8);
            }
            bit_pos += 1;
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known-answer vectors generated with lnd's aezeed package at PRODUCTION
    // scrypt parameters (n=32768, r=8, p=1). The vectors printed in lnd's
    // cipherseed_test.go cannot be used here: that file's init() lowers
    // scryptN to 16 for test speed, so its mnemonics never occur in the wild.
    // Inputs: entropy below, salt "salt1", internal version 0.
    const TEST_ENTROPY: [u8; 16] = [
        0x81, 0xb6, 0x37, 0xd8, 0x63, 0x59, 0xe6, 0x96, 0x0d, 0xe7, 0x95, 0xe4, 0x1e, 0x0b, 0x4c,
        0xfd,
    ];

    const VECTOR_NO_PASSPHRASE: &str = "above judge emerge veteran reform crunch system all snap \
         please shoulder vault hurt city quarter cover enlist swear success suggest drink wagon \
         enrich body";

    const VECTOR_WITH_PASSPHRASE: &str = "absorb century submit father path glove gloom super \
         divert garden ice mirror wisdom grass dice kit ugly castle success suggest drink \
         monster congress flight";

    #[test]
    fn lnd_vector_default_passphrase() {
        let words: Vec<&str> = VECTOR_NO_PASSPHRASE.split_whitespace().collect();
        let decoded = decode(&words, None).unwrap();
        assert_eq!(decoded.entropy, TEST_ENTROPY);
        assert_eq!(decoded.birthday_days, 0);
        assert_eq!(decoded.internal_version, 0);
    }

    #[test]
    fn lnd_vector_custom_passphrase() {
        let words: Vec<&str> = VECTOR_WITH_PASSPHRASE.split_whitespace().collect();
        let decoded = decode(&words, Some("!very_safe_55345_password*")).unwrap();
        assert_eq!(decoded.entropy, TEST_ENTROPY);
        assert_eq!(decoded.birthday_days, 3365);
    }

    #[test]
    fn wrong_passphrase_fails_closed() {
        let words: Vec<&str> = VECTOR_WITH_PASSPHRASE.split_whitespace().collect();
        let err = decode(&words, Some("wrong")).unwrap_err();
        assert!(err.to_string().contains("passphrase"), "{err}");
    }

    #[test]
    fn swapped_words_fail_checksum() {
        let mut words: Vec<&str> = VECTOR_NO_PASSPHRASE.split_whitespace().collect();
        // Swapping mid-phrase words corrupts the payload but not the version
        // byte, so the failure is attributed to the checksum, as in lnd.
        words.swap(5, 6);
        let err = decode(&words, None).unwrap_err();
        assert!(err.to_string().contains("checksum"), "{err}");
    }

    #[test]
    fn unknown_word_is_reported() {
        let mut words: Vec<&str> = VECTOR_NO_PASSPHRASE.split_whitespace().collect();
        words[3] = "litecoin";
        let err = decode(&words, None).unwrap_err();
        assert!(err.to_string().contains("litecoin"), "{err}");
    }
}
