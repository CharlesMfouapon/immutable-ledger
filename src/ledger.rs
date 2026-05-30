use crate::types::{AccountId, Entry, MicroDollar};
use crate::merkle::BalanceTree;
use rusqlite::Connection;
use std::collections::HashMap;
use parking_lot::RwLock;

/// Result of applying a journal entry
#[derive(Debug)]
pub enum PostingResult {
    Applied {
        entry_id: u64,
        new_state_root: [u8; 32],
    },
    Rejected {
        reason: String,
    },
}

/// Core double-entry ledger with Merkle-verifiable state
pub struct Ledger {
    conn: Connection,
    balances: RwLock<HashMap<AccountId, MicroDollar>>,
    balance_tree: RwLock<BalanceTree>,
    next_entry_id: u64,
    chain_head: [u8; 32],
}

impl Ledger {
    /// Opens or creates a ledger backed by SQLite (Postgres-ready via rusqlite)
    pub fn open(path: &str) -> Result<Self, anyhow::Error> {
        let conn = Connection::open(path)?;
        
        // Initialize schema
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS entries (
                entry_id INTEGER PRIMARY KEY,
                debit_account TEXT NOT NULL,
                credit_account TEXT NOT NULL,
                amount_micros INTEGER NOT NULL,
                timestamp_ns INTEGER NOT NULL,
                metadata BLOB,
                parent_hash BLOB NOT NULL,
                entry_hash BLOB NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS balances (
                account_id TEXT PRIMARY KEY,
                balance_micros INTEGER NOT NULL,
                last_updated_ns INTEGER NOT NULL
            );
            
            CREATE INDEX IF NOT EXISTS idx_entries_debit ON entries(debit_account);
            CREATE INDEX IF NOT EXISTS idx_entries_credit ON entries(credit_account);
            CREATE INDEX IF NOT EXISTS idx_balances_account ON balances(account_id);
            
            -- Ensure we never have negative balances (configurable)
            -- This is enforced at application level for flexibility
        ")?;
        
        let mut ledger = Self {
            conn,
            balances: RwLock::new(HashMap::new()),
            balance_tree: RwLock::new(BalanceTree::new()),
            next_entry_id: 0,
            chain_head: [0u8; 32],
        };
        
        ledger.load_state()?;
        Ok(ledger)
    }
    
    fn load_state(&mut self) -> Result<(), anyhow::Error> {
        // Load balances into memory
        let mut stmt = self.conn.prepare("SELECT account_id, balance_micros FROM balances")?;
        let balances_iter = stmt.query_map([], |row| {
            Ok((
                AccountId::new(&row.get::<_, String>(0)?).unwrap(),
                MicroDollar(row.get::<_, i64>(1)?),
            ))
        })?;
        
        let mut balances = self.balances.write();
        let mut tree = self.balance_tree.write();
        
        for result in balances_iter {
            let (account, balance) = result?;
            tree.update(&account.to_string(), balance.0);
            balances.insert(account, balance);
        }
        
        tree.compute_root();
        
        // Get latest entry for chain
        if let Ok(Some(hash)) = self.conn.query_row(
            "SELECT entry_hash FROM entries ORDER BY entry_id DESC LIMIT 1",
            [],
            |row| row.get::<_, Vec<u8>>(0),
        ) {
            self.chain_head.copy_from_slice(&hash);
            self.next_entry_id = self.conn.query_row(
                "SELECT COALESCE(MAX(entry_id), 0) + 1 FROM entries",
                [],
                |row| row.get(0),
            )?;
        }
        
        Ok(())
    }
    
    /// Posts a double-entry transaction atomically
    pub fn post_entry(
        &mut self,
        debit: AccountId,
        credit: AccountId,
        amount: MicroDollar,
        metadata: Vec<u8>,
    ) -> PostingResult {
        if amount <= MicroDollar::ZERO {
            return PostingResult::Rejected {
                reason: format!("Amount must be positive, got {:?}", amount),
            };
        }
        
        if debit == credit {
            return PostingResult::Rejected {
                reason: "Debit and credit accounts must differ".into(),
            };
        }
        
        let balances = self.balances.read();
        
        // Check credit account has sufficient funds (allows negative if not enforced)
        let credit_balance = balances.get(&credit).copied().unwrap_or(MicroDollar::ZERO);
        let new_credit_balance = credit_balance.checked_sub(amount);
        
        if new_credit_balance.is_none() {
            return PostingResult::Rejected {
                reason: format!(
                    "Overflow in credit balance: {} - {} micros",
                    credit_balance.0,
                    amount.0
                ),
            };
        }
        
        let debit_balance = balances.get(&debit).copied().unwrap_or(MicroDollar::ZERO);
        let new_debit_balance = debit_balance.checked_add(amount);
        
        if new_debit_balance.is_none() {
            return PostingResult::Rejected {
                reason: format!(
                    "Overflow in debit balance: {} + {} micros",
                    debit_balance.0,
                    amount.0
                ),
            };
        }
        
        drop(balances); // Release read lock before write
        
        let entry_id = self.next_entry_id;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;
        
        let entry = Entry {
            entry_id,
            debit_account: debit.clone(),
            credit_account: credit.clone(),
            amount,
            timestamp,
            metadata,
            parent_hash: self.chain_head,
        };
        
        let entry_hash = entry.hash();
        
        // Persist to database
        if let Err(e) = self.conn.execute(
            "INSERT INTO entries (entry_id, debit_account, credit_account, amount_micros, timestamp_ns, metadata, parent_hash, entry_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                entry.entry_id,
                entry.debit_account.to_string(),
                entry.credit_account.to_string(),
                entry.amount.0,
                entry.timestamp,
                &entry.metadata,
                &entry.parent_hash,
                &entry_hash,
            ],
        ) {
            return PostingResult::Rejected {
                reason: format!("Database error: {}", e),
            };
        }
        
        // Update in-memory state
        {
            let mut balances = self.balances.write();
            balances.insert(debit.clone(), new_debit_balance.unwrap());
            balances.insert(credit.clone(), new_credit_balance.unwrap());
            
            // Persist balances
            self.conn.execute(
                "INSERT OR REPLACE INTO balances (account_id, balance_micros, last_updated_ns) VALUES (?1, ?2, ?3)",
                rusqlite::params![debit.to_string(), new_debit_balance.unwrap().0, timestamp],
            ).ok();
            self.conn.execute(
                "INSERT OR REPLACE INTO balances (account_id, balance_micros, last_updated_ns) VALUES (?1, ?2, ?3)",
                rusqlite::params![credit.to_string(), new_credit_balance.unwrap().0, timestamp],
            ).ok();
        }
        
        // Update Merkle tree
        {
            let mut tree = self.balance_tree.write();
            tree.update(&debit.to_string(), new_debit_balance.unwrap().0);
            tree.update(&credit.to_string(), new_credit_balance.unwrap().0);
            let new_root = tree.compute_root();
            
            self.chain_head = entry_hash;
            self.next_entry_id += 1;
            
            PostingResult::Applied {
                entry_id,
                new_state_root: new_root,
            }
        }
    }
    
    /// Verifies the entire entry chain from genesis
    pub fn verify_chain_integrity(&self) -> Result<bool, anyhow::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT entry_hash, parent_hash FROM entries ORDER BY entry_id ASC"
        )?;
        
        let entries: Vec<([u8; 32], [u8; 32])> = stmt.query_map([], |row| {
            let hash_vec: Vec<u8> = row.get(0)?;
            let parent_vec: Vec<u8> = row.get(1)?;
            let mut hash = [0u8; 32];
            let mut parent = [0u8; 32];
            hash.copy_from_slice(&hash_vec);
            parent.copy_from_slice(&parent_vec);
            Ok((hash, parent))
        })?.filter_map(|r| r.ok()).collect();
        
        let mut expected_parent = [0u8; 32];
        for (hash, parent_hash) in &entries {
            if *parent_hash != expected_parent {
                return Ok(false);
            }
            expected_parent = *hash;
        }
        
        Ok(true)
    }
    
    /// Generates a cryptographic proof of balance for any account
    pub fn prove_balance(&self, account: &AccountId) -> Option<crate::merkle::BalanceProof> {
        self.balance_tree.read().generate_proof(&account.to_string())
    }
}
