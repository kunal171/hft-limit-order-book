/// Lifecycle states in the users table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    Active,
    Suspended,
    Closed,
}

impl UserStatus {
    /// Converts the Rust enum into the value stored in PostgreSQL.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Closed => "closed",
        }
    }

    /// Converts a stored value back into the enum.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "suspended" => Some(Self::Suspended),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_round_trips_every_status() {
        for status in [
            UserStatus::Active,
            UserStatus::Suspended,
            UserStatus::Closed,
        ] {
            assert_eq!(UserStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn parse_rejects_unknown_status() {
        assert_eq!(UserStatus::parse("deleted"), None);
    }
}
