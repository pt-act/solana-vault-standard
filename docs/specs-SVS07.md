# SVS-7: Native SOL Vault

## Status: Draft
## Authors: Superteam Brasil
## Date: 2026-03-06
## Base: ERC-7535 adapted — Native asset vault for Solana

---

## 1. Overview

SVS-7 accepts and returns native SOL instead of SPL tokens. It handles SOL ↔ wSOL wrapping internally so users interact with native lamports while the vault's internal accounting uses a wSOL token account. Shares are still Token-2022 SPL tokens.

This vault type targets liquid staking, SOL yield strategies, and any product where requiring users to pre-wrap SOL creates unnecessary friction.

---

## 2. How It Differs from SVS-1

| Aspect | SVS-1 | SVS-7 |
|--------|-------|-------|
| Asset token | Any SPL / Token-2022 mint | Native SOL (lamports) |
| User interaction | Transfer SPL tokens | Transfer native SOL via system_program |
| Internal accounting | SPL token account balance | wSOL token account balance |
| Wrap/unwrap | User's responsibility | Vault handles internally |
| Asset mint | Configurable | Always `So11111111111111111111111111111111111111112` (canonical wrapped SOL / native mint) |

---

## 3. State

```rust
#[account]
pub struct SolVault {
    pub authority: Pubkey,
    pub shares_mint: Pubkey, // Token-2022 share token

    /// PDA-owned wSOL token account (SPL Token program).
    pub wsol_vault: Pubkey,

    /// Cached assets in lamports. Used only when `balance_model == Stored`.
    pub total_assets: u64,

    /// Decimal offset exponent used by share conversion math.
    /// For SOL (9 decimals), this is 0 and the offset is 10^0 = 1.
    pub decimals_offset: u8,

    pub balance_model: BalanceModel,
    pub bump: u8,
    pub paused: bool,
    pub vault_id: u64,
    pub _reserved: [u8; 64],
}
// seeds: ["sol_vault", vault_id.to_le_bytes()]

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum BalanceModel {
    /// Reads `wsol_vault.amount` each instruction.
    Live,
    /// Caches `total_assets` in the vault account, updated via `sync()` and operations.
    Stored,
}
```

---

## 4. Instruction Set

| # | Instruction | Signer | Description |
|---|------------|--------|-------------|
| 1 | `initialize` | Authority | Creates SolVault PDA, share mint, wSOL vault account |
| 2 | `deposit_sol` | User | Transfers native SOL → wraps to wSOL → mints shares |
| 3 | `deposit_wsol` | User | Transfers existing wSOL → mints shares (no wrap needed) |
| 4 | `mint_sol` | User | Mints exact shares, pays native SOL |
| 5 | `withdraw_sol` | User | Burns shares → unwraps wSOL → transfers native SOL to user |
| 6 | `withdraw_wsol` | User | Burns shares → transfers wSOL to user (no unwrap) |
| 7 | `redeem_sol` | User | Redeems shares for native SOL |
| 8 | `redeem_wsol` | User | Redeems shares for wSOL |
| 9 | `sync` | Authority | Updates total_assets (Stored model only) |
| 10 | `pause` / `unpause` | Authority | Emergency controls |
| 11 | `transfer_authority` | Authority | Transfer admin |

### 4.1 `deposit_sol` Flow

```
deposit_sol(lamports: u64, min_shares_out: u64):
  ✓ lamports > 0
  ✓ lamports >= MIN_DEPOSIT_AMOUNT
  ✓ vault not paused
  → system_program::transfer(user → vault_wsol_ata, lamports)
  → spl_token::sync_native(vault_wsol_ata)
  → Compute shares = convert_to_shares(lamports, total_shares, total_assets, decimals_offset)
  → require!(shares >= min_shares_out)
  → Mint shares to user
  → Update total_assets (if Stored model)
  → emit Deposit { vault, caller, owner, assets: lamports, shares }
```

### 4.2 `withdraw_sol` Flow

```
withdraw_sol(lamports: u64, max_shares_in: u64):
  ✓ lamports > 0
  ✓ vault not paused
  → Compute shares = convert_to_shares(lamports, total_shares, total_assets, decimals_offset) // ceiling
  → require!(shares <= max_shares_in)
  → Burn shares from user
  → Transfer wSOL from vault_wsol_ata → temp_wsol_account
  → Close temp_wsol_account to user (unwraps to native SOL)
  → system_program::transfer(user → receiver, lamports)
  → Update total_assets (if Stored model)
  → emit Withdraw { vault, caller, receiver, owner, assets: lamports, shares }
```

