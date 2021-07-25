use std::convert::TryInto;

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct SubscriberName(String);

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryInto<SubscriberName> for String {
    type Error = String;

    fn try_into(self) -> Result<SubscriberName, Self::Error> {
        let s = self.trim();
        // `.trim()` returns a view over the input `s` without trailing
        // whitespace-like characters.
        // `.is_empty` checks if the view contains any character.
        let is_empty_or_whitespace = s.is_empty();
        // A grapheme is defined by the Unicode standard as a "user-perceived"
        // character: `å` is a single grapheme, but it is composed of two characters
        // (`a` and ``).
        //
        // `graphemes` returns an iterator over the graphemes in the input `s`.
        // `true` specifies that we want to use the extended grapheme definition set,
        // the recommended one.
        let is_too_long = s.graphemes(true).count() > 256;
        // Iterate over all characters in the input `s` to check if any of them matches
        // one of the characters in the forbidden array.
        let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
        let contains_forbidden_characters = s.chars().any(|g| forbidden_characters.contains(&g));
        if is_empty_or_whitespace || is_too_long || contains_forbidden_characters {
            Err(format!("{} is not a valid subscriber name.", s))
        } else {
            Ok(SubscriberName(s.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claim::{assert_err, assert_ok};

    #[test]
    fn a_256_grapheme_long_name_is_valid() {
        let name = "a".repeat(256);
        assert_ok!(TryInto::<SubscriberName>::try_into(name));
    }

    #[test]
    fn a_257_grapheme_long_name_is_valid() {
        let name = "a".repeat(257);
        assert_err!(TryInto::<SubscriberName>::try_into(name));
    }

    #[test]
    fn a_whitespace_only_name_is_rejected() {
        let name = " ".to_string();
        assert_err!(TryInto::<SubscriberName>::try_into(name));
    }

    #[test]
    fn an_empty_name_is_rejected() {
        let name = "".to_string();
        assert_err!(TryInto::<SubscriberName>::try_into(name));
    }

    #[test]
    fn invalid_name_characters_are_rejected() {
        let names = &['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
        for name in names {
            let name = name.to_string();
            assert_err!(TryInto::<SubscriberName>::try_into(name));
        }
    }

    #[test]
    fn a_valid_name_is_accepted() {
        let name = "Joseph Cheverton-Wynne".to_string();
        assert_ok!(TryInto::<SubscriberName>::try_into(name));
    }
}
