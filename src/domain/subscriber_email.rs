use std::{convert::TryInto, fmt::Display};

use validator::validate_email;

#[derive(Debug)]
pub struct SubscriberEmail(String);

impl Display for SubscriberEmail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryInto<SubscriberEmail> for String {
    type Error = String;

    fn try_into(self) -> Result<SubscriberEmail, Self::Error> {
        if validate_email(&self) {
            Ok(SubscriberEmail(self))
        } else {
            Err(format!("{} is not a valid email", self))
        }
    }
}

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::SubscriberEmail;
    use claim::assert_err;
    use fake::{faker::internet::en::SafeEmail, Fake};

    #[derive(Debug, Clone)]
    struct ValidEmailFixture(pub String);

    impl quickcheck::Arbitrary for ValidEmailFixture {
        fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
            let email = SafeEmail().fake_with_rng(g);
            Self(email)
        }
    }

    #[test]
    fn empty_string_is_rejected() {
        let email = "".to_string();
        assert_err!(TryInto::<SubscriberEmail>::try_into(email));
    }

    #[test]
    fn email_missing_at_symbol_is_rejected() {
        let email = "ursuladomain.com".to_string();
        assert_err!(TryInto::<SubscriberEmail>::try_into(email));
    }

    #[test]
    fn email_missing_subject_is_rejected() {
        let email = "@domain.com".to_string();
        assert_err!(TryInto::<SubscriberEmail>::try_into(email));
    }

    #[quickcheck_macros::quickcheck]
    fn valid_emails_are_parsed_successfully(valid_email: ValidEmailFixture) -> bool {
        TryInto::<SubscriberEmail>::try_into(valid_email.0).is_ok()
    }
}
