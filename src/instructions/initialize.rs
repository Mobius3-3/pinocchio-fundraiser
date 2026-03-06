use pinocchio::{
    AccountView, Address, ProgramResult, cpi::{Seed, Signer}, error::ProgramError, sysvars::{Sysvar, rent::Rent, clock::Clock}
};
use pinocchio_pubkey::derive_address;
use pinocchio_system::instructions::CreateAccount;
use bytemuck::{Pod, Zeroable};
use crate::state::Fundraiser;
use crate::constants::MIN_AMOUNT_TO_RAISE;

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct InitData {
    amount: [u8; 8],
    duration: u8,
    bump: u8
}

impl InitData {
    pub const LEN: usize = core::mem::size_of::<InitData>();
}

pub fn process_initialize_instruction(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [maker, mint_to_raise, fundraiser, _system_program, _token_program, _remain @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // validate accounts and instruction data
    let (amount_to_raise, duration, bump) = {
        if !maker.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        };

        if data.len() != InitData::LEN {
            return Err(ProgramError::InvalidInstructionData);
        };

        let ix_data = bytemuck::from_bytes::<InitData>(data);
        let amount_to_raise = u64::from_le_bytes(ix_data.amount);
        if amount_to_raise < MIN_AMOUNT_TO_RAISE {
            return Err(ProgramError::InvalidInstructionData);
        }

        let bump = ix_data.bump;
        let seed = [b"fundraiser".as_ref(), maker.address().as_ref(), &[bump]];
        let expected_fundraiser_addr = Address::from(derive_address(&seed, None, &crate::ID.to_bytes()));
        if &expected_fundraiser_addr != fundraiser.address() {
            return Err(ProgramError::InvalidAccountData);
        }

        (amount_to_raise, ix_data.duration, bump)
    };

    // init the fundraiser account
    let fundraiser_bump = [bump.to_le()];
    let fundraiser_seeds = [Seed::from(b"fundraiser"), Seed::from(maker.address().as_array()), Seed::from(&fundraiser_bump)];
    let fundraiser_as_signer = Signer::from(&fundraiser_seeds);

    CreateAccount {
        from: maker,
        to: fundraiser,
        lamports: Rent::get()?.try_minimum_balance(Fundraiser::LEN)?,
        space: Fundraiser::LEN as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[fundraiser_as_signer.clone()])?;

    let mut fundraiser_data = fundraiser.try_borrow_mut()?; // locked when writing to data
    let fundraiser = Fundraiser::load_mut(&mut *fundraiser_data)?;
    // guard against zero timestamps (LiteSVM clock often returns 0
    let now = Clock::get()?.unix_timestamp.max(1);

    fundraiser.initialize(
        *maker.address().as_array(),
        *mint_to_raise.address().as_array(),
        amount_to_raise.to_le_bytes(),
        [0; 8],
        now.to_le_bytes(),
        duration,
        fundraiser_bump[0],
    );

    Ok(())
}