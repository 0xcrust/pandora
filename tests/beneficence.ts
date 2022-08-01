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
  getRoundPDA,
  createAssociatedTokenAccount,
  mintTokensToWallet,
  getDonatorAccountPDA
} from "./utils";
import { assert, config } from "chai";

describe("beneficence", () => {
  // Configure the client to use the local cluster.
  //const provider = anchor.AnchorProvider.env();
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);

  const program = anchor.workspace.Beneficence as Program<Beneficence>;

  const admin = anchor.web3.Keypair.generate();
  //const admin = (provider.wallet as Wallet).payer;

  let configPDA: anchor.web3.PublicKey;
  let configBump: number;
  let nativeMintAddress: anchor.web3.PublicKey;
  let nativeMintAuthority: anchor.web3.Keypair;

  it("Initializes application state!", async () => {
   [nativeMintAddress, nativeMintAuthority] = await createTokenMint(provider.connection, admin);
  
   // Airdrop 200 sol to admin
   await airdrop(provider.connection, admin, 2);
   let [configPDA, configBump] = await getConfigPDA(program, admin.publicKey);

   console.log("Initializing...");
   await program.methods
     .initialize()
     .accounts({
       config: configPDA,
       authority: admin.publicKey,
       nativeTokenMint: nativeMintAddress,
       //systemProgram: anchor.web3.SystemProgram.programId,
       //rent: anchor.web3.SYSVAR_RENT_PUBKEY
     })
     .signers([admin])
     .rpc();

   let configState = await program.account.config.fetch(configPDA);

   assert.ok(configState.admin.equals(admin.publicKey));
   assert.ok(configState.nativeTokenMint.equals(nativeMintAddress));
   assert.equal(configState.donationFee.toNumber(), 0);
   assert.equal(configState.stakingInitialized, false);
   assert.equal(configState.activeStakers.toNumber(), 0);
   assert.equal(configState.totalAmountStaked.toNumber(), 0);
   assert.equal(configState.roundVotingPeriodInDays, 1);
   assert.equal(configState.minimumRequiredVotePercentage, 30);
   assert.equal(configState.donatorVotingRights, 60);
   assert.equal(configState.stakerVotingRights, 40);
   assert.equal(configState.stakerModerationRights, 100);
   assert.ok(configState.stakingPool.equals(anchor.web3.PublicKey.default));
   assert.equal(configState.bump, configBump);
  });

  it("Starts and simulates a campaign", async () => {
    let user = anchor.web3.Keypair.generate();
    await airdrop(provider.connection, user, 2);

    let [campaignPDA, campaignBump] = await getCampaignPDA(program, user.publicKey);
    let [vaultPDA, vaultBump] = await getVaultPDA(program, campaignPDA);
    let [round1PDA, roundBump] = await getRoundPDA(program, campaignPDA, 1);
    
    console.log("Starting campaign...");
    let expected_description = "Fund my treatment";
    let expected_target = 550;
    let expected_number_of_rounds = 4;
    let expected_initial_target = 100;
    let expected_cid = "X45KFLJ2901994LLJLDJJ99488422";

    await program.methods
      .startCampaign(
        expected_description,
        new anchor.BN(expected_target),
        expected_number_of_rounds,
        new anchor.BN(expected_initial_target),
        expected_cid
      )
      .accounts({
        fundstarter: user.publicKey,
        campaign: campaignPDA,
        vault: vaultPDA,
        round: round1PDA,
        tokenMint: nativeMintAddress,
      })
      .signers([user])
      .rpc();
    
    let campaignState = await program.account.campaign.fetch(campaignPDA);
    assert.ok(campaignState.fundstarter.equals(user.publicKey));
    assert.ok(campaignState.vault.equals(vaultPDA));
    assert.equal(campaignState.description.toString(), expected_description);
    assert.equal(campaignState.target.toNumber(), expected_target);
    assert.equal(campaignState.cid.toString(), expected_cid);
    assert.equal(campaignState.balance.toNumber(), 0);
    assert.ok(campaignState.tokenMint.equals(nativeMintAddress));
    assert.equal(campaignState.status, 1);
    assert.equal(campaignState.canStartNextRound, true);
    assert.equal(campaignState.totalRounds, expected_number_of_rounds);
    assert.equal(campaignState.activeRound, 1);
    assert.ok(campaignState.activeRoundAddress.equals(round1PDA));
    assert.equal(campaignState.isValidVotes, 0);
    assert.equal(campaignState.notValidVotes, 0);
    assert.equal(campaignState.moderatorVotes.toNumber(), 0);
    assert.equal(campaignState.isValidCampaign, true);

    let roundState = await program.account.round.fetch(round1PDA);
    assert.ok(roundState.roundVotes.equals(anchor.web3.PublicKey.default));
    assert.equal(roundState.round, 1);
    assert.equal(roundState.target.toNumber(), expected_initial_target);
    assert.equal(roundState.balance.toNumber(), 0);
    assert.equal(roundState.donators.toNumber(), 0);
    assert.equal(roundState.status, 1);

/*
    async function donate(amount, donator: anchor.web3.Keypair, program, campaign, round, vault)
    : Promise<anchor.web3.PublicKey> {
      await airdrop(program.provider.connection, donator, 1);
      let donatorWallet = await createAssociatedTokenAccount(program, donator, nativeMintAddress);
      await mintTokensToWallet(donatorWallet, amount + 10, admin, nativeMintAddress, nativeMintAuthority, program);

      let [donatorAccountPDA, _] = await getDonatorAccountPDA(program, roundPDA, donator.publicKey);

      await program.methods
        .donate(new anchor.BN(amount))
        .accounts({
          campaign: campaign,
          vault: vault,
          round: round,
          donatorAccount: donatorAccountPDA,
          donator: donator.publicKey,
          donatorWallet: donatorWallet
        })
        .signers([donator])
        .rpc();

      console.log(`Donated ${amount} tokens to round`);
      return donatorAccountPDA;  
    }*/

    // donate 40 tokens
    let donator = anchor.web3.Keypair.generate();
    let amount = 30;
    console.log("aaa");
    /*
    let donator1Account = await donate(
      40,
      donator1,
      program,
      campaignPDA,
      roundPDA,
      vaultPDA
    );*/
    await airdrop(program.provider.connection, donator, 1);
      let donatorWallet = await createAssociatedTokenAccount(program, donator, nativeMintAddress);
      await mintTokensToWallet(donatorWallet, amount + 10, admin, nativeMintAddress, nativeMintAuthority, program);

      let [donatorAccountPDA, _] = await getDonatorAccountPDA(program, round1PDA, donator.publicKey);

      console.log("Starting....");
      await program.methods
        .donate(new anchor.BN(amount))
        .accounts({
          campaign: campaignPDA,
          vault: vaultPDA,
          round: round1PDA,
          donatorAccount: donatorAccountPDA,
          donator: donator.publicKey,
          donatorWallet: donatorWallet
        })
        .signers([donator])
        .rpc();

      console.log(`Donated ${amount} tokens to round`);
      //return donatorAccountPDA;
    console.log("bbb");
  });
});
