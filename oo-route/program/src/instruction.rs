//! Instruction types

#![allow(clippy::too_many_arguments)]

use crate::error::SwapError;
use solana_program::program_error::ProgramError;
use std::convert::TryInto;
use std::mem::size_of;

use crate::state::OOSwapStruct;
#[cfg(feature = "fuzz")]
use arbitrary::Arbitrary;
use spl_token_swap::instruction::Swap;

/// Instructions supported by the token swap program.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum OOSwapInstruction {
    ///   CalculateSwapReturn the tokens in the pool.
    OOSwap(OOSwapStruct),
}

impl OOSwapInstruction {
    /// Unpacks a byte buffer into a [SwapInstruction](enum.SwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        Ok(match tag {
            0 => {
                let (&swap_info_len, rest) =
                    rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                if rest.len() % 16 != 0 {
                    //必须是16的整数倍
                    return Err(SwapError::InvalidInstruction.into());
                }
                let size = rest.len() / 16;
                if size as u8 != swap_info_len {
                    //swap info 的长度和amount_in的长度 必须一样
                    return Err(SwapError::InvalidInstruction.into());
                }

                let mut data = vec![];
                for _ in (0..size).into_iter() {
                    let (amount_in, rest) = Self::unpack_u64(rest)?;
                    let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                    data.push(Swap {
                        amount_in,
                        minimum_amount_out,
                    });
                }
                Self::OOSwap(OOSwapStruct {
                    data,
                    swap_info_len,
                })
            }
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (amount, rest) = input.split_at(8);
            let amount = amount
                .get(..8)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(SwapError::InvalidInstruction)?;
            Ok((amount, rest))
        } else {
            Err(SwapError::InvalidInstruction.into())
        }
    }
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
