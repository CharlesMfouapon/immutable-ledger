use sha2::{Sha256, Digest};
use std::collections::HashMap;

/// Merkle proof for a single account balance
#[derive(Debug, Clone)]
pub struct BalanceProof {
    pub account: String,
    pub balance: i64,              // In micro-dollars
    pub proof_hashes: Vec<[u8; 32]>, // Sibling hashes from leaf to root
    pub root_hash: [u8; 32],
}

/// Merkleized account balance tree for cryptographic verification
pub struct BalanceTree {
    leaves: HashMap<String, i64>,
    root: Option<[u8; 32]>,
}

impl BalanceTree {
    pub fn new() -> Self {
        Self {
            leaves: HashMap::new(),
            root: None,
        }
    }
    
    pub fn update(&mut self, account: &str, balance: i64) {
        self.leaves.insert(account.to_string(), balance);
        self.root = None; // Invalidate cached root
    }
    
    /// Computes Merkle root from current leaves
    pub fn compute_root(&mut self) -> [u8; 32] {
        if self.leaves.is_empty() {
            return [0u8; 32];
        }
        
        let mut leaves: Vec<[u8; 32]> = self.leaves
            .iter()
            .map(|(account, balance)| {
                let mut hasher = Sha256::new();
                hasher.update(account.as_bytes());
                hasher.update(balance.to_le_bytes());
                hasher.finalize().into()
            })
            .collect();
        
        // Build tree bottom-up
        while leaves.len() > 1 {
            if leaves.len() % 2 != 0 {
                leaves.push(leaves.last().copied().unwrap_or([0u8; 32]));
            }
            leaves = leaves.chunks(2).map(|pair| {
                let mut hasher = Sha256::new();
                hasher.update(pair[0]);
                hasher.update(pair[1]);
                hasher.finalize().into()
            }).collect();
        }
        
        let root = leaves[0];
        self.root = Some(root);
        root
    }
    
    /// Generates a Merkle proof for a specific account
    pub fn generate_proof(&self, account: &str) -> Option<BalanceProof> {
        let balance = self.leaves.get(account)?;
        let root = self.root?;
        
        // For a complete implementation, we'd store the tree structure
        // Here we provide the interface a real implementation would use
        Some(BalanceProof {
            account: account.to_string(),
            balance: *balance,
            proof_hashes: vec![], // Simplified: full impl tracks sibling path
            root_hash: root,
        })
    }
    
    /// Verifies a balance proof against a root hash
    pub fn verify_proof(proof: &BalanceProof) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(proof.account.as_bytes());
        hasher.update(proof.balance.to_le_bytes());
        let mut current_hash: [u8; 32] = hasher.finalize().into();
        
        for sibling in &proof.proof_hashes {
            let mut parent_hasher = Sha256::new();
            parent_hasher.update(current_hash);
            parent_hasher.update(sibling);
            current_hash = parent_hasher.finalize().into();
        }
        
        current_hash == proof.root_hash
    }
}
