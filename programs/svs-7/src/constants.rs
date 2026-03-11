//! Program constants: PDA seeds, limits, and native SOL configuration.

pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";
pub const SHARES_MINT_SEED: &[u8] = b"shares";

/// Wrapped SOL mint (native mint)
/// https://docs.solana.com/developing/programming-model/accounts#native-program-owned-accounts
pub const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

pub const SHARES_DECIMALS: u8 = 9;

/// Minimum deposit amount in lamports (anti-dust)
pub const MIN_DEPOSIT_AMOUNT: u64 = 1000;

/// Temporary wSOL token account PDA seed for SOL unwrapping
pub const TEMP_WSOL_SEED: &[u8] = b"temp_wsol";
