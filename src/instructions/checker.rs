use pinocchio::{
    AccountView, Address, ProgramResult, cpi::{Seed, Signer}, error::ProgramError
};
use pinocchio_pubkey::derive_address;
use pinocchio_token::{instructions::TransferChecked, state::{Mint, TokenAccount}};

use crate::{FundraiserError, state::Fundraiser, utils::{read_u64_le, validate_signer, validate_datasize, validate_addr, validate_ata}};

pub fn process_check_contributions_instruction(accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    let [maker, mint_to_raise, fundraiser_acc, maker_ata, vault, _token_program, _system_program, _remain @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    validate_signer(maker)?;
    validate_datasize(data, 0)?;

    let (fundraiser_maker, fundraiser_mint, fundraiser_bump, fundraiser_target_bytes) = {
        let fundraiser_data = fundraiser_acc.try_borrow()?;
        if fundraiser_data.len() != Fundraiser::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        let fundraiser_state = Fundraiser::load(&fundraiser_data)?;
        (
            fundraiser_state.maker,
            fundraiser_state.mint_to_raise,
            fundraiser_state.bump,
            fundraiser_state.amount_to_raise,
        )
    };

    // derive and check fundraiser PDA
    let seed = [b"fundraiser".as_ref(), fundraiser_maker.as_ref(), &[fundraiser_bump]];
    let expected_fundraiser_addr = Address::from(derive_address(&seed, None, &crate::ID.to_bytes()));
    validate_addr(&expected_fundraiser_addr, fundraiser_acc, FundraiserError::InvalidFundraiser.into())?;
    validate_addr(&Address::from(fundraiser_mint), mint_to_raise, FundraiserError::InvalidFundraiser.into())?;

    // validate ATAs
    validate_ata(vault, mint_to_raise, fundraiser_acc)?;
    validate_ata(maker_ata, mint_to_raise, maker)?;

    // load token state without holding borrows across the transfer
    let (vault_amount, decimals) = {
        let vault_state = TokenAccount::from_account_view(vault)?;
        let mint_state = Mint::from_account_view(mint_to_raise)?;
        (vault_state.amount(), mint_state.decimals())
    };

    // ensure target met
    let target = read_u64_le(fundraiser_target_bytes);
    if vault_amount < target {
        return Err(FundraiserError::TargetNotMet.into());
    }

    // transfer funds to maker (fundraiser PDA as signer)
    let fundraiser_bump = [fundraiser_bump];
    let signer_seeds = [
        Seed::from(b"fundraiser"),
        Seed::from(fundraiser_maker.as_ref()),
        Seed::from(&fundraiser_bump),
    ];
    let fundraiser_signer = Signer::from(&signer_seeds);

    TransferChecked {
        from: vault,
        to: maker_ata,
        authority: fundraiser_acc,
        mint: mint_to_raise,
        amount: vault_amount,
        decimals,
    }
    .invoke_signed(&[fundraiser_signer])?;

    Ok(())
}