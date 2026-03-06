#[cfg(test)]
mod helpers;

#[cfg(test)]
mod tests {
	use super::helpers::{build_check_ix, build_contribute_ix, build_initialize_ix, build_refund_ix, send_ix, set_clock, setup_init, InitSetup};
	use crate::{constants::SECONDS_TO_DAYS, state::{Contributor, Fundraiser}};
	use litesvm_token::{get_spl_account, spl_token::state::Account as TokenAccount, CreateAssociatedTokenAccount, MintTo};
    use solana_signer::Signer;
	use solana_pubkey::Pubkey;

	#[test]
	fn initialize_fundraiser_sets_fields() {
		let InitSetup {
			mut svm,
			payer,
			mint,
			fundraiser_pda,
			bump,
		} = setup_init(6);

		let amount = 1_000_000u64;
		let duration = 7u8;

		let ix = build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, amount, duration);

		let _cu = send_ix(&mut svm, &payer, ix);

		let account = svm
			.get_account(&fundraiser_pda)
			.expect("fundraiser account exists");
		assert_eq!(account.data.len(), Fundraiser::LEN);

		let fundraiser = Fundraiser::load(&account.data).expect("deserialize fundraiser");

		assert_eq!(fundraiser.maker, payer.pubkey().to_bytes());
		assert_eq!(fundraiser.mint_to_raise, mint.to_bytes());
		assert_eq!(u64::from_le_bytes(fundraiser.amount_to_raise), amount);
		assert_eq!(u64::from_le_bytes(fundraiser.current_amount), 0);
		assert!(i64::from_le_bytes(fundraiser.time_started) > 0);
		assert_eq!(fundraiser.duration, duration);
		assert_eq!(fundraiser.bump, bump);
	}

	#[test]
	fn contribute_creates_contributor_and_updates_amounts() {
		let InitSetup {
			mut svm,
			payer,
			mint,
			fundraiser_pda,
			bump,
		} = setup_init(6);

		let amount_to_raise = 1_000_000u64;
		let duration = 7u8;

		let init_ix = build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, amount_to_raise, duration);
		let _ = send_ix(&mut svm, &payer, init_ix);

		let contributor_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&payer.pubkey())
			.send()
			.expect("create contributor ata");

		let vault = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&fundraiser_pda)
			.send()
			.expect("create vault ata");

		MintTo::new(&mut svm, &payer, &mint, &contributor_ata, 200_000)
			.send()
			.expect("mint to contributor");

		let (contributor_pda, contributor_bump) = Pubkey::find_program_address(
			&[b"contributor", fundraiser_pda.as_ref(), payer.pubkey().as_ref()],
			&super::helpers::program_id(),
		);

		let contribution = 50_000u64;
		let contribute_ix = build_contribute_ix(
			&payer,
			&mint,
			&fundraiser_pda,
			&contributor_pda,
			contributor_bump,
			&contributor_ata,
			&vault,
			contribution,
		);

		let _ = send_ix(&mut svm, &payer, contribute_ix);

		let fundraiser_account = svm
			.get_account(&fundraiser_pda)
			.expect("fundraiser account exists");
		assert_eq!(fundraiser_account.data.len(), Fundraiser::LEN);
		let fundraiser = Fundraiser::load(&fundraiser_account.data).expect("deserialize fundraiser");
		assert_eq!(u64::from_le_bytes(fundraiser.current_amount), contribution);

		let contributor_account = svm
			.get_account(&contributor_pda)
			.expect("contributor account exists");
		assert_eq!(contributor_account.data.len(), Contributor::LEN);
		let contributor_state = Contributor::load(&contributor_account.data).expect("deserialize contributor");
		assert_eq!(u64::from_le_bytes(contributor_state.amount), contribution);
		assert_eq!(contributor_state.bump, contributor_bump);
	}

	#[test]
	fn check_transfers_vault_to_maker() {
		let InitSetup {
			mut svm,
			payer,
			mint,
			fundraiser_pda,
			bump,
		} = setup_init(6);

		let amount_to_raise = 100_000u64;
		let duration = 7u8;

		let init_ix = build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, amount_to_raise, duration);
		let _ = send_ix(&mut svm, &payer, init_ix);

		let vault = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&fundraiser_pda)
			.send()
			.expect("create vault ata");
		let maker_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&payer.pubkey())
			.send()
			.expect("create maker ata");

		MintTo::new(&mut svm, &payer, &mint, &vault, amount_to_raise)
			.send()
			.expect("mint to vault");

		let check_ix = build_check_ix(
			&payer,
			&mint,
			&fundraiser_pda,
			&maker_ata,
			&vault,
		);

		let _ = send_ix(&mut svm, &payer, check_ix);

		let maker_token: TokenAccount = get_spl_account(&svm, &maker_ata).expect("maker ata exists");
		assert_eq!(maker_token.amount, amount_to_raise);

		let vault_token: TokenAccount = get_spl_account(&svm, &vault).expect("vault ata exists");
		assert_eq!(vault_token.amount, 0);
	}

	#[test]
	fn refund_returns_contribution_after_expiry() {
		let InitSetup {
			mut svm,
			payer,
			mint,
			fundraiser_pda,
			bump,
		} = setup_init(6);

		let amount_to_raise = 100_000u64;
		let duration_days = 1u8;

		let init_ix = build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, amount_to_raise, duration_days);
		let _ = send_ix(&mut svm, &payer, init_ix);

		let vault = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&fundraiser_pda)
			.send()
			.expect("create vault ata");
		let contributor_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&payer.pubkey())
			.send()
			.expect("create contributor ata");

		let (contributor_pda, contributor_bump) = Pubkey::find_program_address(
			&[b"contributor", fundraiser_pda.as_ref(), payer.pubkey().as_ref()],
			&super::helpers::program_id(),
		);

		let contribution = 10_000u64;
		// Simulate contributed funds sitting in the vault
		MintTo::new(&mut svm, &payer, &mint, &vault, contribution)
			.send()
			.expect("mint to vault");

		let contribute_ix = build_contribute_ix(
			&payer,
			&mint,
			&fundraiser_pda,
			&contributor_pda,
			contributor_bump,
			&contributor_ata,
			&vault,
			contribution,
		);
		let _ = send_ix(&mut svm, &payer, contribute_ix);

		// Advance clock beyond the fundraiser duration
		set_clock(&mut svm, (SECONDS_TO_DAYS * (duration_days as i64 + 1)) + 5);

		let refund_ix = build_refund_ix(
			&payer,
			&payer.pubkey(),
			&mint,
			&fundraiser_pda,
			&contributor_pda,
			&contributor_ata,
			&vault,
		);
		let _ = send_ix(&mut svm, &payer, refund_ix);

		let contributor_token: TokenAccount = get_spl_account(&svm, &contributor_ata).expect("contributor ata exists");
		assert_eq!(contributor_token.amount, contribution);

		let vault_token: TokenAccount = get_spl_account(&svm, &vault).expect("vault ata exists");
		assert_eq!(vault_token.amount, 0);

		let fundraiser_account = svm
			.get_account(&fundraiser_pda)
			.expect("fundraiser account exists");
		let fundraiser = Fundraiser::load(&fundraiser_account.data).expect("deserialize fundraiser");
		assert_eq!(u64::from_le_bytes(fundraiser.current_amount), 0);

		let contributor_account = svm
			.get_account(&contributor_pda)
			.expect("contributor account exists");
		let contributor_state = Contributor::load(&contributor_account.data).expect("deserialize contributor");
		assert_eq!(u64::from_le_bytes(contributor_state.amount), 0);
	}

	#[test]
	fn compute_units_report() {
		// initialize
		let InitSetup { mut svm, payer, mint, fundraiser_pda, bump } = setup_init(6);
		let cu_init = send_ix(&mut svm, &payer, build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, 100_000, 1));
		assert!(cu_init > 0);

		// contribute
		let InitSetup { mut svm, payer, mint, fundraiser_pda, bump } = setup_init(6);
		let _ = send_ix(&mut svm, &payer, build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, 100_000, 1));
		let contributor_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&payer.pubkey())
			.send()
			.expect("create contributor ata");
		let vault = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&fundraiser_pda)
			.send()
			.expect("create vault ata");
		let (contributor_pda, contributor_bump) = Pubkey::find_program_address(
			&[b"contributor", fundraiser_pda.as_ref(), payer.pubkey().as_ref()],
			&super::helpers::program_id(),
		);
		let cu_contrib = send_ix(
			&mut svm,
			&payer,
			build_contribute_ix(
				&payer,
				&mint,
				&fundraiser_pda,
				&contributor_pda,
				contributor_bump,
				&contributor_ata,
				&vault,
				10_000,
			),
		);
		assert!(cu_contrib > 0);

		// checker
		let InitSetup { mut svm, payer, mint, fundraiser_pda, bump } = setup_init(6);
		let target = 50_000u64;
		let _ = send_ix(&mut svm, &payer, build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, target, 1));
		let vault = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&fundraiser_pda)
			.send()
			.expect("create vault ata");
		let maker_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&payer.pubkey())
			.send()
			.expect("create maker ata");
		MintTo::new(&mut svm, &payer, &mint, &vault, target)
			.send()
			.expect("mint to vault");
		let cu_checker = send_ix(&mut svm, &payer, build_check_ix(&payer, &mint, &fundraiser_pda, &maker_ata, &vault));
		assert!(cu_checker > 0);

		// refund
		let InitSetup { mut svm, payer, mint, fundraiser_pda, bump } = setup_init(6);
		let _ = send_ix(&mut svm, &payer, build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, 100_000, 1));
		let vault = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&fundraiser_pda)
			.send()
			.expect("create vault ata");
		let contributor_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&payer.pubkey())
			.send()
			.expect("create contributor ata");
		let (contributor_pda, contributor_bump) = Pubkey::find_program_address(
			&[b"contributor", fundraiser_pda.as_ref(), payer.pubkey().as_ref()],
			&super::helpers::program_id(),
		);
		let contribution = 10_000u64;
		MintTo::new(&mut svm, &payer, &mint, &vault, contribution)
			.send()
			.expect("mint to vault");
		let _ = send_ix(
			&mut svm,
			&payer,
			build_contribute_ix(
				&payer,
				&mint,
				&fundraiser_pda,
				&contributor_pda,
				contributor_bump,
				&contributor_ata,
				&vault,
				contribution,
			),
		);
		set_clock(&mut svm, (SECONDS_TO_DAYS * 2) + 5);
		let cu_refund = send_ix(
			&mut svm,
			&payer,
			build_refund_ix(
				&payer,
				&payer.pubkey(),
				&mint,
				&fundraiser_pda,
				&contributor_pda,
				&contributor_ata,
				&vault,
			),
		);
		assert!(cu_refund > 0);

		println!(
			"\nCompute Units per instruction:\n| Instruction | CU |\n|-------------|------|\n| initialize  | {cu_init} |\n| contribute  | {cu_contrib} |\n| checker     | {cu_checker} |\n| refund      | {cu_refund} |\n"
		);
	}

	#[test]
	fn compute_units_contribute_paths() {
		// Measures contribute when contributor PDA is auto-created vs already initialized.
		let amount_to_raise = 100_000u64;
		let first_contribution = 6_000u64;
		let second_contribution = 3_000u64;

		// Path 1: contributor PDA not yet initialized (triggers CreateAccount)
		let InitSetup { mut svm, payer, mint, fundraiser_pda, bump } = setup_init(6);
		let _ = send_ix(&mut svm, &payer, build_initialize_ix(&payer, &mint, &fundraiser_pda, bump, amount_to_raise, 2));
		let contributor_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&payer.pubkey())
			.send()
			.expect("create contributor ata");
		let vault = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
			.owner(&fundraiser_pda)
			.send()
			.expect("create vault ata");
		let (contributor_pda, contributor_bump) = Pubkey::find_program_address(
			&[b"contributor", fundraiser_pda.as_ref(), payer.pubkey().as_ref()],
			&super::helpers::program_id(),
		);
		let cu_contrib_init = send_ix(
			&mut svm,
			&payer,
			build_contribute_ix(
				&payer,
				&mint,
				&fundraiser_pda,
				&contributor_pda,
				contributor_bump,
				&contributor_ata,
				&vault,
				first_contribution,
			),
		);
		assert!(cu_contrib_init > 0);

		// Path 2: contributor PDA already exists (no CreateAccount)
		let cu_contrib_existing = send_ix(
			&mut svm,
			&payer,
			build_contribute_ix(
				&payer,
				&mint,
				&fundraiser_pda,
				&contributor_pda,
				contributor_bump,
				&contributor_ata,
				&vault,
				second_contribution,
			),
		);
		assert!(cu_contrib_existing > 0);

		println!(
			"\nCompute Units for contribute paths:\n| Scenario                | CU |\n|------------------------|------|\n| contribute (init PDA)  | {cu_contrib_init} |\n| contribute (existing)  | {cu_contrib_existing} |\n"
		);
	}
}