//! Mock Txoracle for VAR devnet integration testing ONLY.
//!
//! Stands in for Tx LINE's `Txoracle::validate_stat`, which returns a borsh `bool`. This mock
//! attests EVERY stat as valid (returns `true`) so the full VAR settlement flow
//! (create -> deposit -> resolve -> claim -> reverify) can be exercised on devnet without the live
//! Tx LINE Merkle feed. NEVER deploy this to mainnet or point a real market at it.

use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, program::set_return_data,
    pubkey::Pubkey,
};

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    // Borsh-encoded `true` is a single byte 0x01. VAR reads this via sol_get_return_data.
    set_return_data(&[1u8]);
    Ok(())
}
