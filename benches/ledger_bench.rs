use criterion::{black_box, criterion_group, criterion_main, Criterion};
use immutable_ledger::types::{AccountId, MicroDollar};
use immutable_ledger::ledger::Ledger;
use tempfile::TempDir;

fn bench_post_entry(c: &mut Criterion) {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("bench.db");
    let mut ledger = Ledger::open(path.to_str().unwrap()).unwrap();
    
    let debit = AccountId::new("ASSET-CASH").unwrap();
    let credit = AccountId::new("LIAB-DEPOSIT").unwrap();
    
    c.bench_function("post_single_entry", |b| {
        b.iter(|| {
            ledger.post_entry(
                debit.clone(),
                credit.clone(),
                MicroDollar(1_000_000_000), // $100.00
                vec![],
            )
        })
    });
}

criterion_group!(benches, bench_post_entry);
criterion_main!(benches);
