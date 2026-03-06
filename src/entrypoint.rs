use pinocchio::{
    address::declare_id, default_panic_handler, error::ProgramError, no_allocator,
    program_entrypoint, AccountView, Address, ProgramResult,
};

use crate::instructions::*;

program_entrypoint!(process_instruction);
no_allocator!();
default_panic_handler!();

declare_id!("4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT");

fn process_instruction(
    program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    if program_id != &ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    match instruction_data.split_first() {
        Some((0, ix_data)) => process_initialize_instruction(accounts, ix_data),
        Some((1, ix_data)) => process_check_contributions_instruction(accounts, ix_data),
        Some((2, ix_data)) => process_contribute_instruction(accounts, ix_data),
        Some((3, ix_data)) => process_refund_instruction(accounts, ix_data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
