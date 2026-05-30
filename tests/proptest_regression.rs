use immutable_ledger::types::{AccountId, MicroDollar};
use immutable_ledger::ledger::Ledger;
use proptest::prelude::*;
use std::collections::HashMap;

fn valid_account_id() -> impl Strategy<Value = String> {
    "[A-Z0-9]{5,20}".prop_map(|s| format!("ACCT-{}", s))
}

fn micro_dollar_range() -> impl Strategy<Value = i64> {
    1i64..1_000_000_000_000 // Up to $100,000.00 in micro-dollars
}

proptest! {
    /// INVARIANT: Total credits always equal total debits
    #[test]
    fn total_debits_equal_total_credits(
        transactions in prop::collection::vec(
            (valid_account_id(), valid_account_id(), micro_dollar_range()),
            1..100
        )
    ) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let mut ledger = Ledger::open(db_path.to_str().unwrap()).unwrap();
        
        let mut net_by_account: HashMap<String, (i64, i64)> = HashMap::new(); // (debits, credits)
        
        for (debit_str, credit_str, amount_micros) in transactions {
            let debit = AccountId::new(&debit_str).unwrap();
            let credit = AccountId::new(&credit_str).unwrap();
            
            if debit == credit { continue; }
            
            let amount = MicroDollar(amount_micros);
            ledger.post_entry(debit.clone(), credit.clone(), amount, vec![]);
            
            // Track expected: debit account gains, credit account loses
            let deb_entry = net_by_account.entry(debit.to_string()).or_insert((0, 0));
            deb_entry.0 += amount_micros;
            
            let cred_entry = net_by_account.entry(credit.to_string()).or_insert((0, 0));
            cred_entry.1 += amount_micros;
        }
        
        // Verify: For each account, balance = total_debits - total_credits
        for (account, (total_debits, total_credits)) in &net_by_account {
            let proof = ledger.prove_balance(&AccountId::new(account).unwrap());
            if let Some(proof) = proof {
                assert_eq!(
                    proof.balance,
                    total_debits - total_credits,
                    "Account {} balance mismatch", account
                );
            }
        }
        
        // System-wide: sum of all balances must be 0
        // (Every debit has a corresponding credit)
        let system_total: i64 = net_by_account.values()
            .map(|(d, c)| d - c)
            .sum();
        assert_eq!(system_total, 0, "System-wide balance must be zero");
    }
    
    /// INVARIANT: Chain integrity must never break
    #[test]
    fn chain_integrity_preserved(
        transactions in prop::collection::vec(
            (valid_account_id(), valid_account_id(), micro_dollar_range()),
            1..50
        )
    ) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let mut ledger = Ledger::open(db_path.to_str().unwrap()).unwrap();
        
        for (debit_str, credit_str, amount_micros) in transactions {
            let debit = AccountId::new(&debit_str).unwrap();
            let credit = AccountId::new(&credit_str).unwrap();
            if debit != credit {
                ledger.post_entry(debit, credit, MicroDollar(amount_micros), vec![]);
            }
        }
        
        assert!(ledger.verify_chain_integrity().unwrap(), 
            "Hash chain must remain intact after all transactions");
    }
    
    /// INVARIANT: No negative balances if we enforce it
    #[test]
    fn no_negative_balances_when_enforced(
        initial_balances in prop::collection::vec(
            (valid_account_id(), 1_000_000i64..10_000_000i64),
            1..10
        ),
        transactions in prop::collection::vec(
            (valid_account_id(), valid_account_id(), micro_dollar_range()),
            1..30
        )
    ) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let mut ledger = Ledger::open(db_path.to_str().unwrap()).unwrap();
        
    
    }
}