---

## 5. SOL Wrapping Mechanics

Solana's canonical wrapped SOL / native mint (`So11111111111111111111111111111111111111112`) requires special handling:

**Depositing SOL:**
1. User transfers native lamports to the vault's wSOL token account (an SPL Token ATA owned by the vault PDA) via `system_program::transfer`
2. Vault calls `sync_native` on that wSOL token account to update `amount` to match the underlying lamports
3. Internal accounting uses the wSOL token account balance

**Withdrawing SOL:**
1. Vault transfers wSOL from the vault wSOL ATA to a temporary wSOL token account (owned by the vault PDA)
2. Vault calls `close_account` on the temporary account, which unwraps wSOL to native lamports sent to the user (then forwarded to the receiver if needed)

**Alternative approach:** The vault PDA holds native SOL directly (no wSOL account). `total_assets` = vault PDA lamport balance minus rent. This is simpler but makes CPI to external DeFi protocols harder since most expect SPL token accounts. The wSOL approach is preferred for composability.

---

## 6. Rent Handling

In the wSOL-account approach (recommended and used by the current reference implementation), the vault PDA itself does not hold user assets. Assets live in the vault's wSOL token account (an SPL Token account), which must be rent-exempt.

- SPL Token account size is 165 bytes (`spl_token::state::Account::LEN`).
- Rent-exempt lamports should be computed from the cluster's `Rent` sysvar.

If implementing an alternative design where the vault PDA holds SOL directly, rent-exempt lamports on the vault PDA must be excluded from `total_assets`.

---

## 7. Decimals

SOL has 9 decimals. The virtual offset exponent is `9 - 9 = 0`, so `offset = 10^0 = 1`. This provides minimal inflation attack protection. Consider using a higher fixed offset (e.g., `offset = 1_000`) for SOL vaults, or requiring a minimum initial deposit.

---

## 8. Dual Interface

SVS-7 exposes both `_sol` and `_wsol` variants for each operation. This allows:
- End users to interact with native SOL (better UX)
- Protocols and smart contracts to interact with wSOL (better composability)
- The vault's internal state is identical regardless of which interface is used

---

## 9. Module Compatibility

**Implementation:** Build with `--features modules`. Module config PDAs passed via `remaining_accounts`.

All modules from [MODULES.md](./MODULES.md) are compatible:

- **svs-fees:** Fees computed on lamport amounts. Fee assets sent as native SOL to fee_recipient.
- **svs-caps:** Caps denominated in lamports.
- **svs-locks:** Works identically (share-based, not asset-based).
- **svs-rewards:** Reward tokens are separate mints, unaffected by SOL handling.
- **svs-access:** Identity-based checks, fully compatible.

---

## 10. Use Cases

- **Liquid staking vaults:** Accept SOL, stake across validators, issue liquid staking shares. Yield distributed via Stored model + `sync()`.
- **SOL savings vaults:** Accept SOL, deploy to lending protocols (Kamino, MarginFi), auto-compound. Live model reads returns directly.
- **SOL DCA vaults:** Accept SOL, shares represent a position that executes periodic swaps. Streaming model (SVS-5 style) could be layered on.

---

## 11. sync_native CPI Pattern

The key to SOL handling is the `sync_native` instruction from SPL Token, which updates a wSOL token account's balance to match its underlying lamport balance:

