//! Program state processor

use crate::instruction::OOSwapInstruction;

use spl_token_swap::instruction::{swap, Swap};

use crate::state::OOSwapStruct;

use crate::{error::SwapError, state::SwapVersion};
use num_traits::FromPrimitive;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::{PrintProgramError, ProgramError},
    program_pack::Pack,
    pubkey::Pubkey,
};

/// Program state handler.
pub struct Processor {}
impl Processor {
    /// Unpacks a spl_token `Account`.
    pub fn unpack_token_account(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<spl_token::state::Account, SwapError> {
        if account_info.owner != token_program_id {
            Err(SwapError::IncorrectTokenProgramId)
        } else {
            spl_token::state::Account::unpack(&account_info.data.borrow())
                .map_err(|_| SwapError::ExpectedAccount)
        }
    }
    /// Calculates the authority id by generating a program address.
    pub fn authority_id(
        program_id: &Pubkey,
        my_info: &Pubkey,
        nonce: u8,
    ) -> Result<Pubkey, SwapError> {
        Pubkey::create_program_address(&[&my_info.to_bytes()[..32], &[nonce]], program_id)
            .or(Err(SwapError::InvalidProgramAddress))
    }
    /// process_swap
    pub fn process_swap(
        program_id: &Pubkey,
        data: Vec<Swap>,
        swap_info_len: u8,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        // 用户相关的account info
        let user_transfer_authority_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?;

        //获取 swap info相关的信息
        for i in (0..swap_info_len).into_iter() {
            let swap_info = next_account_info(account_info_iter)?;
            let authority_info = next_account_info(account_info_iter)?;
            let swap_source_info = next_account_info(account_info_iter)?;
            let swap_destination_info = next_account_info(account_info_iter)?;
            let pool_mint_info = next_account_info(account_info_iter)?;
            let pool_fee_account_info = next_account_info(account_info_iter)?;
            let token_program_info = next_account_info(account_info_iter)?;

            if swap_info.owner != program_id {
                return Err(ProgramError::IncorrectProgramId);
            }
            let token_swap = SwapVersion::unpack(&swap_info.data.borrow())?;
            if *authority_info.key
                != Self::authority_id(program_id, swap_info.key, token_swap.nonce())?
            {
                return Err(SwapError::InvalidProgramAddress.into());
            }
            if !(*swap_source_info.key == *token_swap.token_a_account()
                || *swap_source_info.key == *token_swap.token_b_account())
            {
                return Err(SwapError::IncorrectSwapAccount.into());
            }
            if !(*swap_destination_info.key == *token_swap.token_a_account()
                || *swap_destination_info.key == *token_swap.token_b_account())
            {
                return Err(SwapError::IncorrectSwapAccount.into());
            }
            if *swap_source_info.key == *swap_destination_info.key {
                return Err(SwapError::InvalidInput.into());
            }
            if swap_source_info.key == source_info.key {
                return Err(SwapError::InvalidInput.into());
            }
            if *pool_mint_info.key != *token_swap.pool_mint() {
                return Err(SwapError::IncorrectPoolMint.into());
            }
            if *pool_fee_account_info.key != *token_swap.pool_fee_account() {
                return Err(SwapError::IncorrectFeeAccount.into());
            }
            if *token_program_info.key != *token_swap.token_program_id() {
                return Err(SwapError::IncorrectTokenProgramId.into());
            }

            let swap_bytes = swap_info.key.to_bytes();
            let nonce = token_swap.nonce();
            let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
            let signers = &[&authority_signature_seeds[..]];

            let ix = swap(
                program_id, //TODO 这个 是不是应该修改成 调用的合约的地址
                token_program_info.key,
                swap_info.key,
                authority_info.key,
                user_transfer_authority_info.key,
                source_info.key,
                swap_source_info.key,
                swap_destination_info.key,
                destination_info.key,
                pool_mint_info.key,
                pool_fee_account_info.key,
                None,
                data[i as usize].clone(),
            )?;
            let res = invoke_signed(
                &ix,
                &[
                    swap_info.clone(),
                    authority_info.clone(),
                    user_transfer_authority_info.clone(),
                    source_info.clone(),
                    swap_source_info.clone(),
                    swap_destination_info.clone(),
                    destination_info.clone(),
                    pool_mint_info.clone(),
                    pool_fee_account_info.clone(),
                    token_program_info.clone(),
                ],
                signers,
            );
            if res.is_err() {
                return res;
            }
        }
        return Ok(());
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
        let instruction = OOSwapInstruction::unpack(input)?;
        match instruction {
            OOSwapInstruction::OOSwap(OOSwapStruct {
                data,
                swap_info_len,
            }) => {
                msg!("Instruction: OOSwap");
                Self::process_swap(program_id, data, swap_info_len, accounts)
            }
        }
    }
}

impl PrintProgramError for SwapError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            SwapError::ExpectedAccount => {
                msg!("Error: Deserialized account is not an SPL Token account")
            }
            SwapError::IncorrectFeeAccount => {
                msg!("Error: Pool fee token account incorrect")
            }
            SwapError::IncorrectPoolMint => {
                msg!("Error: Address of the provided pool token mint is incorrect")
            }
            SwapError::InvalidProgramAddress => {
                msg!("Error: Invalid program address generated from nonce and key")
            }
            SwapError::InvalidInput => msg!("Error: InvalidInput"),
            SwapError::IncorrectSwapAccount => {
                msg!("Error: Address of the provided swap token account is incorrect")
            }
            SwapError::EmptySupply => msg!("Error: Input token account empty"),
            SwapError::InvalidInstruction => msg!("Error: InvalidInstruction"),
            SwapError::ZeroTradingTokens => {
                msg!("Error: Given pool token amount results in zero trading tokens")
            }
            SwapError::ConversionFailure => msg!("Error: Conversion to or from u64 failed."),
            SwapError::InvalidFee => {
                msg!("Error: The provided fee does not match the program owner's constraints")
            }
            SwapError::IncorrectTokenProgramId => {
                msg!("Error: The provided token program does not match the token program expected by the swap")
            }
            SwapError::FeeCalculationFailure => msg!(
                "Error: The fee calculation failed due to overflow, underflow, or unexpected 0"
            ),
            SwapError::UnsupportedCurveType => {
                msg!("Error: The provided curve type is not supported by the program owner")
            }
            SwapError::InvalidCurve => {
                msg!("Error: The provided curve parameters are invalid")
            }
            SwapError::UnsupportedCurveOperation => {
                msg!("Error: The operation cannot be performed on the given curve")
            }
        }
    }
}
