use bip39::{Language, Mnemonic};
use zeroize::Zeroizing;

pub fn generate_24_words() -> Result<Zeroizing<String>, String> {
    let m = Mnemonic::generate_in(Language::English, 24)
        .map_err(|_| "Secure entropy generation failed".to_string())?;
    Ok(Zeroizing::new(m.to_string()))
}

pub fn validate_phrase(input: &str) -> Result<Zeroizing<String>, String> {
    let m = Mnemonic::parse_in(Language::English, input.trim())
        .map_err(|_| "Invalid recovery phrase or word not in BIP39 dictionary".to_string())?;
    if m.word_count() != 24 {
        return Err("Only 24-word recovery phrases are supported".into());
    }
    Ok(Zeroizing::new(m.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bip39_generation_and_validation() {
        let p = generate_24_words().unwrap();
        let words: Vec<&str> = p.split_whitespace().collect();
        assert_eq!(words.len(), 24);

        let validated = validate_phrase(&p).unwrap();
        assert_eq!(&*validated, &*p);

        assert!(validate_phrase("invalid word sequence that fails").is_err());
        assert!(validate_phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").is_err(), "Must reject 12-word phrases");
    }
}
