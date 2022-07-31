import * as anchor from "@project-serum/anchor";
import { Program, Wallet } from "@project-serum/anchor";
import { Beneficence } from "../target/types/beneficence";
import * as spl from "@solana/spl-token";
import {
  createTokenMint,
  airdrop,
  mintToAccount,
  getConfigPDA,
  getCampaignPDA,
  getVaultPDA,
  getRoundPDA
} from "./utils";
import { config } from "chai";

describe("beneficence", () => {
  // Configure the client to use the local cluster.
  //const provider = anchor.AnchorProvider.env();
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);

  const program = anchor.workspace.Beneficence as Program<Beneficence>;

  
  let configAddress: anchor.web3.PublicKey;
  let configBump: number;
  let nativeMintAddress: anchor.web3.PublicKey;
  let nativeMintAuthority: anchor.web3.Keypair;

  it("Initializes application state!", async () => {
    // Add your test here.
  
    
  const admin = anchor.web3.Keypair.generate();
  //const admin = (provider.wallet as Wallet).payer;

  [nativeMintAddress, nativeMintAuthority] = await createTokenMint(provider.connection);
  
  // Airdrop 200 sol to admin
  await airdrop(provider.connection, admin, 200);

  let [configAddress, configBump] = await getConfigPDA(program);

  console.log("Initializing...");
  await program.methods
    .initialize()
    .accounts({
      config: configAddress,
      authority: admin.publicKey,
      nativeTokenMint: nativeMintAddress,
      //systemProgram: anchor.web3.SystemProgram.programId,
      //rent: anchor.web3.SYSVAR_RENT_PUBKEY
    })
    .signers([admin])
    .rpc();

  let [campaignAddress, campaignBump] = await getCampaignPDA(program, admin.publicKey);
  let [vaultAddress, vaultBump] = await getVaultPDA(program, campaignAddress);
  let [roundAddress, roundBump] = await getRoundPDA(program, campaignAddress, 1);

  console.log("Starting campaign...");
  await program.methods
    .startCampaign("random, test test", new anchor.BN(20), 4, new anchor.BN(4), "xx124kfldll98")
    .accounts({
      fundstarter: admin.publicKey,
      campaign: campaignAddress,
      vault: vaultAddress,
      round: roundAddress,
      tokenMint: nativeMintAddress,
    })
    .signers([admin])
    .rpc();
  })

});
