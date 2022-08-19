import * as anchor from "@project-serum/anchor";
import { Program, Wallet, AnchorError } from "@project-serum/anchor";
import { Pandora } from "../target/types/pandora";
import * as spl from "@solana/spl-token";
import {
  createTokenMint,
  airdrop,
  getConfigPDA,
  getCampaignPDA,
  getVaultPDA,
  getRoundPDA,
  createAssociatedTokenAccount,
  mintTokensToWallet,
  getDonatorAccountPDA,
  getRoundVotesPDA,
  getStakeAccountPDA,
  getStakingPoolPDA,
  getVoterAccountPDA,
  getModeratorAccountPDA
} from "./utils";
import { assert, config, expect } from "chai";
import { createAssociatedTokenAccountInstruction } from "@solana/spl-token";

describe("beneficence", async () => {
  // Configure the client to use the local cluster.
  //const provider = anchor.AnchorProvider.env();
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Pandora as Program<Pandora>;

  const admin = anchor.web3.Keypair.generate();
  let [configPDA, configBump] = await getConfigPDA(program);
  let nativeMintAddress: anchor.web3.PublicKey;
  let nativeMintAuthority: anchor.web3.Keypair;

  it("Initializes application state!", async () => {
   [nativeMintAddress, nativeMintAuthority] = await createTokenMint(provider.connection, admin);
  
   // Airdrop 2 sol to admin
   await airdrop(provider.connection, admin, 4);
   let [configPDA, configBump] = await getConfigPDA(program);

   console.log("Initializing...");
   await program.methods
     .initialize()
     .accounts({
       config: configPDA,
       authority: admin.publicKey,
       nativeTokenMint: nativeMintAddress,
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

  it("Simulates a campaign", async () => {
    // Initialize and start staking
    let [stakingPoolPDA, stakingPoolBump] = await getStakingPoolPDA(program, configPDA);

    let configState = await program.account.config.fetch(configPDA);
    assert.equal(configState.stakingInitialized, false);
    assert.ok(configState.stakingPool.equals(anchor.web3.PublicKey.default));

    await program.methods
      .initializeStaking()
      .accounts({
        config: configPDA,
        admin: admin.publicKey,
        stakingPool: stakingPoolPDA,
        nativeTokenMint: nativeMintAddress
      })
      .signers([admin])
      .rpc();

    configState = await program.account.config.fetch(configPDA);
    assert.equal(configState.stakingInitialized, true);
    assert.ok(configState.stakingPool.equals(stakingPoolPDA));
    

    async function stake (amount) : Promise<[anchor.web3.Keypair, anchor.web3.PublicKey]> {
      let staker = anchor.web3.Keypair.generate();
      await airdrop(program.provider.connection, staker, 1);
      let stakerWallet = await createAssociatedTokenAccount(program, staker, nativeMintAddress);
      await mintTokensToWallet(stakerWallet, amount + 10, staker, nativeMintAddress, 
        nativeMintAuthority, program);
      let [stakeAccount, _] = await getStakeAccountPDA(program, staker.publicKey);
      
      await program.methods
        .stake(new anchor.BN(amount))
        .accounts({
          config: configPDA,
          stakeAccount: stakeAccount,
          stakerTokenAccount: stakerWallet,
          stakingPool: stakingPoolPDA,
          staker: staker.publicKey,
          mint: nativeMintAddress
        })
        .signers([staker])
        .rpc();
      
      let stakeAccountState = await program.account.stakeAccount.fetch(stakeAccount);
      assert.equal(stakeAccountState.deposit.toNumber(), amount);
      assert.equal(stakeAccountState.reward.toNumber(), 0);
      
      return [staker, stakeAccount];
    }

    let [staker1, stakeAccount1] = await stake(40);
    let [staker2, stakeAccount2] = await stake(30);
    let [staker3, stakeAccount3] = await stake(20);
    let [staker4, stakeAccount4] = await stake(50);
    let [staker5, stakeAccount5] = await stake(60);

    configState = await program.account.config.fetch(configPDA);
    assert.equal(configState.activeStakers.toNumber(), 5);
    assert.equal(configState.totalAmountStaked.toNumber(), 200);

    // User starts a campaign
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
      await mintTokensToWallet(donatorWallet, amount + 10, donator, nativeMintAddress, 
        nativeMintAuthority, program);

      let [donatorAccountPDA, _] = await getDonatorAccountPDA(program, round, donator.publicKey);

      let initialCampaignState = await program.account.campaign.fetch(campaign);
      let initialCampaignBalance = initialCampaignState.balance.toNumber();

      let initialRoundState = await program.account.round.fetch(round);
      let initialRoundBalance = initialRoundState.balance.toNumber();
      let initialRoundDonators = initialRoundState.donators.toNumber();

      await program.methods
        .donate(new anchor.BN(amount))
        .accounts({
          campaign: campaign,
          vault: vault,
          round: round,
          donatorAccount: donatorAccountPDA,
          donator: donator.publicKey,
          donatorTokenAccount: donatorWallet
        })
        .signers([donator])
        .rpc();

      console.log(`Donated ${amount} tokens to round`);

      let updatedCampaignState = await program.account.campaign.fetch(campaign);
      let updatedCampaignBalance = updatedCampaignState.balance.toNumber();

      let updatedRoundState = await program.account.round.fetch(round);
      let updatedRoundBalance = updatedRoundState.balance.toNumber();
      let updatedRoundDonators = updatedRoundState.donators.toNumber();

      assert.equal(updatedCampaignBalance, initialCampaignBalance + amount);
      assert.equal(updatedRoundBalance, initialRoundBalance + amount);
      assert.equal(updatedRoundDonators, initialRoundDonators + 1);

      let donatorAccountState1 = await program.account.donator.fetch(donatorAccountPDA);
      assert.equal(donatorAccountState1.amount.toNumber(), amount);
      assert.equal(donatorAccountState1.round, updatedCampaignState.activeRound);
      assert.equal(donatorAccountState1.round, updatedRoundState.round);

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
    assert.equal(campaignState.status, 1);
    round1State = await program.account.round.fetch(round1PDA);
    assert.equal(round1State.status, 1);


    // try to initialize voting before round is complete
    let [round1VotesAccount, round1VotesBump] = await getRoundVotesPDA(program, round1PDA);
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
    assert.equal(campaignState.status, 1);
    round1State = await program.account.round.fetch(round1PDA);
    assert.equal(round1State.status, 2);


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


    // Stakers ballot for the ability to cast votes
    async function stakerBallot(user: anchor.web3.Keypair, userStakeAccount: anchor.web3.PublicKey,
       round: anchor.web3.PublicKey, campaign: anchor.web3.PublicKey)
      : Promise<anchor.web3.PublicKey> {

      let staker = user;
      let stakeAccount = userStakeAccount;

      let [voterAccountPDA, voterAccountBump] = await getVoterAccountPDA(program, round, staker.publicKey);
      await program.methods
        .initStakerVoting()
        .accounts({
          config: configPDA,
          campaign: campaign,
          round: round,
          staker: staker.publicKey,
          stakeAccount: stakeAccount,
          voterAccount: voterAccountPDA
        })
        .signers([staker])
        .rpc();
    
      let configState = await program.account.config.fetch(configPDA);
      let voterAccountState = await program.account.nextRoundVoter.fetch(voterAccountPDA);
      let stakerAccountState = await program.account.stakeAccount.fetch(stakeAccount);
      
      let deposit = stakerAccountState.deposit.toNumber();
      let totalStake = configState.totalAmountStaked.toNumber();
      let stakerRights = configState.stakerVotingRights;
      let expected_voting_power = Math.trunc((deposit / totalStake) * stakerRights);

      assert.equal(voterAccountState.votingPower, expected_voting_power);
      assert.equal(voterAccountState.hasVoted, false);
      assert.equal(voterAccountState.voterType, 2);

      return voterAccountPDA;
    }

    let staker1VoteAccount = await stakerBallot(staker1, stakeAccount1, round1PDA, campaignPDA);
    let staker2VoteAccount = await stakerBallot(staker2, stakeAccount2, round1PDA, campaignPDA);
    let staker3VoteAccount = await stakerBallot(staker3, stakeAccount3, round1PDA, campaignPDA);
    let staker4VoteAccount = await stakerBallot(staker4, stakeAccount4, round1PDA, campaignPDA);
    let staker5VoteAccount = await stakerBallot(staker5, stakeAccount5, round1PDA, campaignPDA);

    // Donators ballot for the ability to cast votes
    async function donatorBallot(donator: anchor.web3.Keypair, donatorAccountPDA: anchor.web3.PublicKey,
       round: anchor.web3.PublicKey, campaign: anchor.web3.PublicKey, config: anchor.web3.PublicKey)
      : Promise<anchor.web3.PublicKey> {

      let [voterAccountPDA, voterAccountBump] = await getVoterAccountPDA(program, round, donator.publicKey);
      await program.methods
        .initDonatorVoting()
        .accounts({
          config: config,
          campaign: campaign,
          round: round,
          donator: donator.publicKey,
          donatorAccount: donatorAccountPDA,
          voterAccount: voterAccountPDA
        })
        .signers([donator])
        .rpc();

      let totalDonations = await (await program.account.round.fetch(round)).balance.toNumber();
      let donation = await (await program.account.donator.fetch(donatorAccountPDA)).amount.toNumber();
      let votingRights = await (await program.account.config.fetch(config)).donatorVotingRights;

      let expected_voting_power = Math.trunc((donation / totalDonations) * votingRights);

      let voterAccountState = await program.account.nextRoundVoter.fetch(voterAccountPDA);
      assert.equal(voterAccountState.votingPower, expected_voting_power);
      assert.equal(voterAccountState.hasVoted, false);
      assert.equal(voterAccountState.voterType, 1);

      //return [voterAccountPDA, voterAccountBump];
      return voterAccountPDA;
    }

    let donator1VoteAccount = await donatorBallot(donator1, donatorAccount1, round1PDA, campaignPDA, configPDA);
    let donator2VoteAccount = await donatorBallot(donator2, donatorAccount2, round1PDA, campaignPDA, configPDA);

    // Vote
    async function vote(user: anchor.web3.Keypair, userVoterAccount: anchor.web3.PublicKey, 
      round: anchor.web3.PublicKey, roundVoteAccount: anchor.web3.PublicKey, choice: boolean) {
      let voter = user;
      let voterAccountPDA = userVoterAccount;

      let roundVoteState = await program.account.roundVote.fetch(roundVoteAccount);
      let continueVotes = roundVoteState.continueCampaign;
      let terminateVotes = roundVoteState.terminateCampaign;
      let donatorsVoted = roundVoteState.donatorsVoted.toNumber();
      let stakersVoted = roundVoteState.stakersVoted.toNumber();

  
      await program.methods 
        .vote(choice)
        .accounts({
          campaign: campaignPDA,
          round: round,
          voterAccount: voterAccountPDA,
          voter: voter.publicKey,
          roundVotes: roundVoteAccount
        })
        .signers([voter])
        .rpc();

      roundVoteState = await program.account.roundVote.fetch(roundVoteAccount);
      let updatedContinueVotes = roundVoteState.continueCampaign;
      let updatedTerminateVotes = roundVoteState.terminateCampaign;
      let updatedDonatorsVoted = roundVoteState.donatorsVoted.toNumber();
      let updatedStakersVoted = roundVoteState.stakersVoted.toNumber();

      let voterAccountState = await program.account.nextRoundVoter.fetch(voterAccountPDA);
      let votingPower = voterAccountState.votingPower;
      assert.equal(voterAccountState.hasVoted, true);

      if(choice == true) {
        assert.equal(updatedContinueVotes, continueVotes + votingPower);
        assert.equal(updatedTerminateVotes, terminateVotes);
      } else {
        assert.equal(updatedTerminateVotes, terminateVotes + votingPower);
        assert.equal(updatedContinueVotes, continueVotes);
      }

      if(voterAccountState.voterType == 1) {
        assert.equal(updatedDonatorsVoted, donatorsVoted + 1);
        assert.equal(updatedStakersVoted, stakersVoted);
      } else if (voterAccountState.voterType == 2) {
        assert.equal(updatedStakersVoted, stakersVoted + 1);
        assert.equal(updatedDonatorsVoted, donatorsVoted);
      }
    }

    await vote(staker1, staker1VoteAccount, round1PDA, round1VotesAccount, true);
    await vote(staker2, staker2VoteAccount, round1PDA, round1VotesAccount, true);
    await vote(staker3, staker3VoteAccount, round1PDA, round1VotesAccount, true);
    await vote(staker4, staker4VoteAccount, round1PDA, round1VotesAccount, true);
    await vote(staker5, staker5VoteAccount, round1PDA, round1VotesAccount, false);

    await vote(donator1, donator1VoteAccount, round1PDA, round1VotesAccount, true);
    await vote(donator2, donator2VoteAccount, round1PDA,  round1VotesAccount, true);

    let voteAccountState = await program.account.roundVote.fetch(round1VotesAccount);
    console.log("Continue campaign votes?: ", voteAccountState.continueCampaign);
    console.log("Terminate campaign votes?: ", voteAccountState.terminateCampaign);
    console.log(`${voteAccountState.donatorsVoted} donators voted this round`);
    console.log(`${voteAccountState.stakersVoted} stakers voted this round`);

    
    // Try to tally votes and start next round(This will fail because there's no good
    // way to fake 24 hours passing by, but it works in the actual application as long
    // as a whole day has passed).
    let [round2PDA, round2Bump] = await getRoundPDA(program, campaignPDA, 2);

    let tx1 = await program.methods
      .tallyVotes()
      .accounts({
        config: configPDA,
        campaign: campaignPDA,
        round: round1PDA,
        roundVotes: round1VotesAccount
      })
      .signers([])
      .instruction();

    let tx2 = await program.methods
      .startNextRound(new anchor.BN(450))
      .accounts({
        fundstarter: user.publicKey,
        campaign: campaignPDA,
        currentRound: round1PDA,
        nextRound: round2PDA,
      })
      .signers([user])
      .instruction();

    let transaction = new anchor.web3.Transaction();
    await transaction.add(tx1);
    await transaction.add(tx2);


    try {
      const signature = await anchor.web3.sendAndConfirmTransaction(
        provider.connection,
        transaction,
        [user]
      );

      campaignState = await program.account.campaign.fetch(campaignPDA);
      round1State = await program.account.round.fetch(round1PDA);
      voteAccountState = await program.account.roundVote.fetch(round1VotesAccount);

      assert.equal(campaignState.canStartNextRound, true);
      assert.equal(round1State.status, 3); 
      assert.equal(voteAccountState.votingEnded, true);
    } catch(_err) {
      console.log("yup, I expected this to fail lol!, the code it tries to run is still correct though");
    }

    // Start a new campaign with only one round this time to test
    // withdrawal endpoint and verify that it works as expected
    user = anchor.web3.Keypair.generate();
    await airdrop(provider.connection, user, 2);

    [campaignPDA, campaignBump] = await getCampaignPDA(program, user.publicKey);
    [vaultPDA, vaultBump] = await getVaultPDA(program, campaignPDA);
    [round1PDA, roundBump] = await getRoundPDA(program, campaignPDA, 1);
    
    console.log("Starting campaign...");
    expected_description = "Help me pay my medical bills";
    expected_target = 200;
    expected_number_of_rounds = 1;
    expected_initial_target = 150;
    expected_cid = "4tY5KFLJ290154892LLJLDJJ99488422";

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

    round1State = await program.account.round.fetch(round1PDA);
    assert.equal(round1State.target.toNumber(), expected_target);

    async function createModAccount(user: anchor.web3.Keypair, userStakeAccount: anchor.web3.PublicKey,
       campaign: anchor.web3.PublicKey, config: anchor.web3.PublicKey)
      : Promise<anchor.web3.PublicKey> {

      let staker = user;
      let stakeAccount = userStakeAccount;

      let [modAccountPDA, modAccountBump] = await getModeratorAccountPDA(program, campaign, staker.publicKey);
      await program.methods
        .initStakerModeration()
        .accounts({
          config: config,
          campaign: campaign,
          moderatorAccount: modAccountPDA,
          staker: staker.publicKey,
          stakeAccount: stakeAccount,
        })
        .signers([staker])
        .rpc();
    
      let configState = await program.account.config.fetch(config);
      let moderatorState = await program.account.moderator.fetch(modAccountPDA);
      let stakerAccountState = await program.account.stakeAccount.fetch(stakeAccount);
      
      let deposit = stakerAccountState.deposit.toNumber();
      let totalStake = configState.totalAmountStaked.toNumber();
      let stakerRights = configState.stakerModerationRights;
      let expected_voting_power = Math.trunc((deposit / totalStake) * stakerRights);

      assert.equal(moderatorState.votingPower, expected_voting_power);
      assert.equal(moderatorState.hasVoted, false);
      assert.equal(moderatorState.moderatorType, 1);

      return modAccountPDA;
    }

    async function moderate(user: anchor.web3.Keypair, userModAccount: anchor.web3.PublicKey, 
      campaign: anchor.web3.PublicKey, config: anchor.web3.PublicKey, choice: boolean) {
      let moderator = user;
      let modAccountPDA = userModAccount;

      let campaignState = await program.account.campaign.fetch(campaign);
      let positiveVotes = campaignState.isValidVotes;
      let negativeVotes = campaignState.notValidVotes;
      let modsVoted = campaignState.moderatorVotes.toNumber();

      await program.methods 
        .moderate(choice)
        .accounts({
          config: config,
          campaign: campaign,
          moderatorAccount: modAccountPDA,
          moderator: moderator.publicKey
        })
        .signers([moderator])
        .rpc();

      campaignState = await program.account.campaign.fetch(campaign);
      let updatedPositiveVotes = campaignState.isValidVotes;
      let updatedNegativeVotes = campaignState.notValidVotes;
      let updatedModsVoted = campaignState.moderatorVotes.toNumber();

      let modAccountState = await program.account.moderator.fetch(modAccountPDA);
      let votingPower = modAccountState.votingPower;
      assert.equal(modAccountState.hasVoted, true);

      if(choice == true) {
        assert.equal(updatedPositiveVotes, positiveVotes + votingPower);
        assert.equal(updatedNegativeVotes, negativeVotes);
      } else {
        assert.equal(updatedNegativeVotes, negativeVotes + votingPower);
        assert.equal(updatedPositiveVotes, positiveVotes);
      }
      assert.equal(updatedModsVoted, modsVoted + 1);
    }

    // Create stakers moderation accounts
    let staker1ModAccount = await createModAccount(staker1, stakeAccount1, campaignPDA, configPDA);
    let staker2ModAccount = await createModAccount(staker2, stakeAccount2, campaignPDA, configPDA);
    let staker3ModAccount = await createModAccount(staker3, stakeAccount3, campaignPDA, configPDA);
    let staker4ModAccount = await createModAccount(staker4, stakeAccount4, campaignPDA, configPDA);
    let staker5ModAccount = await createModAccount(staker5, stakeAccount5, campaignPDA, configPDA);

    donator1 = anchor.web3.Keypair.generate();
    amount = 40;
    donatorAccount1 = await donate(
      amount,
      donator1,
      program,
      campaignPDA,
      round1PDA,
      vaultPDA
    );

    donator2 = anchor.web3.Keypair.generate();
    amount = 50;
    donatorAccount2 = await donate(
      amount,
      donator2,
      program,
      campaignPDA,
      round1PDA,
      vaultPDA
    );
    
    let donator3 = anchor.web3.Keypair.generate();
    amount = 60;
    let donatorAccount3 = await donate(
      amount,
      donator3,
      program,
      campaignPDA,
      round1PDA,
      vaultPDA
    );

    let donator4 = anchor.web3.Keypair.generate();
    amount = 30;
    let donatorAccount4 = await donate(
      amount,
      donator4,
      program,
      campaignPDA,
      round1PDA,
      vaultPDA
    );

    let donator5 = anchor.web3.Keypair.generate();
    amount = 30;
    let donatorAccount5 = await donate(
      amount,
      donator5,
      program,
      campaignPDA,
      round1PDA,
      vaultPDA
    );

    // START WITHDRAWAL
    let userTokenAccount = await spl.createAssociatedTokenAccount(
      provider.connection,
      user,
      nativeMintAddress,
      user.publicKey
    ); 

    // lets us get either of two different scenarios on each run,
    // either positive moderation results(where the user can withdraw),
    // or negative(where the user cann't withdraw)
    let chance = Math.floor(Math.random() * 2);
    console.log("Chance: ", chance);

    if(chance == 0) {
      // Positive moderation results, withdrawal should be allowed
      await moderate(staker5, staker5ModAccount, campaignPDA, configPDA, true);
      await moderate(staker2, staker2ModAccount, campaignPDA, configPDA, true);
      await moderate(staker4, staker4ModAccount, campaignPDA, configPDA, false);
      await moderate(staker3, staker3ModAccount, campaignPDA, configPDA, true);
      await moderate(staker1, staker1ModAccount, campaignPDA, configPDA, true);

      campaignState = await program.account.campaign.fetch(campaignPDA);
      let amountRaised = campaignState.balance.toNumber();

      await program.methods
        .withdraw()
        .accounts({
          campaign: campaignPDA,
          round: round1PDA,
          vault: vaultPDA,
          fundstarter: user.publicKey,
          walletToWithdrawTo: userTokenAccount,
        })
        .signers([user])
        .rpc();

      campaignState = await program.account.campaign.fetch(campaignPDA);
      console.log("Is_Valid_Campaign votes?: ", campaignState.isValidVotes);
      console.log("Not_Valid_Campaign votes?: ", campaignState.notValidVotes);
      console.log("Is_Valid_Campaign?: ", campaignState.isValidCampaign);
      assert.equal(campaignState.isValidCampaign, true);
      console.log(`${campaignState.moderatorVotes} mods voted this round`);

      let tokenAccountState = await provider.connection.getTokenAccountBalance(userTokenAccount);
      let userWithdrawalBalance = tokenAccountState.value.uiAmount;

      // Last round so status should be set to 'Ended'(3)
      assert.equal(campaignState.status, 3);
      console.log("Funds withdrawn: ", userWithdrawalBalance);
      assert.equal(userWithdrawalBalance, amountRaised);

    } else if(chance == 1) {
      // Negative moderation results, withdrawal should fail
      await moderate(staker5, staker5ModAccount, campaignPDA, configPDA, false);
      await moderate(staker2, staker2ModAccount, campaignPDA, configPDA, false);
      await moderate(staker4, staker4ModAccount, campaignPDA, configPDA, false);
      await moderate(staker1, staker1ModAccount, campaignPDA, configPDA, true);
      await moderate(staker3, staker3ModAccount, campaignPDA, configPDA, false);

      campaignState = await program.account.campaign.fetch(campaignPDA);
      console.log("Is_Valid_Campaign votes?: ", campaignState.isValidVotes);
      console.log("Not_Valid_Campaign votes?: ", campaignState.notValidVotes);
      console.log("Is_Valid_Campaign?: ", campaignState.isValidCampaign);
      assert.equal(campaignState.isValidCampaign, false);
      console.log(`${campaignState.moderatorVotes} mods voted this round`);

      try {
        await program.methods
          .withdraw()
          .accounts({
            campaign: campaignPDA,
            round: round1PDA,
            vault: vaultPDA,
            fundstarter: user.publicKey,
            walletToWithdrawTo: userTokenAccount
          })
          .signers([user])
          .rpc();
        chai.assert(false, "Should fail due to invalid campaign")
      } catch(_err) {
        expect(_err).to.be.instanceOf(AnchorError);
        const err: AnchorError = _err;
        expect(err.error.errorCode.number).to.equal(2003);
        expect(err.error.errorCode.code).to.equal("ConstraintRaw");
        expect(err.program.equals(program.programId)).is.true;
      }
      campaignState = await program.account.campaign.fetch(campaignPDA);
      console.log("Campaign status: ", campaignState.status);
      assert.equal(campaignState.status, 2);
    }
  });

});
