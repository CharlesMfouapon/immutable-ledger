use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::fmt;

/// Micro-dollar precision: $1.00 = 10_000_000 μ$
/// Allows for 8 decimal places of precision without floating point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MicroDollar(pub i64);

impl MicroDollar {
    pub const ZERO: Self = Self(0);
    
    pub fn from_dollars_cents(dollars: i64, micros: i64) -> Self {
        Self(dollars * 10_000_000 + micros)
    }
    
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
    
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

/// Universal account identifier with checksum (IBAN-inspired)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(String);

impl AccountId {
    pub fn new(id: &str) -> Result<Self, anyhow::Error> {
        if id.len() < 5 || id.len() > 34 {
            anyhow::bail!("AccountId must be 5-34 characters, got {}", id.len());
        }
        if !id.chars().all(|c| c.is_alphanumeric() || c == '-') {
            anyhow::bail!("AccountId contains invalid characters");
        }
        Ok(Self(id.to_uppercase()))
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single journal entry following double-entry principles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub entry_id: u64,
    pub debit_account: AccountId,
    pub credit_account: AccountId,
    pub amount: MicroDollar,
    pub timestamp: i64,          // Unix nanos
    pub metadata: Vec<u8>,       // Opaque reference data
    pub parent_hash: [u8; 32],   // Chain to previous entry
}

impl Entry {
    /// Computes the cryptographic hash of this entry
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.entry_id.to_le_bytes());
        hasher.update(self.debit_account.0.as_bytes());
        hasher.update(self.credit_account.0.as_bytes());
        hasher.update(self.amount.0.to_le_bytes());
        hasher.update(self.timestamp.to_le_bytes());
        hasher.update(&self.metadata);
        hasher.update(self.parent_hash);
        hasher.finalize().into()
    }
}
