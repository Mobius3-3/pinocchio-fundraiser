use bytemuck::{Pod, Zeroable};
use pinocchio::{
    AccountView, Address, ProgramResult, cpi::{Seed, Signer}, error::ProgramError, sysvars::{Sysvar, clock::Clock, rent::Rent}
};
use pinocchio_pubkey::derive_address;
use pinocchio_system::instructions::CreateAccount;
use crate::{
    FundraiserError, constants::{MAX_CONTRIBUTION_PERCENTAGE, PERCENTAGE_SCALER, SECONDS_TO_DAYS}, state::{Contributor, Fundraiser}, utils::{read_i64_le, read_u64_le, write_u64_le, validate_signer, validate_datasize, validate_addr, validate_xeq, validate_ata}
};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ContributeData {
    amount: [u8; 8],
    bump: u8,
}

impl ContributeData {
    pub const LEN: usize = core::mem::size_of::<ContributeData>();
}

pub fn process_contribute_instruction(accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    let [contributor, mint_to_raise, fundraiser_acc, contributor_acc, contributor_ata, vault, _token_program, _system_program, _remain @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // reusable validations
    validate_signer(contributor)?;
    validate_datasize(data, ContributeData::LEN)?;
    validate_ata(vault, mint_to_raise, fundraiser_acc)?;
    validate_ata(contributor_ata, mint_to_raise, contributor)?;

    // manually validations
    let ix_data = bytemuck::from_bytes::<ContributeData>(data);
    let amount = read_u64_le(ix_data.amount);
    let contributor_bump = ix_data.bump;
    validate_xeq(amount, 0u64, FundraiserError::ContributionTooSmall.into())?;

    let mut fundraiser_data = fundraiser_acc.try_borrow_mut()?;
    validate_datasize(&fundraiser_data, Fundraiser::LEN)?;
    let fundraiser_state = Fundraiser::load_mut(&mut *fundraiser_data)?;
    validate_addr(&Address::from(fundraiser_state.mint_to_raise), mint_to_raise, FundraiserError::InvalidFundraiser.into())?;
    let seed = [b"fundraiser".as_ref(), fundraiser_state.maker.as_ref(), &[fundraiser_state.bump]];
    let expected_fundraiser_addr = Address::from(derive_address(&seed, None, &crate::ID.to_bytes()));
    validate_addr(&expected_fundraiser_addr, fundraiser_acc, FundraiserError::InvalidFundraiser.into())?;

    let contributor_seed = [
        b"contributor".as_ref(),
        fundraiser_acc.address().as_ref(),
        contributor.address().as_ref(),
        &[contributor_bump],
    ];
    let expected_contributor_addr = Address::from(derive_address(&contributor_seed, None, &crate::ID.to_bytes()));
    validate_addr(&expected_contributor_addr, contributor_acc, FundraiserError::InvalidContributor.into())?;

    let now = Clock::get()?.unix_timestamp;
    let time_started = read_i64_le(fundraiser_state.time_started);
    let elapsed_days = ((now - time_started) / SECONDS_TO_DAYS).max(0) as u64;
    if elapsed_days >= fundraiser_state.duration as u64 {
        return Err(FundraiserError::FundraiserEnded.into());
    }

    // initialize contributor if not yet created
    {
        let mut contributor_data = contributor_acc.try_borrow_mut()?;
        let contributor_len = contributor_data.len();
        if contributor_len == 0 {
            drop(contributor_data);

            let contributor_bump_bytes = [contributor_bump.to_le()];
            let contributor_seeds = [
                Seed::from(b"contributor"),
                Seed::from(fundraiser_acc.address().as_array()),
                Seed::from(contributor.address().as_array()),
                Seed::from(&contributor_bump_bytes),
            ];
            let contributor_as_signer = Signer::from(&contributor_seeds);

            CreateAccount {
                from: contributor,
                to: contributor_acc,
                lamports: Rent::get()?.try_minimum_balance(Contributor::LEN)?,
                space: Contributor::LEN as u64,
                owner: &crate::ID,
            }
            .invoke_signed(&[contributor_as_signer])?;

            contributor_data = contributor_acc.try_borrow_mut()?;
            let contributor_state = Contributor::load_mut(&mut *contributor_data)?;
            contributor_state.initialize([0; 8], contributor_bump);
        } else if contributor_len != Contributor::LEN {
            return Err(FundraiserError::InvalidContributor.into());
        }
    }

    let mut contributor_data = contributor_acc.try_borrow_mut()?;
    let contributor_state = Contributor::load_mut(&mut *contributor_data)?;
    
    let amount_to_raise = read_u64_le(fundraiser_state.amount_to_raise);
    let max_contribution = amount_to_raise.saturating_mul(MAX_CONTRIBUTION_PERCENTAGE) / PERCENTAGE_SCALER;
    let contributor_amount = read_u64_le(contributor_state.amount);

    if amount > max_contribution {
        return Err(FundraiserError::ContributionTooBig.into());
    }
    if contributor_amount > max_contribution || contributor_amount.saturating_add(amount) > max_contribution {
        return Err(FundraiserError::MaximumContributionsReached.into());
    }

    let new_current = read_u64_le(fundraiser_state.current_amount).saturating_add(amount);
    let new_contributor = contributor_amount.saturating_add(amount);

    write_u64_le(&mut fundraiser_state.current_amount, new_current);
    write_u64_le(&mut contributor_state.amount, new_contributor);

    Ok(())
}