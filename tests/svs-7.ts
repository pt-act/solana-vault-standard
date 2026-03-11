import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  createSyncNativeInstruction,
  getAccount,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { expect } from "chai";

// NOTE:
// - This suite is patterned after tests/svs-1.ts and tests/svs-2.ts.
// - It is guarded so it does not fail CI until SVS-7 is wired into the Anchor workspace.
//   To enable, add svs_7 to Anchor.toml and ensure the program builds + generates an IDL.

describe("svs-7 (Native SOL Vault)", function () {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const connection = provider.connection;
  const payer = (provider.wallet as anchor.Wallet).payer;

  let program: Program<any>;

  const WSOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");
  const LAMPORTS_PER_SOL = 1_000_000_000;

  const getVaultPDA = (vaultId: BN): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("sol_vault"), vaultId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
  };

  const getSharesMintPDA = (vault: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync([Buffer.from("shares"), vault.toBuffer()], program.programId);
  };

  const getWsolVaultAta = (vault: PublicKey): PublicKey => {
    // Spec expectation: wSOL is the canonical SPL Token mint, so the wSOL vault should be a Token-Program ATA.
    // If SVS-7 decides to use Token-2022 for the vault token account, this will need to switch to TOKEN_2022_PROGRAM_ID.
    return getAssociatedTokenAddressSync(
      WSOL_MINT,
      vault,
      true,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );
  };

  const checkModulesFeature = (): boolean => {
    try {
      return (program.idl as any).instructions.some((ix: { name: string }) =>
        ix.name === "initializeFeeConfig" || ix.name === "initialize_fee_config"
      );
    } catch {
      return false;
    }
  };

  const getFeeConfigPDA = (vault: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync([Buffer.from("fee_config"), vault.toBuffer()], program.programId);
  };

  const getCapConfigPDA = (vault: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync([Buffer.from("cap_config"), vault.toBuffer()], program.programId);
  };

  const getAccessConfigPDA = (vault: PublicKey): [PublicKey, number] => {
    return PublicKey.findProgramAddressSync([Buffer.from("access_config"), vault.toBuffer()], program.programId);
  };

  const getSharesAta = (sharesMint: PublicKey, owner: PublicKey): PublicKey => {
    return getAssociatedTokenAddressSync(
      sharesMint,
      owner,
      false,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );
  };

  async function getTokenAccountAmount(tokenAccount: PublicKey): Promise<bigint> {
    // SVS-7 wSOL vault is expected to be Token Program, but this helper supports either.
    try {
      return (await getAccount(connection, tokenAccount, undefined, TOKEN_PROGRAM_ID)).amount;
    } catch {
      return (await getAccount(connection, tokenAccount, undefined, TOKEN_2022_PROGRAM_ID)).amount;
    }
  }

  async function wrapSolToAta(owner: Keypair, ata: PublicKey, lamports: number): Promise<void> {
    const tx = new anchor.web3.Transaction().add(
      SystemProgram.transfer({
        fromPubkey: owner.publicKey,
        toPubkey: ata,
        lamports,
      }),
      createSyncNativeInstruction(ata)
    );

    await provider.sendAndConfirm(tx, [owner]);
  }

  async function initializeVault(vaultId: BN, balanceModel: any): Promise<{
    vault: PublicKey;
    sharesMint: PublicKey;
    wsolVault: PublicKey;
  }> {
    const [vault] = getVaultPDA(vaultId);
    const [sharesMint] = getSharesMintPDA(vault);
    const wsolVault = getWsolVaultAta(vault);

    const tx = await (program.methods as any)
      .initialize(vaultId, balanceModel)
      .accountsStrict({
        authority: payer.publicKey,
        vault,
        sharesMint,
        wsolMint: WSOL_MINT,
        wsolVault,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    console.log("Initialize tx:", tx);

    return { vault, sharesMint, wsolVault };
  }

  before(function () {
    const p = (anchor.workspace as any).Svs7;
    if (!p) {
      console.log("⚠️  SVS-7 program not found in anchor.workspace. Add svs_7 to Anchor.toml to enable these tests.");
      this.skip();
    }

    program = p as Program;
  });

  describe("Live balance model (reads wSOL vault amount)", () => {
    const vaultId = new BN(7001);

    let vault: PublicKey;
    let sharesMint: PublicKey;
    let wsolVault: PublicKey;
    let userSharesAccount: PublicKey;

    before(async () => {
      ({ vault, sharesMint, wsolVault } = await initializeVault(vaultId, { live: {} }));
      userSharesAccount = getSharesAta(sharesMint, payer.publicKey);

      console.log("Setup (Live):");
      console.log("  Program ID:", program.programId.toBase58());
      console.log("  Vault:", vault.toBase58());
      console.log("  Shares Mint:", sharesMint.toBase58());
      console.log("  wSOL Vault:", wsolVault.toBase58());
    });

    it("deposit_sol mints shares and increases wSOL vault", async () => {
      const depositLamports = new BN(1 * LAMPORTS_PER_SOL);

      const wsolBefore = await getTokenAccountAmount(wsolVault);

      await (program.methods as any)
        .depositSol(depositLamports, new BN(0))
        .accountsStrict({
          user: payer.publicKey,
          vault,
          wsolMint: WSOL_MINT,
          wsolVault,
          sharesMint,
          userSharesAccount,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const userShares = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
      const wsolAfter = await getTokenAccountAmount(wsolVault);

      expect(Number(userShares.amount)).to.be.greaterThan(0);
      expect(wsolAfter - wsolBefore).to.equal(BigInt(depositLamports.toString()));
    });

    it("deposit_wsol deposits wrapped SOL and mints shares", async () => {
      const depositLamports = 100_000_000; // 0.1 SOL

      const userWsolAta = await getOrCreateAssociatedTokenAccount(
        connection,
        payer,
        WSOL_MINT,
        payer.publicKey,
        false,
        undefined,
        undefined,
        TOKEN_PROGRAM_ID
      );

      await wrapSolToAta(payer, userWsolAta.address, depositLamports);

      const sharesBefore = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);

      await (program.methods as any)
        .depositWsol(new BN(depositLamports), new BN(0))
        .accountsStrict({
          user: payer.publicKey,
          vault,
          wsolMint: WSOL_MINT,
          userWsolAccount: userWsolAta.address,
          wsolVault,
          sharesMint,
          userSharesAccount,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const sharesAfter = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
      expect(Number(sharesAfter.amount)).to.be.greaterThan(Number(sharesBefore.amount));
    });

    it("mint_sol mints exact shares", async () => {
      const sharesToMint = new BN(10_000_000); // 0.01 shares (9 decimals)

      const sharesBefore = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);

      await (program.methods as any)
        .mintSol(sharesToMint, new BN("18446744073709551615"))
        .accountsStrict({
          user: payer.publicKey,
          vault,
          wsolMint: WSOL_MINT,
          wsolVault,
          sharesMint,
          userSharesAccount,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const sharesAfter = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
      expect(Number(sharesAfter.amount) - Number(sharesBefore.amount)).to.equal(sharesToMint.toNumber());
    });
  });

  describe("Stored balance model (tracks total_assets in vault)", () => {
    const vaultId = new BN(7002);

    let vault: PublicKey;
    let sharesMint: PublicKey;
    let wsolVault: PublicKey;
    let userSharesAccount: PublicKey;

    before(async () => {
      ({ vault, sharesMint, wsolVault } = await initializeVault(vaultId, { stored: {} }));
      userSharesAccount = getSharesAta(sharesMint, payer.publicKey);

      console.log("Setup (Stored):");
      console.log("  Vault:", vault.toBase58());
      console.log("  Shares Mint:", sharesMint.toBase58());
      console.log("  wSOL Vault:", wsolVault.toBase58());
    });

    it("deposit_sol increments stored total_assets", async () => {
      const depositLamports = new BN(500_000_000); // 0.5 SOL

      await (program.methods as any)
        .depositSol(depositLamports, new BN(0))
        .accountsStrict({
          user: payer.publicKey,
          vault,
          wsolMint: WSOL_MINT,
          wsolVault,
          sharesMint,
          userSharesAccount,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const vaultAccount = await (program.account as any).solVault.fetch(vault);
      expect(vaultAccount.totalAssets.toString()).to.equal(depositLamports.toString());
    });

    it("sync is expected to exist for Stored model", async () => {
      // Once SVS-7 sync is implemented + added to IDL, this should be updated to call it.
      expect((program.methods as any).sync).to.not.be.undefined;
    });
  });

  describe("Modules smoke (fees / caps / access)", function () {
    before(function () {
      if (!checkModulesFeature()) {
        this.skip();
      }
    });

    it("entry fee reduces minted shares (deposit_sol)", async () => {
      const vaultId = new BN(7101);
      const { vault, sharesMint, wsolVault } = await initializeVault(vaultId, { live: {} });

      const [feeConfig] = getFeeConfigPDA(vault);
      const feeRecipient = Keypair.generate();

      await (program.methods as any)
        .initializeFeeConfig(100, 0, 0, 0) // 1% entry fee
        .accountsStrict({
          authority: payer.publicKey,
          vault,
          feeConfig,
          feeRecipient: feeRecipient.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const userSharesAccount = getSharesAta(sharesMint, payer.publicKey);
      const depositLamports = 1 * LAMPORTS_PER_SOL;

      await (program.methods as any)
        .depositSol(new BN(depositLamports), new BN(0))
        .accountsStrict({
          user: payer.publicKey,
          vault,
          wsolMint: WSOL_MINT,
          wsolVault,
          sharesMint,
          userSharesAccount,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: feeConfig, isWritable: false, isSigner: false },
        ])
        .rpc();

      const sharesAfter = await getAccount(connection, userSharesAccount, undefined, TOKEN_2022_PROGRAM_ID);
      const expectedFee = Math.ceil((depositLamports * 100) / 10000);
      const expectedNet = depositLamports - expectedFee;
      expect(Number(sharesAfter.amount)).to.equal(expectedNet);
    });

    it("global cap blocks deposits when exceeded", async () => {
      const vaultId = new BN(7102);
      const { vault, sharesMint, wsolVault } = await initializeVault(vaultId, { live: {} });

      const [capConfig] = getCapConfigPDA(vault);

      await (program.methods as any)
        .initializeCapConfig(new BN(1 * LAMPORTS_PER_SOL), new BN(0))
        .accountsStrict({
          authority: payer.publicKey,
          vault,
          capConfig,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const userSharesAccount = getSharesAta(sharesMint, payer.publicKey);

      try {
        await (program.methods as any)
          .depositSol(new BN(2 * LAMPORTS_PER_SOL), new BN(0))
          .accountsStrict({
            user: payer.publicKey,
            vault,
            wsolMint: WSOL_MINT,
            wsolVault,
            sharesMint,
            userSharesAccount,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .remainingAccounts([
            { pubkey: capConfig, isWritable: false, isSigner: false },
          ])
          .rpc();
        expect.fail("Deposit should have been blocked by GlobalCapExceeded");
      } catch (err: any) {
        expect(err.toString()).to.include("GlobalCapExceeded");
      }
    });

    it("whitelist mode with empty root rejects deposits (access)", async () => {
      const vaultId = new BN(7103);
      const { vault, sharesMint, wsolVault } = await initializeVault(vaultId, { live: {} });

      const [accessConfig] = getAccessConfigPDA(vault);

      await (program.methods as any)
        .initializeAccessConfig({ whitelist: {} }, new Array(32).fill(0))
        .accountsStrict({
          authority: payer.publicKey,
          vault,
          accessConfig,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const userSharesAccount = getSharesAta(sharesMint, payer.publicKey);

      try {
        await (program.methods as any)
          .depositSol(new BN(100_000_000), new BN(0))
          .accountsStrict({
            user: payer.publicKey,
            vault,
            wsolMint: WSOL_MINT,
            wsolVault,
            sharesMint,
            userSharesAccount,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .remainingAccounts([
            { pubkey: accessConfig, isWritable: false, isSigner: false },
          ])
          .rpc();
        expect.fail("Deposit should have been blocked by access control");
      } catch (err: any) {
        // If merkle root is not set, the underlying access module returns MerkleRootNotSet,
        // which is mapped to ModuleError::InvalidProof by svs-module-hooks.
        expect(err.toString()).to.include("InvalidProof");
      }
    });
  });
});
