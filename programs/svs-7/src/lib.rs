//! SVS-7: Native SOL Vault (wraps internally)
//!
//! SVS-7 accepts native SOL (lamports) and internally wraps/un-wraps via the
//! canonical wSOL SPL Token mint. Shares are Token-2022 tokens.

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

// NOTE: placeholder program id. Replace with the deployed svs_7 program id.
declare_id!("11111111111111111111111111111111");

#[program]
pub mod svs_7 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, vault_id: u64, balance_model: state::BalanceModel) -> Result<()> {
        instructions::initialize::handler(ctx, vault_id, balance_model)
    }

    pub fn deposit_sol(ctx: Context<DepositSol>, assets: u64, min_shares_out: u64) -> Result<()> {
        instructions::deposit_sol::handler(ctx, assets, min_shares_out)
    }

    pub fn deposit_wsol(ctx: Context<DepositWsol>, assets: u64, min_shares_out: u64) -> Result<()> {
        instructions::deposit_wsol::handler(ctx, assets, min_shares_out)
    }

    pub fn mint_sol(ctx: Context<MintSol>, shares: u64, max_assets_in: u64) -> Result<()> {
        instructions::mint_sol::handler(ctx, shares, max_assets_in)
    }

    pub fn withdraw_sol(ctx: Context<WithdrawSol>, assets: u64, max_shares_in: u64) -> Result<()> {
        instructions::withdraw_sol::handler(ctx, assets, max_shares_in)
    }

    pub fn withdraw_wsol(ctx: Context<WithdrawWsol>, assets: u64, max_shares_in: u64) -> Result<()> {
        instructions::withdraw_wsol::handler(ctx, assets, max_shares_in)
    }

    pub fn redeem_sol(ctx: Context<RedeemSol>, shares: u64, min_assets_out: u64) -> Result<()> {
        instructions::redeem_sol::handler(ctx, shares, min_assets_out)
    }

    pub fn redeem_wsol(ctx: Context<RedeemWsol>, shares: u64, min_assets_out: u64) -> Result<()> {
        instructions::redeem_wsol::handler(ctx, shares, min_assets_out)
    }

    pub fn sync(ctx: Context<SyncVault>) -> Result<()> {
        instructions::sync::handler(ctx)
    }

    // ============ Admin ============

    pub fn pause(ctx: Context<Admin>) -> Result<()> {
        instructions::admin::pause(ctx)
    }

    pub fn unpause(ctx: Context<Admin>) -> Result<()> {
        instructions::admin::unpause(ctx)
    }

    pub fn transfer_authority(ctx: Context<Admin>, new_authority: Pubkey) -> Result<()> {
        instructions::admin::transfer_authority(ctx, new_authority)
    }

    // ============ View Functions (CPI composable) ============

    pub fn preview_deposit(ctx: Context<VaultView>, assets: u64) -> Result<()> {
        instructions::view::preview_deposit(ctx, assets)
    }

    pub fn preview_mint(ctx: Context<VaultView>, shares: u64) -> Result<()> {
        instructions::view::preview_mint(ctx, shares)
    }

    pub fn preview_withdraw(ctx: Context<VaultView>, assets: u64) -> Result<()> {
        instructions::view::preview_withdraw(ctx, assets)
    }

    pub fn preview_redeem(ctx: Context<VaultView>, shares: u64) -> Result<()> {
        instructions::view::preview_redeem(ctx, shares)
    }

    pub fn convert_to_shares(ctx: Context<VaultView>, assets: u64) -> Result<()> {
        instructions::view::convert_to_shares_view(ctx, assets)
    }

    pub fn convert_to_assets(ctx: Context<VaultView>, shares: u64) -> Result<()> {
        instructions::view::convert_to_assets_view(ctx, shares)
    }

    pub fn total_assets(ctx: Context<VaultView>) -> Result<()> {
        instructions::view::get_total_assets(ctx)
    }

    pub fn max_deposit(ctx: Context<VaultView>) -> Result<()> {
        instructions::view::max_deposit(ctx)
    }

    pub fn max_mint(ctx: Context<VaultView>) -> Result<()> {
        instructions::view::max_mint(ctx)
    }

    pub fn max_withdraw(ctx: Context<VaultViewWithOwner>) -> Result<()> {
        instructions::view::max_withdraw(ctx)
    }

    pub fn max_redeem(ctx: Context<VaultViewWithOwner>) -> Result<()> {
        instructions::view::max_redeem(ctx)
    }

    // ============ Module Admin Instructions (requires "modules" feature) ============

    #[cfg(feature = "modules")]
    pub fn initialize_fee_config(
        ctx: Context<InitializeFeeConfig>,
        entry_fee_bps: u16,
        exit_fee_bps: u16,
        management_fee_bps: u16,
        performance_fee_bps: u16,
    ) -> Result<()> {
        instructions::module_admin::initialize_fee_config(
            ctx,
            entry_fee_bps,
            exit_fee_bps,
            management_fee_bps,
            performance_fee_bps,
        )
    }

    #[cfg(feature = "modules")]
    pub fn update_fee_config(
        ctx: Context<UpdateFeeConfig>,
        entry_fee_bps: Option<u16>,
        exit_fee_bps: Option<u16>,
        management_fee_bps: Option<u16>,
        performance_fee_bps: Option<u16>,
    ) -> Result<()> {
        instructions::module_admin::update_fee_config(
            ctx,
            entry_fee_bps,
            exit_fee_bps,
            management_fee_bps,
            performance_fee_bps,
        )
    }

    #[cfg(feature = "modules")]
    pub fn initialize_cap_config(
        ctx: Context<InitializeCapConfig>,
        global_cap: u64,
        per_user_cap: u64,
    ) -> Result<()> {
        instructions::module_admin::initialize_cap_config(ctx, global_cap, per_user_cap)
    }

    #[cfg(feature = "modules")]
    pub fn update_cap_config(
        ctx: Context<UpdateCapConfig>,
        global_cap: Option<u64>,
        per_user_cap: Option<u64>,
    ) -> Result<()> {
        instructions::module_admin::update_cap_config(ctx, global_cap, per_user_cap)
    }

    #[cfg(feature = "modules")]
    pub fn initialize_lock_config(
        ctx: Context<InitializeLockConfig>,
        lock_duration: i64,
    ) -> Result<()> {
        instructions::module_admin::initialize_lock_config(ctx, lock_duration)
    }

    #[cfg(feature = "modules")]
    pub fn update_lock_config(ctx: Context<UpdateLockConfig>, lock_duration: i64) -> Result<()> {
        instructions::module_admin::update_lock_config(ctx, lock_duration)
    }

    #[cfg(feature = "modules")]
    pub fn initialize_access_config(
        ctx: Context<InitializeAccessConfig>,
        mode: state::AccessMode,
        merkle_root: [u8; 32],
    ) -> Result<()> {
        instructions::module_admin::initialize_access_config(ctx, mode, merkle_root)
    }

    #[cfg(feature = "modules")]
    pub fn update_access_config(
        ctx: Context<UpdateAccessConfig>,
        mode: Option<state::AccessMode>,
        merkle_root: Option<[u8; 32]>,
    ) -> Result<()> {
        instructions::module_admin::update_access_config(ctx, mode, merkle_root)
    }
}
