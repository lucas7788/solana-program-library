//! State transition types

use crate::error::EscrowError;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use enum_dispatch::enum_dispatch;
use solana_program::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};
use spl_token_2022::{
    extension::StateWithExtensions,
    state::{Account, AccountState},
};
use std::sync::Arc;

/// Trait representing access to program state across all versions
#[enum_dispatch]
pub trait EscrowState {
    /// Is the escrow initialized, with data written to it
    fn is_initialized(&self) -> bool;
    /// Bump seed used to generate the program address / authority
    fn bump_seed(&self) -> u8;
    /// Token program ID associated with the swap
    // fn token_program_id(&self) -> &Pubkey;
    /// Address of token A liquidity account
    fn token_account(&self) -> &Pubkey;

    /// Address of token A mint
    fn token_mint(&self) -> &Pubkey;
}

/// All versions of SwapState
#[enum_dispatch(EscrowState)]
pub enum EscrowVersion {
    /// Latest version, used for all new swaps
    EscrowV1,
}

/// SwapVersion does not implement program_pack::Pack because there are size
/// checks on pack and unpack that would break backwards compatibility, so
/// special implementations are provided here
impl EscrowVersion {
    /// Size of the latest version of the SwapState
    pub const LATEST_LEN: usize = 1 + EscrowV1::LEN; // add one for the version enum

    /// Pack a swap into a byte array, based on its version
    pub fn pack(src: Self, dst: &mut [u8]) -> Result<(), ProgramError> {
        match src {
            Self::EscrowV1(swap_info) => {
                dst[0] = 1;
                EscrowV1::pack(swap_info, &mut dst[1..])
            }
        }
    }

    /// Unpack the swap account based on its version, returning the result as a
    /// SwapState trait object
    pub fn unpack(input: &[u8]) -> Result<Arc<dyn EscrowState>, ProgramError> {
        let (&version, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidAccountData)?;
        match version {
            1 => Ok(Arc::new(EscrowV1::unpack(rest)?)),
            _ => Err(ProgramError::UninitializedAccount),
        }
    }

    /// Special check to be done before any instruction processing, works for
    /// all versions
    pub fn is_initialized(input: &[u8]) -> bool {
        match Self::unpack(input) {
            Ok(swap) => swap.is_initialized(),
            Err(_) => false,
        }
    }
}

/// Program states.
#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct EscrowV1 {
    /// Initialized state.
    pub is_initialized: bool,
    /// Bump seed used in program address.
    /// The program address is created deterministically with the bump seed,
    /// swap program id, and swap account pubkey.  This program address has
    /// authority over the swap's token A account, token B account, and pool
    /// token mint.
    pub bump_seed: u8,

    /// Program ID of the tokens being exchanged.
    // pub token_program_id: Pubkey,

    /// Token A
    pub token_a: Pubkey,

    /// Mint information for token A
    pub token_a_mint: Pubkey,
}

impl EscrowState for EscrowV1 {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    fn bump_seed(&self) -> u8 {
        self.bump_seed
    }

    // fn token_program_id(&self) -> &Pubkey {
    //     &self.token_program_id
    // }

    fn token_account(&self) -> &Pubkey {
        &self.token_a
    }

    fn token_mint(&self) -> &Pubkey {
        &self.token_a_mint
    }
}

impl Sealed for EscrowV1 {}
impl IsInitialized for EscrowV1 {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for EscrowV1 {
    const LEN: usize = 98;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 98];
        let (is_initialized, bump_seed, token_program_id, token_a, token_a_mint) =
            mut_array_refs![output, 1, 1, 32, 32, 32];
        is_initialized[0] = self.is_initialized as u8;
        bump_seed[0] = self.bump_seed;
        token_a.copy_from_slice(self.token_a.as_ref());
        token_a_mint.copy_from_slice(self.token_a_mint.as_ref());
    }

    /// Unpacks a byte buffer into a [SwapV1](struct.SwapV1.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 98];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, bump_seed, token_program_id, token_a, token_a_mint) =
            array_refs![input, 1, 1, 32, 32, 32];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            bump_seed: bump_seed[0],
            token_a: Pubkey::new_from_array(*token_a),
            token_a_mint: Pubkey::new_from_array(*token_a_mint),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    const TEST_BUMP_SEED: u8 = 255;
    const TEST_TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array([1u8; 32]);
    const TEST_TOKEN_A: Pubkey = Pubkey::new_from_array([2u8; 32]);
    const TEST_TOKEN_A_MINT: Pubkey = Pubkey::new_from_array([5u8; 32]);

    const TEST_AMP: u64 = 1;

    #[test]
    fn swap_version_pack() {
        let swap_info = EscrowVersion::EscrowV1(EscrowV1 {
            is_initialized: true,
            bump_seed: TEST_BUMP_SEED,
            token_a: TEST_TOKEN_A,
            token_a_mint: TEST_TOKEN_A_MINT,
        });

        let mut packed = [0u8; EscrowVersion::LATEST_LEN];
        EscrowVersion::pack(swap_info, &mut packed).unwrap();
        let unpacked = EscrowVersion::unpack(&packed).unwrap();

        assert!(unpacked.is_initialized());
        assert_eq!(unpacked.bump_seed(), TEST_BUMP_SEED);
        assert_eq!(*unpacked.token_program_id(), TEST_TOKEN_PROGRAM_ID);
        assert_eq!(*unpacked.token_account(), TEST_TOKEN_A);
        assert_eq!(*unpacked.token_mint(), TEST_TOKEN_A_MINT);
    }

    #[test]
    fn swap_v1_pack() {
        let curve_type = TEST_CURVE_TYPE.try_into().unwrap();
        let calculator = Arc::new(TEST_CURVE);
        let swap_curve = SwapCurve {
            curve_type,
            calculator,
        };
        let swap_info = EscrowV1 {
            is_initialized: true,
            bump_seed: TEST_BUMP_SEED,
            token_a: TEST_TOKEN_A,
            token_a_mint: TEST_TOKEN_A_MINT,
        };

        let mut packed = [0u8; EscrowV1::LEN];
        EscrowV1::pack_into_slice(&swap_info, &mut packed);
        let unpacked = EscrowV1::unpack(&packed).unwrap();
        assert_eq!(swap_info, unpacked);

        let mut packed = vec![1u8, TEST_BUMP_SEED];
        packed.extend_from_slice(&TEST_TOKEN_PROGRAM_ID.to_bytes());
        packed.extend_from_slice(&TEST_TOKEN_A.to_bytes());
        packed.extend_from_slice(&TEST_TOKEN_A_MINT.to_bytes());
        packed.extend_from_slice(&TEST_AMP.to_le_bytes());
        packed.extend_from_slice(&[0u8; 24]);
        let unpacked = EscrowV1::unpack(&packed).unwrap();
        assert_eq!(swap_info, unpacked);

        let packed = [0u8; EscrowV1::LEN];
        let swap_info: EscrowV1 = Default::default();
        let unpack_unchecked = EscrowV1::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, swap_info);
        let err = EscrowV1::unpack(&packed).unwrap_err();
        assert_eq!(err, ProgramError::UninitializedAccount);
    }
}
