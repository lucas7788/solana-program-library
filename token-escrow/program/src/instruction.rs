//! Instruction types

#![allow(clippy::too_many_arguments)]

use crate::error::EscrowError;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
};
use std::convert::TryInto;
use std::mem::size_of;

#[cfg(feature = "fuzz")]
use arbitrary::Arbitrary;

/// Initialize instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct Initialize {}

/// DepositAllTokenTypes instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositTokenTypes {
    /// Maximum token A amount to deposit, prevents excessive slippage
    pub token_a_amount: u64,
}

/// WithdrawAllTokenTypes instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct WithdrawTokenTypes {
    /// Minimum amount of token A to receive, prevents excessive slippage
    pub token_amount: u64,
}

/// Instructions supported by the token swap program.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum EscrowInstruction {
    ///   Initializes a new escrow
    ///
    ///   0. `[writable, signer]` New Token-escrow to create.
    ///   1. `[]` swap authority derived from `create_program_address(&[Token-swap account])`
    ///   2. `[]` token_a Account. Must be non zero, owned by swap authority.
    ///   3. `[]` token_b Account. Must be non zero, owned by swap authority.
    ///   4. `[writable]` Pool Token Mint. Must be empty, owned by swap authority.
    ///   5. `[]` Pool Token Account to deposit trading and withdraw fees.
    ///   Must be empty, not owned by swap authority
    ///   6. `[writable]` Pool Token Account to deposit the initial pool token
    ///   supply.  Must be empty, not owned by swap authority.
    ///   7. `[]` Pool Token program id
    Initialize(Initialize),

    ///   Deposit both types of tokens into the pool.  The output is a "pool"
    ///   token representing ownership in the pool. Inputs are converted to
    ///   the current ratio.
    ///
    ///   0. `[]` Token-escrow
    ///   1. `[]` escrow authority
    ///   2. `[]` user transfer authority
    ///   3. `[writable]` token_a user transfer authority can transfer amount,
    ///   4. `[writable]` token_a Base Account to deposit into.
    ///   5. `[]` Token A program id
    DepositTokenTypes(DepositTokenTypes),

    ///   Withdraw both types of tokens from the pool at the current ratio, given
    ///   pool tokens.  The pool tokens are burned in exchange for an equivalent
    ///   amount of token A and B.
    ///
    ///   0. `[]` Token-escrow
    ///   1. `[]` escrow authority
    ///   2. `[]` user transfer authority
    ///   3. `[writable]` SOURCE Pool account, amount is transferable by user transfer authority.
    ///   4. `[writable]` token_a Swap Account to withdraw FROM.
    ///   5. `[writable]` token_a user Account to credit.
    ///   6. `[]` Token A program id
    WithdrawTokenTypes(WithdrawTokenTypes),
}

impl EscrowInstruction {
    /// Unpacks a byte buffer into a [SwapInstruction](enum.SwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(EscrowError::InvalidInstruction)?;
        Ok(match tag {
            0 => Self::Initialize(Initialize {}),
            2 => {
                let (maximum_token_a_amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositTokenTypes(DepositTokenTypes {
                    token_a_amount: maximum_token_a_amount,
                })
            }
            3 => {
                let (minimum_token_a_amount, _rest) = Self::unpack_u64(rest)?;
                Self::WithdrawTokenTypes(WithdrawTokenTypes {
                    token_amount: minimum_token_a_amount,
                })
            }
            _ => return Err(EscrowError::InvalidInstruction.into()),
        })
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (amount, rest) = input.split_at(8);
            let amount = amount
                .get(..8)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(EscrowError::InvalidInstruction)?;
            Ok((amount, rest))
        } else {
            Err(EscrowError::InvalidInstruction.into())
        }
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match &*self {
            Self::Initialize(Initialize {}) => {
                buf.push(0);
            }
            Self::DepositTokenTypes(DepositTokenTypes {
                token_a_amount: maximum_token_a_amount,
            }) => {
                buf.push(2);
                buf.extend_from_slice(&maximum_token_a_amount.to_le_bytes());
            }
            Self::WithdrawTokenTypes(WithdrawTokenTypes {
                token_amount: minimum_token_a_amount,
            }) => {
                buf.push(3);
                buf.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    escrow_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    token_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let init_data = EscrowInstruction::Initialize(Initialize {});
    let data = init_data.pack();

    let accounts = vec![
        AccountMeta::new(*escrow_pubkey, true),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*token_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'deposit_all_token_types' instruction.
pub fn deposit_token_types(
    program_id: &Pubkey,
    token_a_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    user_transfer_authority_pubkey: &Pubkey,
    deposit_token_a_pubkey: &Pubkey,
    swap_token_a_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
    instruction: DepositTokenTypes,
) -> Result<Instruction, ProgramError> {
    let data = EscrowInstruction::DepositTokenTypes(instruction).pack();

    let accounts = vec![
        AccountMeta::new_readonly(*swap_pubkey, false),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*user_transfer_authority_pubkey, true),
        AccountMeta::new(*deposit_token_a_pubkey, false),
        AccountMeta::new(*swap_token_a_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new_readonly(*token_a_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'withdraw_all_token_types' instruction.
pub fn withdraw_token_types(
    program_id: &Pubkey,
    token_a_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    user_transfer_authority_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    swap_token_a_pubkey: &Pubkey,
    destination_token_a_pubkey: &Pubkey,
    instruction: WithdrawTokenTypes,
) -> Result<Instruction, ProgramError> {
    let data = EscrowInstruction::WithdrawTokenTypes(instruction).pack();

    let accounts = vec![
        AccountMeta::new_readonly(*swap_pubkey, false),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*user_transfer_authority_pubkey, true),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*swap_token_a_pubkey, false),
        AccountMeta::new(*destination_token_a_pubkey, false),
        AccountMeta::new_readonly(*token_a_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Unpacks a reference from a bytes buffer.
/// TODO actually pack / unpack instead of relying on normal memory layout.
pub fn unpack<T>(input: &[u8]) -> Result<&T, ProgramError> {
    if input.len() < size_of::<u8>() + size_of::<T>() {
        return Err(ProgramError::InvalidAccountData);
    }
    #[allow(clippy::cast_ptr_alignment)]
    let val: &T = unsafe { &*(&input[1] as *const u8 as *const T) };
    Ok(val)
}
