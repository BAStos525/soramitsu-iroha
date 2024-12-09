//! Validation and execution logic of instructions for multisig transactions

use alloc::collections::{btree_map::BTreeMap, btree_set::BTreeSet};

use super::*;

impl VisitExecute for MultisigPropose {
    fn visit<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        let proposer = executor.context().authority.clone();
        let multisig_account = self.account.clone();
        let host = executor.host();
        let instructions_hash = HashOf::new(&self.instructions);
        let multisig_role = multisig_role_for(&multisig_account);
        let is_downward_proposal = host
            .query_single(FindAccountMetadata::new(
                proposer.clone(),
                SIGNATORIES.parse().unwrap(),
            ))
            .map_or(false, |proposer_signatories| {
                proposer_signatories
                    .try_into_any::<BTreeMap<AccountId, u8>>()
                    .dbg_unwrap()
                    .contains_key(&multisig_account)
            });
        let has_multisig_role = host
            .query(FindRolesByAccountId::new(proposer))
            .filter_with(|role_id| role_id.eq(multisig_role))
            .execute_single()
            .is_ok();

        if !(is_downward_proposal || has_multisig_role) {
            deny!(executor, "not qualified to propose multisig");
        };

        if host
            .query_single(FindAccountMetadata::new(
                multisig_account.clone(),
                approvals_key(&instructions_hash),
            ))
            .is_ok()
        {
            deny!(executor, "multisig proposal duplicates")
        };
    }

    fn execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) -> Result<(), ValidationFail> {
        let proposer = executor.context().authority.clone();
        let multisig_account = self.account;

        // Authorize as the multisig account
        executor.context_mut().authority = multisig_account.clone();

        let instructions_hash = HashOf::new(&self.instructions);
        let signatories: BTreeMap<AccountId, u8> = executor
            .host()
            .query_single(FindAccountMetadata::new(
                multisig_account.clone(),
                SIGNATORIES.parse().unwrap(),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let now_ms: u64 = executor
            .context()
            .curr_block
            .creation_time()
            .as_millis()
            .try_into()
            .dbg_expect("shouldn't overflow within 584942417 years");
        let approvals = BTreeSet::from([proposer]);

        // Recursively deploy multisig authentication down to the personal leaf signatories
        for signatory in signatories.keys().cloned() {
            let is_multisig_again = executor
                .host()
                .query(FindRoleIds)
                .filter_with(|role_id| role_id.eq(multisig_role_for(&signatory)))
                .execute_single_opt()
                .dbg_unwrap()
                .is_some();

            if is_multisig_again {
                let propose_to_approve_me = {
                    let approve_me =
                        MultisigApprove::new(multisig_account.clone(), instructions_hash);

                    MultisigPropose::new(signatory, [approve_me.into()].to_vec())
                };

                propose_to_approve_me.visit_execute(executor);
            }
        }

        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account.clone(),
            instructions_key(&instructions_hash).clone(),
            Json::new(&self.instructions),
        )));

        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account.clone(),
            proposed_at_ms_key(&instructions_hash).clone(),
            Json::new(now_ms),
        )));

        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account,
            approvals_key(&instructions_hash).clone(),
            Json::new(&approvals),
        )));

        Ok(())
    }
}

impl VisitExecute for MultisigApprove {
    fn visit<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        let approver = executor.context().authority.clone();
        let multisig_account = self.account.clone();
        let host = executor.host();
        let multisig_role = multisig_role_for(&multisig_account);

        if host
            .query(FindRolesByAccountId::new(approver))
            .filter_with(|role_id| role_id.eq(multisig_role))
            .execute_single()
            .is_err()
        {
            deny!(executor, "not qualified to approve multisig");
        };
    }

    fn execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) -> Result<(), ValidationFail> {
        let approver = executor.context().authority.clone();
        let multisig_account = self.account;

        // Authorize as the multisig account
        executor.context_mut().authority = multisig_account.clone();

        let host = executor.host();
        let instructions_hash = self.instructions_hash;
        let signatories: BTreeMap<AccountId, u8> = host
            .query_single(FindAccountMetadata::new(
                multisig_account.clone(),
                SIGNATORIES.parse().unwrap(),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let quorum: u16 = host
            .query_single(FindAccountMetadata::new(
                multisig_account.clone(),
                QUORUM.parse().unwrap(),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let transaction_ttl_ms: u64 = host
            .query_single(FindAccountMetadata::new(
                multisig_account.clone(),
                TRANSACTION_TTL_MS.parse().unwrap(),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let instructions: Vec<InstructionBox> = host
            .query_single(FindAccountMetadata::new(
                multisig_account.clone(),
                instructions_key(&instructions_hash),
            ))?
            .try_into_any()
            .dbg_unwrap();
        let proposed_at_ms: u64 = host
            .query_single(FindAccountMetadata::new(
                multisig_account.clone(),
                proposed_at_ms_key(&instructions_hash),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();
        let now_ms: u64 = executor
            .context()
            .curr_block
            .creation_time()
            .as_millis()
            .try_into()
            .dbg_expect("shouldn't overflow within 584942417 years");
        let mut approvals: BTreeSet<AccountId> = host
            .query_single(FindAccountMetadata::new(
                multisig_account.clone(),
                approvals_key(&instructions_hash),
            ))
            .dbg_unwrap()
            .try_into_any()
            .dbg_unwrap();

        approvals.insert(approver);

        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account.clone(),
            approvals_key(&instructions_hash),
            Json::new(&approvals),
        )));

        let is_authenticated = quorum
            <= signatories
                .into_iter()
                .filter(|(id, _)| approvals.contains(id))
                .map(|(_, weight)| u16::from(weight))
                .sum();

        let is_expired = proposed_at_ms.saturating_add(transaction_ttl_ms) < now_ms;

        if is_authenticated || is_expired {
            // Cleanup the transaction entry
            visit_seq!(
                executor.visit_remove_account_key_value(&RemoveKeyValue::account(
                    multisig_account.clone(),
                    approvals_key(&instructions_hash),
                ))
            );

            visit_seq!(
                executor.visit_remove_account_key_value(&RemoveKeyValue::account(
                    multisig_account.clone(),
                    proposed_at_ms_key(&instructions_hash),
                ))
            );

            visit_seq!(
                executor.visit_remove_account_key_value(&RemoveKeyValue::account(
                    multisig_account.clone(),
                    instructions_key(&instructions_hash),
                ))
            );

            if is_expired {
                // TODO Notify that the proposal has expired, while returning Ok for the entry deletion to take effect
            } else {
                // Validate and execute the authenticated multisig transaction
                for instruction in instructions {
                    visit_seq!(executor.visit_instruction(&instruction));
                }
            }
        }

        Ok(())
    }
}
