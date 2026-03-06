macro_rules! impl_len {
    ($struct_name:ident) => {
        impl $struct_name {
            pub const LEN: usize = core::mem::size_of::<$struct_name>();
        }
    };
}

macro_rules! impl_load {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn load(data: &[u8]) -> Result<&Self, pinocchio::error::ProgramError> {
                if data.len() != Self::LEN {
                    return Err(pinocchio::error::ProgramError::InvalidAccountData);
                }
                // it is safe to transmute here because we have already checked the length and the struct is `Pod`
                Ok(bytemuck::from_bytes(data))
            }

            pub fn load_mut(data: &mut [u8]) -> Result<&mut Self, pinocchio::error::ProgramError> {
                if data.len() != Self::LEN {
                    return Err(pinocchio::error::ProgramError::InvalidAccountData);
                }
                Ok(bytemuck::from_bytes_mut(data))
            }
        }
    };
}   

pub(crate) use impl_len;
pub(crate) use impl_load;

// Small byte helpers shared across instructions
pub fn read_u64_le(bytes: [u8; 8]) -> u64 {
    u64::from_le_bytes(bytes)
}

pub fn write_u64_le(dst: &mut [u8; 8], value: u64) {
    *dst = value.to_le_bytes();
}

pub fn read_i64_le(bytes: [u8; 8]) -> i64 {
    i64::from_le_bytes(bytes)
}

// Validation helpers reused across instructions
pub fn validate_signer(account: &pinocchio::AccountView) -> pinocchio::ProgramResult {
    if !account.is_signer() {
        return Err(pinocchio::error::ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

pub fn validate_datasize(data: &[u8], expected: usize) -> pinocchio::ProgramResult {
    if data.len() != expected {
        return Err(pinocchio::error::ProgramError::InvalidInstructionData);
    }
    Ok(())
}

pub fn validate_addr(expected: &pinocchio::Address, account: &pinocchio::AccountView, err: pinocchio::error::ProgramError) -> pinocchio::ProgramResult {
    if expected != account.address() {
        return Err(err);
    }
    Ok(())
}

pub fn validate_xeq<T: PartialEq>(lhs: T, rhs: T, err: pinocchio::error::ProgramError) -> pinocchio::ProgramResult {
    if lhs == rhs {
        return Err(err);
    }
    Ok(())
}

pub fn validate_ata(
    ata: &pinocchio::AccountView,
    mint: &pinocchio::AccountView,
    owner: &pinocchio::AccountView,
) -> pinocchio::ProgramResult {
    let ata_state = pinocchio_token::state::TokenAccount::from_account_view(ata)?;
    if ata_state.owner() != owner.address() {
        return Err(pinocchio::error::ProgramError::IllegalOwner);
    }
    if ata_state.mint() != mint.address() {
        return Err(pinocchio::error::ProgramError::InvalidAccountData);
    }
    Ok(())
}
