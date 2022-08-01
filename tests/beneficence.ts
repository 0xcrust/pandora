import * as anchor from "@project-serum/anchor";
import { Program, Wallet, AnchorError } from "@project-serum/anchor";
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
  getDonatorAccountPDA,
  getRoundVotesPDA
} from "./utils";
import { assert, config, expect } from "chai";

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

  it("Simulates staking and unstaking", async () => {

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
    let expected_number_of_rounds = 2;
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

    let round1State = await program.account.round.fetch(round1PDA);
    assert.ok(round1State.roundVotes.equals(anchor.web3.PublicKey.default));
    assert.equal(round1State.round, 1);
    assert.equal(round1State.target.toNumber(), expected_initial_target);
    assert.equal(round1State.balance.toNumber(), 0);
    assert.equal(round1State.donators.toNumber(), 0);
    assert.equal(round1State.status, 1);

    async function donate(amount, donator: anchor.web3.Keypair, program, campaign, round, vault)
    : Promise<anchor.web3.PublicKey> {
      await airdrop(program.provider.connection, donator, 1);
      let donatorWallet = await createAssociatedTokenAccount(program, donator, nativeMintAddress);
      await mintTokensToWallet(donatorWallet, amount + 10, admin, nativeMintAddress, nativeMintAuthority, program);

      let [donatorAccountPDA, _] = await getDonatorAccountPDA(program, round, donator.publicKey);

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
    }

    // donate 40 tokens
    let donator1 = anchor.web3.Keypair.generate();
    let amount = 40;
    let donatorAccount1 = await donate(
      amount,
      donator1,
      program,
      campaignPDA,
      round1PDA,
      vaultPDA
    );

    campaignState = await program.account.campaign.fetch(campaignPDA);
    assert.equal(campaignState.balance.toNumber(), amount);
    assert.equal(campaignState.status, 1);
    
    round1State = await program.account.round.fetch(round1PDA);
    assert.equal(round1State.balance.toNumber(), amount);
    assert.equal(round1State.donators.toNumber(), 1);
    assert.equal(round1State.status, 1);

    let donatorAccountState1 = await program.account.donator.fetch(donatorAccount1);
    assert.ok(donatorAccountState1.donator.equals(donator1.publicKey));
    assert.equal(donatorAccountState1.amount.toNumber(), amount);
    assert.equal(donatorAccountState1.round, campaignState.activeRound);
    assert.equal(donatorAccountState1.round, 1);

    // try to initialize voting before round is complete
    let [round1VotesAccount, _] = await getRoundVotesPDA(program, round1PDA);
    try {
      await program.methods
      .initializeVoting()
      .accounts({
        campaign: campaignPDA,
        roundVotes: round1VotesAccount,
        fundstarter: user.publicKey,
        round: round1PDA,
        vault: vaultPDA,
      })
      .signers([user])
      .rpc();
      chai.assert(false, "Should fail because round is still active");

    } catch (_err) {
      expect(_err).to.be.instanceOf(AnchorError);
      const err: AnchorError = _err;
      expect(err.error.errorCode.number).to.equal(2003);
      expect(err.error.errorCode.code).to.equal("ConstraintRaw");
      expect(err.program.equals(program.programId)).is.true;
    }

    // donate 70 tokens
    let donator2 = anchor.web3.Keypair.generate();
    amount = 70;
    let donatorAccount2 = await donate(
      amount,
      donator2,
      program,
      campaignPDA,
      round1PDA,
      vaultPDA
    );

    campaignState = await program.account.campaign.fetch(campaignPDA);
    assert.equal(campaignState.balance.toNumber(), 110);
    assert.equal(campaignState.status, 1);
    
    round1State = await program.account.round.fetch(round1PDA);
    assert.equal(round1State.balance.toNumber(), 110);
    assert.equal(round1State.donators.toNumber(), 2);
    assert.equal(round1State.status, 2);

    let donatorAccountState2 = await program.account.donator.fetch(donatorAccount2);
    assert.ok(donatorAccountState2.donator.equals(donator2.publicKey));
    assert.equal(donatorAccountState2.amount.toNumber(), amount);
    assert.equal(donatorAccountState2.round, campaignState.activeRound);
    assert.equal(donatorAccountState2.round, 1);

    // try to donate 20 tokens
    try {
      let failedDonator = anchor.web3.Keypair.generate();
      amount = 20;
      let failedDonatorAccount = await donate(
        amount,
        failedDonator,
        program,
        campaignPDA,
        round1PDA,
        vaultPDA
        );
      chai.assert(false,  "Should fail due to round target met");
    } catch (_err) {
      expect(_err).to.be.instanceOf(AnchorError);
      const err: AnchorError = _err;
      expect(err.error.errorCode.number).to.equal(6007);
      expect(err.error.errorCode.code).to.equal("RoundClosedToDonations");
      expect(err.program.equals(program.programId)).is.true;
    }
    
    campaignState = await program.account.campaign.fetch(campaignPDA);
    assert.equal(campaignState.balance.toNumber(), 110);
    assert.equal(campaignState.status, 1);
    
    round1State = await program.account.round.fetch(round1PDA);
    assert.equal(round1State.balance.toNumber(), 110);
    assert.equal(round1State.donators.toNumber(), 2);
    assert.equal(round1State.status, 2);

    // Initialize voting
    await program.methods
      .initializeVoting()
      .accounts({
        campaign: campaignPDA,
        roundVotes: round1VotesAccount,
        fundstarter: user.publicKey,
        round: round1PDA,
        vault: vaultPDA,
      })
      .signers([user])
      .rpc();

    campaignState = await program.account.campaign.fetch(campaignPDA);
  });
});
