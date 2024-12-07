//! Validation and execution logic of instructions for multisig accounts

use super::*;

impl VisitExecute for MultisigRegister {
    fn visit<V: Execute + Visit + ?Sized>(&self, _executor: &mut V) {}

    fn execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) -> Result<(), ValidationFail> {
        let multisig_account = self.account;
        let multisig_role = multisig_role_for(&multisig_account);

        // The multisig registrant needs to have sufficient permission to register personal accounts
        // TODO Loosen to just being one of the signatories? But impose the procedure of propose and approve?
        visit_seq!(executor
            .visit_register_account(&Register::account(Account::new(multisig_account.clone()))));

        let domain_owner = executor
            .host()
            .query(FindDomains)
            .filter_with(|domain| domain.id.eq(multisig_account.domain().clone()))
            .execute_single()
            .dbg_expect("domain should be found as the preceding account registration succeeded")
            .owned_by()
            .clone();

        // Authorize as the domain owner:
        // Just having permission to register accounts is insufficient to register multisig roles
        executor.context_mut().authority = domain_owner.clone();

        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account.clone(),
            SIGNATORIES.parse().unwrap(),
            Json::new(&self.signatories),
        )));

        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account.clone(),
            QUORUM.parse().unwrap(),
            Json::new(self.quorum),
        )));

        visit_seq!(executor.visit_set_account_key_value(&SetKeyValue::account(
            multisig_account.clone(),
            TRANSACTION_TTL_MS.parse().unwrap(),
            Json::new(self.transaction_ttl_ms),
        )));

        visit_seq!(executor.visit_register_role(&Register::role(
            // Temporarily grant a multisig role to the domain owner to delegate the role to the signatories
            Role::new(multisig_role.clone(), domain_owner.clone()),
        )));

        for signatory in self.signatories.keys().cloned() {
            visit_seq!(executor
                .visit_grant_account_role(&Grant::account_role(multisig_role.clone(), signatory)));
        }

        visit_seq!(
            executor.visit_revoke_account_role(&Revoke::account_role(multisig_role, domain_owner))
        );

        Ok(())
    }
}
