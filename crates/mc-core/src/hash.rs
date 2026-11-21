//! Empreintes SHA-256 en hexadécimal minuscule.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

/// SHA-256 des morceaux mis bout à bout, en 64 caractères hexadécimaux
/// minuscules.
///
/// L'hexadécimal est écrit à la main depuis sha2 0.11 : sa sortie est un
/// `hybrid_array::Array`, qui n'implémente pas `LowerHex`, là où le
/// `generic_array` de sha2 0.10 se laissait formater par `{:x}`. Le résultat
/// ne doit pas changer d'un octet : `plan_hash` est enregistré avec le plan au
/// dry-run puis recomparé avant d'appliquer, et `scope_hash` retrouve en base
/// la simulation préalable d'un nettoyage CI. Un autre format refuserait les
/// plans et les simulations faits avant la montée.
pub(crate) fn sha256_hex(parts: &[&[u8]]) -> String {
    let mut h = Sha256::new();
    for part in parts {
        h.update(part);
    }
    h.finalize()
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    /// Vecteurs de la FIPS 180-2 : ils figent le format (minuscules, deux
    /// chiffres par octet, zéros de tête conservés).
    #[test]
    fn suit_les_vecteurs_de_reference() {
        assert_eq!(
            sha256_hex(&[b"abc"]),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(&[]),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// Plusieurs morceaux valent leur concaténation, comme les `update`
    /// successifs qu'écrivait `scope_hash`.
    #[test]
    fn concatene_les_morceaux() {
        assert_eq!(sha256_hex(&[b"a", b"bc"]), sha256_hex(&[b"abc"]));
    }
}
