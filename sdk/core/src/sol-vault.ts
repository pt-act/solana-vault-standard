/**
 * SVS-7 Solana SOL Vault SDK
 *
 * SVS-7 accepts native SOL (lamports) and wraps/unwraps via the canonical wSOL SPL mint.
 * Shares are Token-2022 tokens.
 */

import { BN, Program, AnchorProvider } from "@coral-xyz/anchor";
import {
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  getAccount,
  getMint,
} from "@solana/spl-token";

import * as math from "./math";

export const WSOL_MINT = new PublicKey(
  "So11111111111111111111111111111111111111112",
);

export interface SolVaultState {
  authority: PublicKey;
  sharesMint: PublicKey;
  wsolVault: PublicKey;
  totalAssets: BN;
  decimalsOffset: number;
  balanceModel: unknown;
  bump: number;
  paused: boolean;
  vaultId: BN;
}

export interface DepositParams {
  assets: BN;
  minSharesOut: BN;
}

export interface WithdrawParams {
  assets: BN;
  maxSharesIn: BN;
}

export class SolanaSolVault {
  readonly program: Program;
  readonly provider: AnchorProvider;
  readonly vault: PublicKey;

  readonly sharesMint: PublicKey;
  readonly wsolVault: PublicKey;

  private _state: SolVaultState | null = null;

  protected constructor(
    program: Program,
    provider: AnchorProvider,
    vault: PublicKey,
    sharesMint: PublicKey,
    wsolVault: PublicKey,
  ) {
    this.program = program;
    this.provider = provider;
    this.vault = vault;
    this.sharesMint = sharesMint;
    this.wsolVault = wsolVault;
  }

  static async load(program: Program, vault: PublicKey): Promise<SolanaSolVault> {
    const provider = program.provider as AnchorProvider;

    const accountNs = program.account as Record<
      string,
      { fetch: (addr: PublicKey) => Promise<unknown> }
    >;

    const raw = (await accountNs["solVault"].fetch(vault)) as any;

    const instance = new SolanaSolVault(
      program,
      provider,
      vault,
      raw.sharesMint as PublicKey,
      raw.wsolVault as PublicKey,
    );

    await instance.refresh();
    return instance;
  }

  async refresh(): Promise<SolVaultState> {
    const accountNs = this.program.account as Record<
      string,
      { fetch: (addr: PublicKey) => Promise<unknown> }
    >;
    const raw = (await accountNs["solVault"].fetch(this.vault)) as any;

    this._state = {
      authority: raw.authority as PublicKey,
      sharesMint: raw.sharesMint as PublicKey,
      wsolVault: raw.wsolVault as PublicKey,
      totalAssets: new BN(raw.totalAssets.toString()),
      decimalsOffset: raw.decimalsOffset as number,
      balanceModel: raw.balanceModel,
      bump: raw.bump as number,
      paused: raw.paused as boolean,
      vaultId: new BN(raw.vaultId.toString()),
    };

    return this._state;
  }

  async getState(): Promise<SolVaultState> {
    if (!this._state) {
      await this.refresh();
    }
    return this._state!;
  }

  getUserSharesAccount(owner: PublicKey): PublicKey {
    return getAssociatedTokenAddressSync(
      this.sharesMint,
      owner,
      false,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );
  }

  getUserWsolAccount(owner: PublicKey): PublicKey {
    return getAssociatedTokenAddressSync(
      WSOL_MINT,
      owner,
      false,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );
  }

  async totalAssets(): Promise<BN> {
    const account = await getAccount(
      this.provider.connection,
      this.wsolVault,
      undefined,
      TOKEN_PROGRAM_ID,
    );
    return new BN(account.amount.toString());
  }

  async totalShares(): Promise<BN> {
    const mint = await getMint(
      this.provider.connection,
      this.sharesMint,
      undefined,
      TOKEN_2022_PROGRAM_ID,
    );
    return new BN(mint.supply.toString());
  }

  async previewDeposit(assets: BN): Promise<BN> {
    const state = await this.refresh();
    const totalAssets = await this.totalAssets();
    const totalShares = await this.totalShares();
    return math.previewDeposit(
      assets,
      totalAssets,
      totalShares,
      state.decimalsOffset,
    );
  }

  async previewWithdraw(assets: BN): Promise<BN> {
    const state = await this.refresh();
    const totalAssets = await this.totalAssets();
    const totalShares = await this.totalShares();
    return math.previewWithdraw(
      assets,
      totalAssets,
      totalShares,
      state.decimalsOffset,
    );
  }

  async depositSol(user: PublicKey, params: DepositParams): Promise<string> {
    const userSharesAccount = this.getUserSharesAccount(user);

    return (this.program.methods as any)
      .depositSol(params.assets, params.minSharesOut)
      .accountsStrict({
        user,
        vault: this.vault,
        wsolMint: WSOL_MINT,
        wsolVault: this.wsolVault,
        sharesMint: this.sharesMint,
        userSharesAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
  }

  async depositWsol(user: PublicKey, params: DepositParams): Promise<string> {
    const userSharesAccount = this.getUserSharesAccount(user);
    const userWsolAccount = this.getUserWsolAccount(user);

    return (this.program.methods as any)
      .depositWsol(params.assets, params.minSharesOut)
      .accountsStrict({
        user,
        vault: this.vault,
        wsolMint: WSOL_MINT,
        userWsolAccount,
        wsolVault: this.wsolVault,
        sharesMint: this.sharesMint,
        userSharesAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
  }

  async withdrawSol(
    user: PublicKey,
    receiver: PublicKey,
    params: WithdrawParams,
  ): Promise<string> {
    const userSharesAccount = this.getUserSharesAccount(user);

    const [tempWsolAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("temp_wsol"), this.vault.toBuffer(), user.toBuffer()],
      this.program.programId,
    );

    return (this.program.methods as any)
      .withdrawSol(params.assets, params.maxSharesIn)
      .accountsStrict({
        user,
        receiver,
        vault: this.vault,
        wsolMint: WSOL_MINT,
        wsolVault: this.wsolVault,
        sharesMint: this.sharesMint,
        userSharesAccount,
        tempWsolAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();
  }

  async withdrawWsol(user: PublicKey, params: WithdrawParams): Promise<string> {
    const userSharesAccount = this.getUserSharesAccount(user);
    const userWsolAccount = this.getUserWsolAccount(user);

    return (this.program.methods as any)
      .withdrawWsol(params.assets, params.maxSharesIn)
      .accountsStrict({
        user,
        vault: this.vault,
        wsolMint: WSOL_MINT,
        userWsolAccount,
        wsolVault: this.wsolVault,
        sharesMint: this.sharesMint,
        userSharesAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .rpc();
  }
}
