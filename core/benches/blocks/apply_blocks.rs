use eyre::Result;
use iroha_core::{block::CommittedBlock, prelude::*};
use iroha_data_model::prelude::*;

#[path = "./common.rs"]
mod common;

use common::*;

pub struct WsvApplyBlocks {
    wsv: WorldStateView,
    blocks: Vec<CommittedBlock>,
}

impl WsvApplyBlocks {
    /// Create [`WorldStateView`] and blocks for benchmarking
    ///
    /// # Errors
    /// - Failed to parse [`AccountId`]
    /// - Failed to generate [`KeyPair`]
    /// - Failed to create instructions for block
    pub fn setup(rt: &tokio::runtime::Handle) -> Result<Self> {
        let domains = 100;
        let accounts_per_domain = 1000;
        let assets_per_domain = 1000;
        let account_id: AccountId = "alice@wonderland".parse()?;
        let key_pair = KeyPair::random();
        let wsv = build_wsv(rt, &account_id, &key_pair);

        let nth = 100;
        let instructions = [
            populate_wsv(domains, accounts_per_domain, assets_per_domain, &account_id),
            delete_every_nth(domains, accounts_per_domain, assets_per_domain, nth),
            restore_every_nth(domains, accounts_per_domain, assets_per_domain, nth),
        ];

        let blocks = {
            // Clone wsv because it will be changed during creation of block
            let mut wsv = wsv.clone();
            instructions
                .into_iter()
                .map(|instructions| {
                    let block = create_block(&mut wsv, instructions, account_id.clone(), &key_pair);
                    wsv.apply_without_execution(&block).map(|()| block)
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        Ok(Self { wsv, blocks })
    }

    /// Run benchmark body.
    ///
    /// # Errors
    /// - Not enough blocks
    /// - Failed to apply block
    ///
    /// # Panics
    /// If wsv isn't one block ahead of finalized wsv.
    pub fn measure(Self { wsv, blocks }: &Self) -> Result<()> {
        let mut finalized_wsv = wsv.clone();
        let mut wsv = finalized_wsv.clone();

        assert_eq!(wsv.height(), 0);
        for (block, i) in blocks.iter().zip(1..) {
            finalized_wsv = wsv.clone();
            wsv.apply(block)?;
            assert_eq!(wsv.height(), i);
            assert_eq!(wsv.height(), finalized_wsv.height() + 1);
        }

        Ok(())
    }
}
