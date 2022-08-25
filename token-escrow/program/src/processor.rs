//! Program state processor

use crate::{
    error::EscrowError,
    instruction::{DepositTokenTypes, EscrowInstruction, Initialize, WithdrawTokenTypes},
    state::{EscrowState, EscrowV1, EscrowVersion},
};
use num_traits::FromPrimitive;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    instruction::Instruction,
    msg,
    program::invoke_signed,
    program_error::{PrintProgramError, ProgramError},
    program_option::COption,
    pubkey::Pubkey,
};
use spl_token_2022::{
    check_spl_token_program_account,
    error::TokenError,
    extension::StateWithExtensions,
    state::{Account, Mint},
};
use std::{convert::TryInto, error::Error};

/// Program state handler.
pub struct Processor {}
impl Processor {
    /// Unpacks a spl_token `Account`.
    pub fn unpack_token_account(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<Account, EscrowError> {
        if account_info.owner != token_program_id
            && check_spl_token_program_account(account_info.owner).is_err()
        {
            Err(EscrowError::IncorrectTokenProgramId)
        } else {
            StateWithExtensions::<Account>::unpack(&account_info.data.borrow())
                .map(|a| a.base)
                .map_err(|_| EscrowError::ExpectedAccount)
        }
    }

    /// Unpacks a spl_token `Mint`.
    pub fn unpack_mint(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<Mint, EscrowError> {
        if account_info.owner != token_program_id
            && check_spl_token_program_account(account_info.owner).is_err()
        {
            Err(EscrowError::IncorrectTokenProgramId)
        } else {
            StateWithExtensions::<Mint>::unpack(&account_info.data.borrow())
                .map(|m| m.base)
                .map_err(|_| EscrowError::ExpectedMint)
        }
    }

    /// Calculates the authority id by generating a program address.
    pub fn authority_id(
        program_id: &Pubkey,
        my_info: &Pubkey,
        bump_seed: u8,
    ) -> Result<Pubkey, EscrowError> {
        Pubkey::create_program_address(&[&my_info.to_bytes()[..32], &[bump_seed]], program_id)
            .or(Err(EscrowError::InvalidProgramAddress))
    }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer<'a>(
        escrow: &Pubkey,
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        bump_seed: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let escrow_bytes = escrow.to_bytes();
        let authority_signature_seeds = [&escrow_bytes[..32], &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];
        #[allow(deprecated)]
        let ix = spl_token_2022::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;
        invoke_signed_wrapper::<TokenError>(
            &ix,
            &[source, destination, authority, token_program],
            signers,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn check_accounts(
        escrow: &dyn EscrowState,
        program_id: &Pubkey,
        escrow_account_info: &AccountInfo,
        authority_info: &AccountInfo,
        token_a_info: &AccountInfo,
        user_token_a_info: Option<&AccountInfo>,
    ) -> ProgramResult {
        if escrow_account_info.owner != program_id {
            return Err(ProgramError::IncorrectProgramId);
        }
        if *authority_info.key
            != Self::authority_id(program_id, escrow_account_info.key, escrow.bump_seed())?
        {
            return Err(EscrowError::InvalidProgramAddress.into());
        }
        if *token_a_info.key != *escrow.token_account() {
            return Err(EscrowError::IncorrectSwapAccount.into());
        }
        if let Some(user_token_a_info) = user_token_a_info {
            if token_a_info.key == user_token_a_info.key {
                return Err(EscrowError::InvalidInput.into());
            }
        }
        Ok(())
    }

    /// Processes an [Initialize](enum.Instruction.html).
    pub fn process_initialize(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        // token-escrow authority account
        let authority_info = next_account_info(account_info_iter)?;
        // owned by token-escrow authority account
        let token_info = next_account_info(account_info_iter)?;

        if EscrowVersion::is_initialized(&escrow_info.data.borrow()) {
            return Err(EscrowError::AlreadyInUse.into());
        }

        let (escrow_authority, bump_seed) =
            Pubkey::find_program_address(&[&escrow_info.key.to_bytes()], program_id);
        if *authority_info.key != escrow_authority {
            return Err(EscrowError::InvalidProgramAddress.into());
        }

        let obj = EscrowVersion::EscrowV1(EscrowV1 {
            is_initialized: true,
            bump_seed,
            token: *token_info.key,
            token_mint: *token_info.key,// 可能也不需要
        });
        EscrowVersion::pack(obj, &mut escrow_info.data.borrow_mut())?;
        Ok(())
    }

    /// Processes an [DepositAllTokenTypes](enum.Instruction.html).
    pub fn process_deposit_token_types(
        program_id: &Pubkey,
        token_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let user_transfer_authority_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let token_info = next_account_info(account_info_iter)?;
        let dest_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_escrow = EscrowVersion::unpack(&escrow_info.data.borrow())?;
        Self::check_accounts(
            token_escrow.as_ref(),
            program_id,
            escrow_info,
            authority_info,
            token_info,
            Some(source_info),
        )?;

        Self::token_transfer(
            escrow_info.key,
            token_program_info.clone(),
            source_info.clone(),
            token_info.clone(),
            user_transfer_authority_info.clone(),
            token_escrow.bump_seed(),
            token_amount,
        )?;
        Ok(())
    }

    /// Processes an [WithdrawAllTokenTypes](enum.Instruction.html).
    pub fn process_withdraw_token_types(
        program_id: &Pubkey,
        token_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let user_transfer_authority_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let token_info = next_account_info(account_info_iter)?;
        let dest_token_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_escrow = EscrowVersion::unpack(&escrow_info.data.borrow())?;
        Self::check_accounts(
            token_escrow.as_ref(),
            program_id,
            escrow_info,
            authority_info,
            token_info,
            Some(dest_token_info),
        )?;

        if token_amount > 0 {
            Self::token_transfer(
                escrow_info.key,
                token_program_info.clone(),
                token_info.clone(),
                dest_token_info.clone(),
                authority_info.clone(),
                token_escrow.bump_seed(),
                token_amount,
            )?;
        }
        Ok(())
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        Self::process_with_constraints(program_id, accounts, input)
    }

    /// Processes an instruction given extra constraint
    pub fn process_with_constraints(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        input: &[u8],
    ) -> ProgramResult {
        let instruction = EscrowInstruction::unpack(input)?;
        match instruction {
            EscrowInstruction::Initialize(Initialize {}) => {
                msg!("Instruction: Init");
                Self::process_initialize(program_id, accounts)
            }
            EscrowInstruction::DepositTokenTypes(DepositTokenTypes {
                token_a_amount: maximum_token_a_amount,
            }) => {
                msg!("Instruction: DepositAllTokenTypes");
                Self::process_deposit_token_types(program_id, maximum_token_a_amount, accounts)
            }
            EscrowInstruction::WithdrawTokenTypes(WithdrawTokenTypes {
                token_amount: minimum_token_a_amount,
            }) => {
                msg!("Instruction: WithdrawAllTokenTypes");
                Self::process_withdraw_token_types(program_id, minimum_token_a_amount, accounts)
            }
        }
    }
}

fn to_u128(val: u64) -> Result<u128, EscrowError> {
    val.try_into().map_err(|_| EscrowError::ConversionFailure)
}

fn to_u64(val: u128) -> Result<u64, EscrowError> {
    val.try_into().map_err(|_| EscrowError::ConversionFailure)
}

fn invoke_signed_wrapper<T>(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    signers_seeds: &[&[&[u8]]],
) -> Result<(), ProgramError>
where
    T: 'static + PrintProgramError + DecodeError<T> + FromPrimitive + Error,
{
    invoke_signed(instruction, account_infos, signers_seeds).map_err(|err| {
        err.print::<T>();
        err
    })
}

#[cfg(test)]
mod tests {}
