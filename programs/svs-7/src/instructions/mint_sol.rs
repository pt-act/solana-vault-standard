//! mint_sol: mint exact shares by depositing SOL.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint as SplMint, Token, TokenAccount as SplTokenAccount},
    token_2022::{self, MintTo, Token2022},
    token_interface::{Mint as InterfaceMint, TokenAccount as InterfaceTokenAccount},
};

use crate::{
    constants::{MIN_DEPOSIT_AMOUNT, SOL_VAULT_SEED},
    error::VaultError,
    events::Deposit as DepositEvent,
    math::{convert_to_assets, Rounding},
    state::{validate_wsol_mint, BalanceModel, SolVault},
};

#[cfg(feature = "modules")]
use svs_module_hooks as module_hooks;

#[derive(Accounts)]
pub struct MintSol<'info> {
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
        constraint = wsol_vault.key() == vault.wsol_vault,
    )]
    pub wsol_vault: Account<'info, SplTokenAccount>,

    #[account(
        mut,
        constraint = shares_mint.key() == vault.shares_mint,
    )]
    pub shares_mint: InterfaceAccount<'info, InterfaceMint>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = shares_mint,
        associated_token::authority = user,
        associated_token::token_program = token_2022_program,
    )]
    pub user_shares_account: InterfaceAccount<'info, InterfaceTokenAccount>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<MintSol>, shares: u64, max_assets_in: u64) -> Result<()> {
    require!(shares > 0, VaultError::ZeroAmount);

    validate_wsol_mint(&ctx.accounts.wsol_mint.key())?;

    // Sync native before reading balance.
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

    // Compute required assets (ceiling rounding - favors vault)
    let assets = convert_to_assets(
        shares,
        total_assets,
        total_shares,
        vault.decimals_offset,
        Rounding::Ceiling,
    )?;

    // ===== Module Hooks (if enabled) =====
    #[cfg(feature = "modules")]
    let net_shares = {
        let remaining = ctx.remaining_accounts;
        let vault_key = vault.key();
        let user_key = ctx.accounts.user.key();

        module_hooks::check_deposit_access(remaining, ctx.program_id, &vault_key, &user_key, &[])?;
        module_hooks::check_deposit_caps(
            remaining,
            ctx.program_id,
            &vault_key,
            &user_key,
            total_assets,
            assets,
        )?;

        // NOTE: For `mint_sol`, fee is applied in shares, not assets.
        let result = module_hooks::apply_entry_fee(remaining, ctx.program_id, &vault_key, shares)?;
        result.net_shares
    };

    #[cfg(not(feature = "modules"))]
    let net_shares = shares;

    // The caller specifies max assets they'll pay; fee affects shares received.
    require!(assets <= max_assets_in, VaultError::SlippageExceeded);
    require!(assets >= MIN_DEPOSIT_AMOUNT, VaultError::DepositTooSmall);

    // Transfer lamports to wsol_vault and sync
    invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.wsol_vault.key(),
            assets,
        ),
        &[
            ctx.accounts.user.to_account_info(),
            ctx.accounts.wsol_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;

    invoke(
        &spl_token::instruction::sync_native(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.wsol_vault.key(),
        )?,
        &[ctx.accounts.wsol_vault.to_account_info()],
    )?;

    // Mint shares
    let vault_id_bytes = ctx.accounts.vault.vault_id.to_le_bytes();
    let bump = ctx.accounts.vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[SOL_VAULT_SEED, vault_id_bytes.as_ref(), &[bump]]];

    token_2022::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.shares_mint.to_account_info(),
                to: ctx.accounts.user_shares_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer_seeds,
        ),
        net_shares,
    )?;

    if let BalanceModel::Stored = ctx.accounts.vault.balance_model {
        let vault_mut = &mut ctx.accounts.vault;
        vault_mut.total_assets = vault_mut
            .total_assets
            .checked_add(assets)
            .ok_or(VaultError::MathOverflow)?;
    }

    emit!(DepositEvent {
        vault: ctx.accounts.vault.key(),
        caller: ctx.accounts.user.key(),
        owner: ctx.accounts.user.key(),
        assets,
        shares: net_shares,
    });

    Ok(())
}
