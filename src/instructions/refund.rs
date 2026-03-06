use pinocchio::{
    AccountView, Address, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
};
use pinocchio_pubkey::derive_address;
use pinocchio_token::{instructions::TransferChecked, state::{Mint, TokenAccount}};

use crate::{
    FundraiserError,
    constants::SECONDS_TO_DAYS,
    state::{Contributor, Fundraiser},
    utils::{read_i64_le, read_u64_le, write_u64_le, validate_addr, validate_ata, validate_datasize, validate_signer},
};

pub fn process_refund_instruction(accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    let [
        contributor,
        maker,
        mint_to_raise,
        fundraiser_acc,
        contributor_acc,
        contributor_ata,
        vault,
        _token_program,
        _system_program,
        _remain @ ..,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    validate_signer(contributor)?;
    validate_datasize(data, 0)?;

    // Snapshot fundraiser data without holding a mutable borrow across the CPI
    let (
        fundraiser_maker,
        fundraiser_mint,
        fundraiser_bump,
        fundraiser_target_bytes,
        fundraiser_time_started,
        fundraiser_duration,
        fundraiser_current,
    ) = {
        let data = fundraiser_acc.try_borrow()?;
        if data.len() != Fundraiser::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        let state = Fundraiser::load(&data)?;
        (
            state.maker,
            state.mint_to_raise,
            state.bump,
            state.amount_to_raise,
            state.time_started,
            state.duration,
            state.current_amount,
        )
    };

    // Validate fundraiser PDA and mint
    let fundraiser_seeds = [b"fundraiser".as_ref(), fundraiser_maker.as_ref(), &[fundraiser_bump]];
    let expected_fundraiser = Address::from(derive_address(&fundraiser_seeds, None, &crate::ID.to_bytes()));
    validate_addr(&expected_fundraiser, fundraiser_acc, FundraiserError::InvalidFundraiser.into())?;
    validate_addr(&Address::from(fundraiser_mint), mint_to_raise, FundraiserError::InvalidFundraiser.into())?;
    validate_addr(&Address::from(fundraiser_maker), maker, FundraiserError::InvalidFundraiser.into())?;

    // Validate ATAs
    validate_ata(contributor_ata, mint_to_raise, contributor)?;
    validate_ata(vault, mint_to_raise, fundraiser_acc)?;

    // Load contributor state and validate PDA
    let (contributor_bump, contributor_amount) = {
        let mut data = contributor_acc.try_borrow_mut()?;
        if data.len() != Contributor::LEN {
            return Err(FundraiserError::InvalidContributor.into());
        }
        let state = Contributor::load_mut(&mut *data)?;
        (state.bump, read_u64_le(state.amount))
    };

    let contributor_seed = [
        b"contributor".as_ref(),
        fundraiser_acc.address().as_ref(),
        contributor.address().as_ref(),
        &[contributor_bump],
    ];
    let expected_contributor = Address::from(derive_address(&contributor_seed, None, &crate::ID.to_bytes()));
    validate_addr(&expected_contributor, contributor_acc, FundraiserError::InvalidContributor.into())?;

    if contributor_amount == 0 {
        return Err(FundraiserError::ContributionTooSmall.into());
    }

    // Time check: fundraiser must have ended
    let now = Clock::get()?.unix_timestamp;
    let elapsed_days = ((now - read_i64_le(fundraiser_time_started)) / SECONDS_TO_DAYS).max(0) as u64;
    if elapsed_days < fundraiser_duration as u64 {
        return Err(FundraiserError::FundraiserNotEnded.into());
    }

    // Token state
    let (vault_amount, decimals) = {
        let vault_state = TokenAccount::from_account_view(vault)?;
        let mint_state = Mint::from_account_view(mint_to_raise)?;
        (vault_state.amount(), mint_state.decimals())
    };

    let target = read_u64_le(fundraiser_target_bytes);
    if vault_amount >= target {
        return Err(FundraiserError::TargetMet.into());
    }
    if vault_amount < contributor_amount {
        return Err(FundraiserError::InvalidAmount.into());
    }

    // Transfer refund from vault to contributor
    let bump_bytes = [fundraiser_bump];
    let signer_seeds = [
        Seed::from(b"fundraiser"),
        Seed::from(fundraiser_maker.as_ref()),
        Seed::from(&bump_bytes),
    ];
    let fundraiser_signer = Signer::from(&signer_seeds);

    TransferChecked {
        from: vault,
        to: contributor_ata,
        authority: fundraiser_acc,
        mint: mint_to_raise,
        amount: contributor_amount,
        decimals,
    }
    .invoke_signed(&[fundraiser_signer])?;

    // Update state post-transfer
    {
        let mut fundraiser_data = fundraiser_acc.try_borrow_mut()?;
        let fundraiser_state = Fundraiser::load_mut(&mut *fundraiser_data)?;
        let current = read_u64_le(fundraiser_current).saturating_sub(contributor_amount);
        write_u64_le(&mut fundraiser_state.current_amount, current);
    }

    {
        let mut contributor_data = contributor_acc.try_borrow_mut()?;
        let contributor_state = Contributor::load_mut(&mut *contributor_data)?;
        contributor_state.amount = [0; 8];
    }

    Ok(())
}