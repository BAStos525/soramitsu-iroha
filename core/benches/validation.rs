#![allow(missing_docs, clippy::restriction)]

use std::{collections::BTreeSet, str::FromStr as _, sync::Arc};

use criterion::{criterion_group, criterion_main, Criterion};
use iroha_core::{
    prelude::*,
    tx::{AcceptedTransaction, TransactionOrigin, TransactionValidator},
    wsv::World,
};
use iroha_data_model::prelude::*;

const TRANSACTION_TIME_TO_LIVE_MS: u64 = 100_000;

const START_DOMAIN: &str = "start";
const START_ACCOUNT: &str = "starter";

const TRANSACTION_LIMITS: TransactionLimits = TransactionLimits {
    max_instruction_number: 4096,
    max_wasm_size_bytes: 0,
};

fn build_test_transaction(keys: KeyPair) -> SignedTransaction {
    let domain_name = "domain";
    let domain_id = DomainId::from_str(domain_name).expect("does not panic");
    let create_domain = RegisterBox::new(Domain::new(domain_id));
    let account_name = "account";
    let (public_key, _) = KeyPair::generate()
        .expect("Failed to generate KeyPair.")
        .into();
    let create_account = RegisterBox::new(Account::new(
        AccountId::new(
            account_name.parse().expect("Valid"),
            domain_name.parse().expect("Valid"),
        ),
        [public_key],
    ));
    let asset_definition_id = AssetDefinitionId::new(
        "xor".parse().expect("Valid"),
        domain_name.parse().expect("Valid"),
    );
    let create_asset = RegisterBox::new(AssetDefinition::quantity(asset_definition_id));
    let instructions: Vec<Instruction> = vec![
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ];
    Transaction::new(
        AccountId::new(
            START_ACCOUNT.parse().expect("Valid"),
            START_DOMAIN.parse().expect("Valid"),
        ),
        instructions.into(),
        TRANSACTION_TIME_TO_LIVE_MS,
    )
    .sign(keys)
    .expect("Failed to sign.")
}

fn build_test_and_transient_wsv(keys: KeyPair) -> WorldStateView {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let (public_key, _) = keys.into();

    WorldStateView::new(
        {
            let domain_id = DomainId::from_str(START_DOMAIN).expect("Valid");
            let mut domain = Domain::new(domain_id).build();
            let account_id = AccountId::new(
                START_ACCOUNT.parse().expect("Valid"),
                START_DOMAIN.parse().expect("Valid"),
            );
            let account = Account::new(account_id, [public_key]).build();
            assert!(domain.add_account(account).is_none());
            World::with([domain], BTreeSet::new())
        },
        kura,
    )
}

fn accept_transaction(criterion: &mut Criterion) {
    let keys = KeyPair::generate().expect("Failed to generate keys");
    let transaction = build_test_transaction(keys);
    let mut success_count = 0;
    let mut failures_count = 0;
    let _ = criterion.bench_function("accept", |b| {
        b.iter(|| {
            match AcceptedTransaction::from_transaction::<{ TransactionOrigin::ConsensusBlock }>(
                transaction.clone(),
                &TRANSACTION_LIMITS,
            ) {
                Ok(_) => success_count += 1,
                Err(_) => failures_count += 1,
            }
        });
    });
    println!("Success count: {success_count}, Failures count: {failures_count}");
}

fn sign_transaction(criterion: &mut Criterion) {
    let keys = KeyPair::generate().expect("Failed to generate keys");
    let transaction = build_test_transaction(keys);
    let key_pair = KeyPair::generate().expect("Failed to generate KeyPair.");
    let mut success_count = 0;
    let mut failures_count = 0;
    let _ = criterion.bench_function("sign", |b| {
        b.iter(|| match transaction.clone().sign(key_pair.clone()) {
            Ok(_) => success_count += 1,
            Err(_) => failures_count += 1,
        });
    });
    println!("Success count: {success_count}, Failures count: {failures_count}");
}

