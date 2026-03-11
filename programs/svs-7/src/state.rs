//! Program state.

use anchor_lang::prelude::*;

use std::str::FromStr;

use crate::constants::WSOL_MINT;
use crate::error::VaultError;

/// Balance model for SVS-7.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum BalanceModel {
    /// Reads `wsol_vault.amount` each instruction.
    Live,
    /// Caches `total_assets` in the vault account, updated via sync() or operations.
    Stored,
}

#[account]
pub struct SolVault {
    pub authority: Pubkey,
    pub shares_mint: Pubkey,

    /// PDA-owned wSOL token account (SPL Token program).
    pub wsol_vault: Pubkey,

    /// Cached assets in lamports. Used only when `balance_model == Stored`.
    pub total_assets: u64,

    /// Decimal offset exponent used by share conversion math.
    /// For SOL (9 decimals), this should be 0.
    pub decimals_offset: u8,

    pub balance_model: BalanceModel,
    pub bump: u8,
    pub paused: bool,
    pub vault_id: u64,
    pub _reserved: [u8; 64],
}

impl SolVault {
    /// Anchor account size.
    pub const LEN: usize = 8 + 32 + 32 + 32 + 8 + 1 + 1 + 1 + 1 + 8 + 64;
}

/// Validate the configured wSOL mint.
///
/// We treat the canonical wrapped SOL mint (`So111...`) as the only supported asset mint.
pub fn validate_wsol_mint(mint: &Pubkey) -> Result<()> {
    let expected = Pubkey::from_str(WSOL_MINT).map_err(|_| VaultError::InvalidWsolMint)?;
    require_keys_eq!(*mint, expected, VaultError::InvalidWsolMint);
    Ok(())
}

// =============================================================================
// Optional Module State Accounts
// =============================================================================

#[cfg(feature = "modules")]
pub mod module_state {
    use super::*;

    // Re-export seeds from svs-module-hooks (shared across programs)
    pub use svs_module_hooks::state::{
        ACCESS_CONFIG_SEED, CAP_CONFIG_SEED, FEE_CONFIG_SEED, LOCK_CONFIG_SEED, SHARE_LOCK_SEED,
        USER_DEPOSIT_SEED,
    };

    #[account]
    pub struct FeeConfig {
        pub vault: Pubkey,
        pub fee_recipient: Pubkey,
        pub entry_fee_bps: u16,
        pub exit_fee_bps: u16,
        pub management_fee_bps: u16,
        pub performance_fee_bps: u16,
        pub high_water_mark: u64,
        pub last_fee_collection: i64,
        pub bump: u8,
    }

    impl FeeConfig {
        pub const LEN: usize = 8 + 32 + 32 + 2 + 2 + 2 + 2 + 8 + 8 + 1;
    }

    #[account]
    pub struct CapConfig {
        pub vault: Pubkey,
        pub global_cap: u64,
        pub per_user_cap: u64,
        pub bump: u8,
    }

    impl CapConfig {
        pub const LEN: usize = 8 + 32 + 8 + 8 + 1;
    }

    #[account]
    pub struct UserDeposit {
        pub vault: Pubkey,
        pub user: Pubkey,
        pub cumulative_assets: u64,
        pub bump: u8,
    }

    impl UserDeposit {
        pub const LEN: usize = 8 + 32 + 32 + 8 + 1;
    }

    #[account]
    pub struct LockConfig {
        pub vault: Pubkey,
        pub lock_duration: i64,
        pub bump: u8,
    }

    impl LockConfig {
        pub const LEN: usize = 8 + 32 + 8 + 1;
    }

    #[account]
    pub struct ShareLock {
        pub vault: Pubkey,
        pub owner: Pubkey,
        pub locked_until: i64,
        pub bump: u8,
    }

    impl ShareLock {
        pub const LEN: usize = 8 + 32 + 32 + 8 + 1;
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum AccessMode {
        Open,
        Whitelist,
        Blacklist,
    }

    #[account]
    pub struct AccessConfig {
        pub vault: Pubkey,
        pub mode: AccessMode,
        pub merkle_root: [u8; 32],
        pub bump: u8,
    }

    impl AccessConfig {
        pub const LEN: usize = 8 + 32 + 1 + 32 + 1;
    }
}

#[cfg(feature = "modules")]
pub use module_state::*;
