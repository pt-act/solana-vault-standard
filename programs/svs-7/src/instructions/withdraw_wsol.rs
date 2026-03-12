//! withdraw_wsol: burn shares and receive wSOL.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token::{Mint as SplMint, Token, TokenAccount as SplTokenAccount, Transfer},
    token_2022::{self, Burn, Token2022},
    token_interface::{Mint as InterfaceMint, TokenAccount as InterfaceTokenAccount},
};

use crate::{
    constants::SOL_VAULT_SEED,
    error::VaultError,
    events::Withdraw as WithdrawEvent,
    math::{convert_to_shares, Rounding},
    state::{validate_wsol_mint, BalanceModel, SolVault},
};

#[cfg(feature = "modules")]
use svs_module_hooks as module_hooks;

#[derive(Accounts)]
pub struct WithdrawWsol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Account<'info, SolVault>,

    pub wsol_mint: Account<'info, SplMint>,

    #[account(
        mut,
        constraint = user_wsol_account.mint == wsol_mint.key(),
        constraint = user_wsol_account.owner == user.key(),
    )]
    pub user_wsol_account: Account<'info, SplTokenAccount>,

    #[account(
        mut,
        constraint = wsol_vault.key() == vault.wsol_vault,
    )]
    pub wsol_vault: Account<'info, SplTokenAccount>,

    #[account(
        mut,
        constraint = shares_mint.key() == vault.shares_mint,
    )]
    pub shares_mint: InterfaceAccount<'info, InterfaceMint>,

    #[account(
        mut,
        constraint = user_shares_account.mint == vault.shares_mint,
        constraint = user_shares_account.owner == user.key(),
    )]
    pub user_shares_account: InterfaceAccount<'info, InterfaceTokenAccount>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

pub fn handler(ctx: Context<WithdrawWsol>, assets: u64, max_shares_in: u64) -> Result<()> {
    require!(assets > 0, VaultError::ZeroAmount);

    validate_wsol_mint(&ctx.accounts.wsol_mint.key())?;

    // Sync native in case someone transferred lamports directly.
    invoke(
        &spl_token::instruction::sync_native(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.wsol_vault.key(),
        )?,
        &[ctx.accounts.wsol_vault.to_account_info()],
    )?;
    ctx.accounts.wsol_vault.reload()?;

    let vault = &ctx.accounts.vault;
    let total_shares = ctx.accounts.shares_mint.supply;

    let total_assets = match vault.balance_model {
        BalanceModel::Live => ctx.accounts.wsol_vault.amount,
        BalanceModel::Stored => vault.total_assets,
    };

    // ===== Module Hooks (if enabled) =====
    #[cfg(feature = "modules")]
    let net_assets = {
        let remaining = ctx.remaining_accounts;
        let clock = Clock::get()?;
        let vault_key = vault.key();
        let user_key = ctx.accounts.user.key();

        module_hooks::check_deposit_access(remaining, ctx.program_id, &vault_key, &user_key, &[])?;
        module_hooks::check_share_lock(
            remaining,
            ctx.program_id,
            &vault_key,
            &user_key,
            clock.unix_timestamp,
        )?;

        let result = module_hooks::apply_exit_fee(remaining, ctx.program_id, &vault_key, assets)?;
        result.net_assets
    };

    #[cfg(not(feature = "modules"))]
    let net_assets = assets;

    require!(assets <= total_assets, VaultError::InsufficientAssets);

    let shares = convert_to_shares(
        assets,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Ceiling,
    )?;

    require!(shares <= max_shares_in, VaultError::SlippageExceeded);
    require!(
        ctx.accounts.user_shares_account.amount >= shares,
        VaultError::InsufficientShares
    );

    // Burn shares.
    token_2022::burn(
        CpiContext::new(
            ctx.accounts.token_2022_program.to_account_info(),
            Burn {
                mint: ctx.accounts.shares_mint.to_account_info(),
                from: ctx.accounts.user_shares_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        shares,
    )?;

    // Transfer net wSOL to user.
    let vault_id_bytes = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = ctx.accounts.vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[SOL_VAULT_SEED, vault_id_bytes.as_ref(), &[bump]]];

    anchor_spl::token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.wsol_vault.to_account_info(),
                to: ctx.accounts.user_wsol_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        net_assets,
    )?;

    // Update cached total assets
    if matches!(ctx.accounts.vault.balance_model, BalanceModel::Stored) {
        let vault_mut = &mut ctx.accounts.vault;
        vault_mut.total_assets = vault_mut
            .total_assets
            .checked_sub(net_assets)
            .ok_or(VaultError::MathOverflow)?;
    }

    emit!(WithdrawEvent {
        vault: ctx.accounts.vault.key(),
        caller: ctx.accounts.user.key(),
        receiver: ctx.accounts.user.key(),
        owner: ctx.accounts.user.key(),
        assets: net_assets,
        shares,
    });

    Ok(())
}