fn validate_transaction(criterion: &mut Criterion) {
    let keys = KeyPair::generate().expect("Failed to generate keys");
    let transaction =
        AcceptedTransaction::from_transaction::<{ TransactionOrigin::ConsensusBlock }>(
            build_test_transaction(keys.clone()),
            &TRANSACTION_LIMITS,
        )
        .expect("Failed to accept transaction.");
    let mut success_count = 0;
    let mut failure_count = 0;
    let _ = criterion.bench_function("validate", move |b| {
        let transaction_validator = TransactionValidator::new(
            TRANSACTION_LIMITS,
            Arc::new(AllowAll::new()),
            Arc::new(AllowAll::new()),
        );
        b.iter(|| {
            match transaction_validator.validate(
                transaction.clone(),
                false,
                &Arc::new(build_test_and_transient_wsv(keys.clone())),
            ) {
                Ok(_) => success_count += 1,
                Err(_) => failure_count += 1,
            }
        });
    });
    println!("Success count: {success_count}, Failure count: {failure_count}");
}

fn chain_blocks(criterion: &mut Criterion) {
    let keys = KeyPair::generate().expect("Failed to generate keys");
    let transaction =
        AcceptedTransaction::from_transaction::<{ TransactionOrigin::ConsensusBlock }>(
            build_test_transaction(keys),
            &TRANSACTION_LIMITS,
        )
        .expect("Failed to accept transaction.");
    let block = PendingBlock::new(vec![transaction.into()], Vec::new());
    let mut previous_block_hash = block.clone().chain_first().hash();
    let mut success_count = 0;
    let _ = criterion.bench_function("chain_block", |b| {
        b.iter(|| {
            success_count += 1;
            let new_block =
                block
                    .clone()
                    .chain(success_count, Some(previous_block_hash.transmute()), 0);
            previous_block_hash = new_block.hash();
        });
    });
    println!("Total count: {success_count}");
}

fn sign_blocks(criterion: &mut Criterion) {
    let keys = KeyPair::generate().expect("Failed to generate keys");
    let transaction =
        AcceptedTransaction::from_transaction::<{ TransactionOrigin::ConsensusBlock }>(
            build_test_transaction(keys.clone()),
            &TRANSACTION_LIMITS,
        )
        .expect("Failed to accept transaction.");
    let transaction_validator = TransactionValidator::new(
        TRANSACTION_LIMITS,
        Arc::new(AllowAll::new()),
        Arc::new(AllowAll::new()),
    );
    let block = PendingBlock::new(vec![transaction.into()], Vec::new())
        .chain_first()
        .validate(
            &transaction_validator,
            &Arc::new(build_test_and_transient_wsv(keys)),
        );
    let key_pair = KeyPair::generate().expect("Failed to generate KeyPair.");
    let mut success_count = 0;
    let mut failures_count = 0;
    let _ = criterion.bench_function("sign_block", |b| {
        b.iter(|| match block.clone().sign(key_pair.clone()) {
            Ok(_) => success_count += 1,
            Err(_) => failures_count += 1,
        });
    });
    println!("Success count: {success_count}, Failures count: {failures_count}");
}

fn validate_blocks(criterion: &mut Criterion) {
    // Prepare WSV
    let key_pair = KeyPair::generate().expect("Failed to generate KeyPair.");
    let domain_name = "global";
    let account_id = AccountId::new(
        "root".parse().expect("Valid"),
        domain_name.parse().expect("Valid"),
    );
    let (public_key, _) = key_pair.into();
    let account = Account::new(account_id, [public_key]).build();
    let domain_id = DomainId::from_str(domain_name).expect("is valid");
    let mut domain = Domain::new(domain_id).build();
    assert!(domain.add_account(account).is_none());
    // Pepare test transaction
    let keys = KeyPair::generate().expect("Failed to generate keys");
    let transaction =
        AcceptedTransaction::from_transaction::<{ TransactionOrigin::ConsensusBlock }>(
            build_test_transaction(keys),
            &TRANSACTION_LIMITS,
        )
        .expect("Failed to accept transaction.");
    let block = PendingBlock::new(vec![transaction.into()], Vec::new()).chain_first();
    let transaction_validator = TransactionValidator::new(
        TRANSACTION_LIMITS,
        Arc::new(AllowAll::new()),
        Arc::new(AllowAll::new()),
    );
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let _ = criterion.bench_function("validate_block", |b| {
        b.iter(|| {
            block.clone().validate(
                &transaction_validator,
                &Arc::new(WorldStateView::new(
                    World::with([domain.clone()], BTreeSet::new()),
                    kura.clone(),
                )),
            )
        });
    });
}

criterion_group!(
    transactions,
    accept_transaction,
    sign_transaction,
    validate_transaction,
    validate_blocks
);
criterion_group!(
    blocks,
    chain_blocks,
    sign_blocks, // validate_blocks
);
criterion_main!(transactions, blocks);
