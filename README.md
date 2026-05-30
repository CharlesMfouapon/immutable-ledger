# immutable-ledger
Cryptographically verifiable double-entry ledger with Merkle proofs. Immutable audit trail, deterministic state transitions, ACID-compliant backing store.

[![Rust CI](https://github.com/CharlesMfouapon/immutable-ledger/actions/workflows/ci.yml/badge.svg)](https://github.com/YOUR_USERNAME/immutable-ledger/actions/workflows/ci.yml)
[![Rust 1.75+](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust)](https://rust-lang.org)
[![Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

**Cryptographically verifiable double-entry ledger.** Every transaction is hash-chained. Every balance is Merkle-provable. Every state transition is deterministic.

## Core Guarantees
- **Immutability**: Hash-chained journal entries detect any tampering
- **Verifiability**: Merkle proofs for any account balance without full state
- **Conservation**: Σ(debits) ≡ Σ(credits) — provably, via property tests
- **ACID**: SQLite-backed transactions with row-level locking

## Quick Start
```bash
git clone https://github.com/CharlesMfouapon/immutable-ledger.git
cd immutable-ledger
cargo test --release
cargo bench
```

Usage Example

```rust
use immutable_ledger::types::{AccountId, MicroDollar};
use immutable_ledger::ledger::Ledger;

let mut ledger = Ledger::open("bank.db").unwrap();

// Post a $100.00 transfer
let result = ledger.post_entry(
    AccountId::new("ASSET-CASH").unwrap(),
    AccountId::new("LIAB-CUSTOMER-001").unwrap(),
    MicroDollar::from_dollars_cents(100, 0),
    vec![],
);

// Verify entire chain integrity
assert!(ledger.verify_chain_integrity().unwrap());

// Generate cryptographic balance proof
let proof = ledger.prove_balance(&AccountId::new("ASSET-CASH").unwrap());
```

## Property-Based Testing

We use proptest to verify mathematical invariants across millions of randomized transaction sequences:

* [x] Total debits = Total credits (system-wide zero)
* [x] Hash chain integrity never breaks
* [x] Balance proofs validate against Merkle root

See [tests/proptest_regression.rs](https://github.com/CharlesMfouapon/immutable-ledger/test/proptest_regression.rs)

```
