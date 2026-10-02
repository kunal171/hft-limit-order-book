use serde::{Deserialize, Serialize};

/// Allowed roles in the users table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Trader,
    System,
}

impl UserRole {
    /// Converts the Rust enum into the value stored in PostgreSQL.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Trader => "trader",
            Self::System => "system",
        }
    }

    /// Converts a stored value back into the enum.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "admin" => Some(Self::Admin),
            "trader" => Some(Self::Trader),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_round_trips_every_role() {
        for role in [UserRole::Admin, UserRole::Trader, UserRole::System] {
            assert_eq!(UserRole::parse(role.as_str()), Some(role));
        }
    }

    #[test]
    fn parse_rejects_unknown_role() {
        assert_eq!(UserRole::parse("superuser"), None);
    }
}