```rust
/// Sync native SOL balance after direct transfer
pub fn sync_native_cpi<'info>(
    token_program: &AccountInfo<'info>,
    native_account: &AccountInfo<'info>,
) -> Result<()> {
    let ix = spl_token::instruction::sync_native(
        token_program.key,
        native_account.key,
    )?;

    invoke(&ix, &[native_account.clone()])?;
    Ok(())
}

// Full deposit_sol implementation
pub fn deposit_sol(ctx: Context<DepositSol>, lamports: u64, min_shares_out: u64) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    require!(!vault.paused, VaultError::VaultPaused);
    require!(lamports > 0, VaultError::ZeroAmount);

    // 1. Sync native first so conversions see any prior lamport donations.
    sync_native_cpi(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.wsol_vault.to_account_info(),
    )?;
    ctx.accounts.wsol_vault.reload()?;

    // 2. Compute shares using the pre-transfer total_assets.
    let total_assets_before = match vault.balance_model {
        BalanceModel::Live => ctx.accounts.wsol_vault.amount,
        BalanceModel::Stored => vault.total_assets,
    };
    let total_shares = ctx.accounts.shares_mint.supply;
    let offset = 10u64.pow(vault.decimals_offset as u32);

    let shares = convert_to_shares(lamports, total_shares, total_assets_before, offset)?;

    // 3. Transfer native SOL from user to the vault's wSOL account.
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.depositor.to_account_info(),
                to: ctx.accounts.wsol_vault.to_account_info(),
            },
        ),
        lamports,
    )?;

    // 4. Sync native so `wsol_vault.amount` reflects the new lamport balance.
    sync_native_cpi(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.wsol_vault.to_account_info(),
    )?;
    ctx.accounts.wsol_vault.reload()?;
    require!(shares >= min_shares_out, VaultError::SlippageExceeded);

    // 5. Mint shares
    mint_to_with_signer(ctx, shares)?;

    // 6. Update stored balance if applicable
    if vault.balance_model == BalanceModel::Stored {
        vault.total_assets = vault.total_assets.checked_add(lamports)?;
    }

    emit!(Deposit {
        vault: vault.key(),
        depositor: ctx.accounts.depositor.key(),
        assets: lamports,
        shares,
    });

    Ok(())
}
```

---

## 12. deposit_sol vs deposit_wsol Comparison

| Aspect | `deposit_sol` | `deposit_wsol` |
|--------|--------------|----------------|
| **Input** | Native SOL (lamports) | Pre-wrapped wSOL |
| **User Action** | Send SOL directly | Wrap SOL first, then send |
| **Compute Cost** | ~15,000 CU | ~12,000 CU |
| **Account Count** | Requires system_program | Standard SPL transfer |
| **User Experience** | One-click, seamless | Two-step (wrap + deposit) |
| **Use Case** | End users, wallets | Protocols, composability |

### Flow Diagrams

**deposit_sol Flow**:
```
User SOL → system_program::transfer → vault_wsol_ata → sync_native → mint_shares
         (native lamports)           (updates token balance)      (shares to user)
```

**deposit_wsol Flow**:
```
User wSOL → spl_token::transfer_checked → vault_wsol_ata → mint_shares
         (pre-wrapped)                    (standard transfer)  (shares to user)
```

**Recommendation**: Expose both interfaces. Use `deposit_sol` for end-user UIs, `deposit_wsol` for protocol integrations that already hold wSOL.

---

## 13. Concrete Example: Deposit 5 SOL

### Initial State
- User balance: 10 SOL
- Vault wSOL balance: 100 SOL (100,000,000,000 lamports)
- Shares outstanding: 100 shares (100,000,000,000 base units at 9 decimals)
- Share price: 1 SOL = 1 share

### deposit_sol(5_000_000_000, 4_500_000_000)
*Depositing 5 SOL with 10% slippage tolerance*

1. **sync_native()** (optional pre-step): Ensure token `amount` reflects any prior lamport changes
   - Vault wSOL `amount` field: 100 SOL

2. **Calculate shares (pre-transfer totals)**:
   - Before deposit: `total_assets = 100 SOL`, `total_shares = 100 shares`
   - `decimals_offset = 0` → `offset = 10^0 = 1`
   - `shares = 5 * (100 + 1) / (100 + 1) = 5 shares`

3. **system_program::transfer + sync_native()**: 5 SOL from user to vault_wsol_ata
   - Vault wSOL `amount` field: now 105 SOL

4. **Mint 5 shares to user**

### Final State
- User balance: 5 SOL, 5 shares
- Vault wSOL balance: 105 SOL
- Shares outstanding: 105 shares
- Share price: still 1 SOL = 1 share (unchanged, as expected for deposit)

---

## 14. Compute Unit Estimates

| Instruction | Approximate CU | Notes |
|-------------|---------------|-------|
| `initialize` | ~30,000 | Create vault + shares mint + wSOL vault |
| `deposit_sol` | ~15,000 | system_transfer + sync_native + mint |
| `deposit_wsol` | ~12,000 | SPL transfer + mint (no sync needed) |
| `withdraw_sol` | ~20,000 | burn + transfer + close_account (unwrap) |
| `withdraw_wsol` | ~15,000 | burn + SPL transfer |
| `sync` (Stored model) | ~8,000 | State update |

---

## See Also

- [SVS-1](./SVS-1.md) — Base SPL token vault
- [SVS-2](./SVS-2.md) — Stored balance model
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Cross-variant design
- [MODULES.md](./MODULES.md) — Module integration
