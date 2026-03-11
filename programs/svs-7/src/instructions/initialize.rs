//! Initialize instruction: create sol_vault PDA, shares mint, and wSOL vault.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
    token_2022::{
        spl_token_2022::{extension::ExtensionType, instruction::initialize_mint2},
        Token2022,
    },
};

use crate::{
    constants::{SHARES_DECIMALS, SHARES_MINT_SEED, SOL_VAULT_SEED, WSOL_MINT},
    error::VaultError,
    events::VaultInitialized,
    state::{validate_wsol_mint, BalanceModel, SolVault},
};

#[derive(Accounts)]
#[instruction(vault_id: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = SolVault::LEN,
        seeds = [SOL_VAULT_SEED, &vault_id.to_le_bytes()],
        bump
    )]
    pub vault: Account<'info, SolVault>,

    /// CHECK: Shares mint is initialized via CPI in handler
    #[account(
        mut,
        seeds = [SHARES_MINT_SEED, vault.key().as_ref()],
        bump
    )]
    pub shares_mint: UncheckedAccount<'info>,

    /// Canonical wSOL mint (SPL Token program).
    pub wsol_mint: Account<'info, Mint>,

    /// PDA-owned wSOL token account (ATA under SPL Token program).
    #[account(
        init,
        payer = authority,
        associated_token::mint = wsol_mint,
        associated_token::authority = vault,
        associated_token::token_program = token_program,
    )]
    pub wsol_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<Initialize>,
    vault_id: u64,
    balance_model: BalanceModel,
) -> Result<()> {
    // Validate mint is the canonical wSOL mint.
    validate_wsol_mint(&ctx.accounts.wsol_mint.key())?;

    let vault_key = ctx.accounts.vault.key();
    let shares_mint_bump = ctx.bumps.shares_mint;

    // Calculate space for a basic Token-2022 mint (no extensions)
    let mint_size = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&[])
        .map_err(|_| VaultError::MathOverflow)?;

    let lamports = ctx.accounts.rent.minimum_balance(mint_size);

    let shares_mint_seeds: &[&[u8]] = &[
        SHARES_MINT_SEED,
        vault_key.as_ref(),
        &[shares_mint_bump],
    ];

    // Create shares mint account
    invoke_signed(
        &anchor_lang::solana_program::system_instruction::create_account(
            &ctx.accounts.authority.key(),
            &ctx.accounts.shares_mint.key(),
            lamports,
            mint_size as u64,
            &ctx.accounts.token_2022_program.key(),
        ),
        &[
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.shares_mint.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[shares_mint_seeds],
    )?;

    // Initialize shares mint (vault PDA is mint authority)
    let init_mint_ix = initialize_mint2(
        &ctx.accounts.token_2022_program.key(),
        &ctx.accounts.shares_mint.key(),
        &vault_key,
        None,
        SHARES_DECIMALS,
    )?;

    invoke_signed(
        &init_mint_ix,
        &[ctx.accounts.shares_mint.to_account_info()],
        &[shares_mint_seeds],
    )?;

    // Set vault state
    let vault = &mut ctx.accounts.vault;
    vault.authority = ctx.accounts.authority.key();
    vault.shares_mint = ctx.accounts.shares_mint.key();
    vault.wsol_vault = ctx.accounts.wsol_vault.key();
    vault.total_assets = 0;
    vault.decimals_offset = 0; // SOL has 9 decimals
    vault.balance_model = balance_model;
    vault.bump = ctx.bumps.vault;
    vault.paused = false;
    vault.vault_id = vault_id;
    vault._reserved = [0u8; 64];

    emit!(VaultInitialized {
        vault: vault.key(),
        authority: vault.authority,
        asset_mint: ctx.accounts.wsol_mint.key(),
        shares_mint: vault.shares_mint,
        vault_id,
    });

    msg!("SVS-7 vault initialized for wSOL mint {}", WSOL_MINT);

    Ok(())
}
