//! sync: Update `vault.total_assets` to match actual wSOL vault balance (Stored model only).

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token::{Token, TokenAccount};

use crate::{
    error::VaultError,
    events::VaultSynced,
    state::{BalanceModel, SolVault},
};

#[derive(Accounts)]
pub struct SyncVault<'info> {
    #[account(
        constraint = authority.key() == vault.authority @ VaultError::Unauthorized,
    )]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub vault: Account<'info, SolVault>,

    #[account(
        mut,
        constraint = wsol_vault.key() == vault.wsol_vault,
    )]
    pub wsol_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<SyncVault>) -> Result<()> {
    require!(
        matches!(ctx.accounts.vault.balance_model, BalanceModel::Stored),
        VaultError::SyncNotSupported
    );

    // Sync native in case someone transferred lamports directly.
    invoke(
        &spl_token::instruction::sync_native(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.wsol_vault.key(),
        )?,
        &[ctx.accounts.wsol_vault.to_account_info()],
    )?;
    ctx.accounts.wsol_vault.reload()?;

    let vault = &mut ctx.accounts.vault;
    let previous_total = vault.total_assets;
    let new_total = ctx.accounts.wsol_vault.amount;

    vault.total_assets = new_total;

    emit!(VaultSynced {
        vault: vault.key(),
        previous_total,
        new_total,
    });

    Ok(())
}
