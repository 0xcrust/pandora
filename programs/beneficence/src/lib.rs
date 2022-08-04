use anchor_lang::{prelude::*, solana_program::clock};
use anchor_spl::token::{CloseAccount, Mint, Token, TokenAccount, Transfer};

declare_id!("duWSjMfQ8HYiikV3N6kUfMdBHHN8BaNzv5KWcXaUawA");

const DAY_IN_SECONDS: u64 = 60 * 60 * 24;

#[program]
pub mod beneficence {

    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        
        config.admin = ctx.accounts.authority.key();
        config.native_token_mint = ctx.accounts.native_token_mint.key();
        config.donation_fee = 0;
        config.staking_initialized = false;
        config.active_stakers = 0;
        config.total_amount_staked = 0;
        config.round_voting_period_in_days = 1;
        config.minimum_required_vote_percentage = 30;
        config.donator_voting_rights = 60;
        config.staker_voting_rights = 40;
        config.staker_moderation_rights = 100;
        config.staking_pool = Pubkey::default();
        config.bump = *ctx.bumps.get("config").unwrap();

        Ok(())
    }

    pub fn start_campaign(
        ctx: Context<StartCampaign>,
        description: String,
        target: u64,
        number_of_funding_rounds: u8,
        initial_target: u64,
        cid: String,
    ) -> Result<()> {
        require!(target > 0, ErrorCode::InvalidTarget);
        require!(
            description.chars().count() <= MAX_DESCRIPTION_SIZE,
            ErrorCode::DescriptionTooLong
        );
        require!(
            initial_target <= target,
            ErrorCode::CantExceedCampaignTarget
        );

        let initial_round_target: u64;

        if number_of_funding_rounds == 1 {
            initial_round_target = target;
        } else {
            initial_round_target = initial_target;
        }

        let campaign = &mut ctx.accounts.campaign;
        campaign.fundstarter = ctx.accounts.fundstarter.key();
        campaign.vault = ctx.accounts.vault.key();
        campaign.description = description;
        campaign.target = target;
        campaign.cid = cid;
        campaign.balance = 0;
        campaign.token_mint = ctx.accounts.token_mint.key();
        campaign.status = CampaignStatus::CampaignActive.to_u8();
        campaign.can_start_next_round = true;
        campaign.total_rounds = number_of_funding_rounds;
        campaign.active_round = 1;
        campaign.active_round_address = ctx.accounts.round.key();
        campaign.is_valid_votes = 0;
        campaign.not_valid_votes = 0;
        campaign.moderator_votes = 0;
        campaign.is_valid_campaign = true;
        campaign.bump = *ctx.bumps.get("campaign").unwrap();

        let round = &mut ctx.accounts.round;
        round.round_votes = Pubkey::default();
        round.round = 1;
        round.target = initial_round_target;
        round.balance = 0;
        round.donators = 0;
        round.status = RoundStatus::DonationsOpen.to_u8();

        Ok(())
    }

    pub fn donate(ctx: Context<Donate>, amount: u64) -> Result<()> {
        let campaign_status = CampaignStatus::from(ctx.accounts.campaign.status)?;
        let round_status = RoundStatus::from(ctx.accounts.round.status)?;
        require!(
            campaign_status == CampaignStatus::CampaignActive,
            ErrorCode::CampaignInactive
        );
        require!(
            round_status == RoundStatus::DonationsOpen,
            ErrorCode::RoundClosedToDonations
        );

        let round = &mut ctx.accounts.round;
        let donator_account = &mut ctx.accounts.donator_account;
        let campaign = &mut ctx.accounts.campaign;
        let donating_wallet = ctx.accounts.donator_token_account.to_owned();
        let vault = &mut ctx.accounts.vault.to_owned();
        let donator = ctx.accounts.donator.to_owned();
        let token_program = ctx.accounts.token_program.to_owned();
        let donation_size = amount;

        let transfer_instruction = Transfer {
            from: donating_wallet.to_account_info(),
            to: vault.to_account_info(),
            authority: donator.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(token_program.to_account_info(), transfer_instruction);

        anchor_spl::token::transfer(cpi_ctx, donation_size)?;
        campaign.balance = campaign.balance.checked_add(donation_size).unwrap();

        round.balance = round.balance.checked_add(donation_size).unwrap();
        round.donators = round.donators.checked_add(1).unwrap();

        //donator_account.donator = ctx.accounts.donator.key();
        donator_account.amount = amount;
        donator_account.round = campaign.active_round;
        donator_account.bump = *ctx.bumps.get("donator_account").unwrap();

        //vault.reload()?;
        if round.balance >= round.target {
            msg!("round target met!");
            round.status = RoundStatus::RoundTargetMet.to_u8();
        }

        if campaign.balance >= campaign.target {
            msg!("Campaign target met!");
            campaign.status = CampaignStatus::CampaignTargetMet.to_u8();
        }

        Ok(())
    }

    pub fn initialize_voting(ctx: Context<InitializeVoting>) -> Result<()> {
        let clock = clock::Clock::get().unwrap();
        let round_votes = &mut ctx.accounts.round_votes;
        round_votes.continue_campaign = 0;
        round_votes.terminate_campaign = 0;
        round_votes.donators_voted = 0;
        round_votes.stakers_voted = 0;
        round_votes.start_time = clock.unix_timestamp;
        round_votes.voting_ended = false;

        let round = &mut ctx.accounts.round;
        round.round_votes = ctx.accounts.round_votes.key();
        Ok(())
    }

    pub fn vote(ctx: Context<VoteNextRound>, continue_campaign: bool) -> Result<()> {
        let campaign_status = ctx.accounts.campaign.status;
        let round_status = ctx.accounts.round.status;
        require!(
            campaign_status == CampaignStatus::CampaignActive.to_u8(),
            ErrorCode::CampaignInactive
        );
        require_eq!(round_status, RoundStatus::RoundTargetMet.to_u8());

        let round_votes = &mut ctx.accounts.round_votes;
        let voter = &mut ctx.accounts.voter_account;

        match continue_campaign {
            true => {
                round_votes.continue_campaign =
                    round_votes.continue_campaign
                    .checked_add(voter.voting_power.into())
                    .unwrap();
            }
            false => {
                round_votes.terminate_campaign =
                    round_votes.terminate_campaign
                    .checked_add(voter.voting_power.into())
                    .unwrap();
            }
        }

        match VoterType::from(voter.voter_type)? {
            VoterType::Donator => {
                round_votes.donators_voted = 
                    round_votes.donators_voted.checked_add(1).unwrap();
            },
            VoterType::Staker => {
                round_votes.stakers_voted = 
                    round_votes.stakers_voted.checked_add(1).unwrap();
            }
        }

        voter.has_voted = true;

        Ok(())
    }

    // Should be chained in the same tx as the instruction to start next round
    pub fn tally_votes(ctx: Context<TallyVotes>) -> Result<()> {
        let current_time = clock::Clock::get().unwrap().unix_timestamp;
        let voting_start_time = ctx.accounts.round_votes.start_time;
        let time_elapsed_in_seconds = current_time - voting_start_time;
        let voting_period_in_seconds = (ctx.accounts.config.round_voting_period_in_days as u64)
            .checked_mul(DAY_IN_SECONDS)
            .unwrap();

        require!(
            time_elapsed_in_seconds as u64 > voting_period_in_seconds,
            ErrorCode::VotingStillActive
        );

        let round_votes = &mut ctx.accounts.round_votes;
        let maximum_possible_voters = ctx.accounts.config.active_stakers
            .checked_add(ctx.accounts.round.donators as u64)
            .unwrap();
        let voters_this_round = round_votes.stakers_voted
            .checked_add(round_votes.donators_voted)
            .unwrap();
        let minimum_voters_required = (30 as u64)
            .checked_mul(maximum_possible_voters)
            .unwrap()
            .checked_div(100 as u64)
            .unwrap();

        let round = &mut ctx.accounts.round;
        let campaign = &mut ctx.accounts.campaign;

        if round_votes.terminate_campaign > round_votes.continue_campaign &&
             voters_this_round > minimum_voters_required 
        {
            campaign.can_start_next_round = false;
        } 
        
        round.status = RoundStatus::RoundEnded.to_u8();
        //round.status = 200;
        round_votes.voting_ended = true;
        Ok(())
    }


    pub fn start_next_round(ctx: Context<StartNextRound>, target: u64) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        let round_target: u64;

        if campaign.active_round + 1 == campaign.total_rounds {
            round_target = campaign.target
                .checked_sub(campaign.balance)
                .unwrap();
        } else {
            round_target = target;
        }

        require!(
            campaign.balance + round_target <= campaign.target,
            ErrorCode::CantExceedCampaignTarget
        );

        campaign.active_round_address = ctx.accounts.next_round.key();
        campaign.active_round = campaign.active_round.checked_add(1).unwrap();
        campaign.can_start_next_round = true;

        let round = &mut ctx.accounts.next_round;
        round.round_votes = Pubkey::default();
        round.round = campaign.active_round;
        round.target = round_target;
        round.balance = 0;
        round.donators = 0;
        round.status = RoundStatus::DonationsOpen.to_u8();
    
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;

        let fundstarter = ctx.accounts.fundstarter.to_owned();
        let funds_pot = &mut ctx.accounts.vault.to_owned();
        let destination_account = ctx.accounts.wallet_to_withdraw_to.to_owned();
        let token_program = ctx.accounts.token_program.to_owned();

        // We reload to get the amount of tokens in our pot and withdraw all of it
        funds_pot.reload()?;
        let amount_to_withdraw = funds_pot.amount;

        let transfer_instruction = Transfer {
            from: funds_pot.to_account_info(),
            to: destination_account.to_account_info(),
            authority: campaign.to_account_info(),
        };

        let state_seeds = &[
            b"campaign".as_ref(),
            fundstarter.key.as_ref(),
            &[campaign.bump],
        ];
        let signer = &[&state_seeds[..]];

        let cpi_ctx = CpiContext::new(token_program.to_account_info(), transfer_instruction)
            .with_signer(signer);
        anchor_spl::token::transfer(cpi_ctx, amount_to_withdraw)?;

        //campaign.balance = campaign.balance.checked_sub(amount_to_withdraw).unwrap();

        let should_close = {
            funds_pot.reload()?;
            funds_pot.amount == 0
        };

        if should_close {
            let close_instruction = CloseAccount {
                account: funds_pot.to_account_info(),
                destination: fundstarter.to_account_info(),
                authority: campaign.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(token_program.to_account_info(), close_instruction)
                .with_signer(signer);
            anchor_spl::token::close_account(cpi_ctx)?;
        }

        let current_round = campaign.active_round;
        let total_rounds = campaign.total_rounds;

        if current_round == total_rounds {
            campaign.status = CampaignStatus::CampaignEnded.to_u8();
        }

        Ok(())
    }

    pub fn initialize_staking(ctx: Context<InitializeStaking>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.staking_initialized = true;
        config.staking_pool = ctx.accounts.staking_pool.key();
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        let token_program = &ctx.accounts.token_program;
        let clock = clock::Clock::get().unwrap();
        let stake_account = &mut ctx.accounts.stake_account;

        anchor_spl::token::transfer(
            CpiContext::new(
                token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.staker_token_account.to_account_info(),
                    to: ctx.accounts.staking_pool.to_account_info(),
                    authority: ctx.accounts.staker.to_account_info(),
                }
            ),
            amount
        )?;

        //stake_account.staker = ctx.accounts.staker.key(); 
        stake_account.stake_time = clock.unix_timestamp;
        stake_account.deposit = amount;
        stake_account.reward = 0;

        let config = &mut ctx.accounts.config;
        config.active_stakers = config.active_stakers
            .checked_add(1).unwrap();
        config.total_amount_staked = config.total_amount_staked
            .checked_add(amount).unwrap();

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        let config_bump = ctx.accounts.config.bump;
        let config_seeds = &[
            "config".as_bytes().as_ref(),
            ctx.accounts.config.admin.as_ref(),
             &[config_bump]
        ];
        let signer = &[&config_seeds[..]];
        
        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.staking_pool.to_account_info(),
                    to: ctx.accounts.staker_token_account.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                }
            ).with_signer(signer),
            ctx.accounts.stake_account.deposit
        )?;

        let config = &mut ctx.accounts.config;
        config.active_stakers = config.active_stakers
            .checked_add(1).unwrap();
        config.total_amount_staked = config.total_amount_staked
            .checked_add(ctx.accounts.stake_account.deposit).unwrap();

        Ok(())
    }

    pub fn init_donator_voting(ctx: Context<DonatorVotingInit>) -> Result<()> {

        let total_donations = ctx.accounts.round.balance;
        let donation = ctx.accounts.donator_account.amount;
        let donator_voting_rights = ctx.accounts.config.donator_voting_rights;

        let voting_power = donation
            .checked_mul(donator_voting_rights as u64)
            .unwrap()
            .checked_div(total_donations)
            .unwrap();

        let voter_account = &mut ctx.accounts.voter_account;
        voter_account.voting_power = voting_power as u8;
        voter_account.has_voted = false;
        voter_account.voter_type = VoterType::Donator.to_u8();
        voter_account.bump = *ctx.bumps.get("voter_account").unwrap();

        Ok(())
    }

    pub fn init_staker_voting(ctx: Context<StakerVotingInit>) -> Result<()> {

        let staker_deposit = ctx.accounts.stake_account.deposit;
        let total_amount_staked = ctx.accounts.config.total_amount_staked;
        let staker_voting_rights = ctx.accounts.config.staker_voting_rights;

        let voting_power = staker_deposit
            .checked_mul(staker_voting_rights as u64)
            .unwrap()
            .checked_div(total_amount_staked)
            .unwrap();

        let voter_account = &mut ctx.accounts.voter_account;
        voter_account.voting_power = voting_power as u8;
        voter_account.has_voted = false;
        voter_account.voter_type = VoterType::Staker.to_u8();
        voter_account.bump = *ctx.bumps.get("voter_account").unwrap();

        Ok(())
    }

    pub fn init_staker_moderation(ctx: Context<StakerModerationInit>) -> Result<()> {
        let staker_deposit = ctx.accounts.stake_account.deposit;
        let total_amount_staked = ctx.accounts.config.total_amount_staked;
        let staker_moderation_rights = ctx.accounts.config.staker_moderation_rights;

        let voting_power = staker_deposit
            .checked_mul(staker_moderation_rights as u64)
            .unwrap()
            .checked_div(total_amount_staked)
            .unwrap();

        let moderator_account = &mut ctx.accounts.moderator_account;
        moderator_account.voting_power = voting_power as u8;
        moderator_account.has_voted = false;
        moderator_account.moderator_type = ModeratorType::Staker.to_u8();
        
        Ok(())
    }

    pub fn moderate(ctx: Context<Moderate>, thumbs_up: bool ) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        let moderator_account = &mut ctx.accounts.moderator_account;

        match thumbs_up {
            true => {
                campaign.is_valid_votes = campaign.is_valid_votes
                    .checked_add(moderator_account.voting_power)
                    .unwrap();
            }
            false => {
                campaign.not_valid_votes = campaign.not_valid_votes
                    .checked_add(moderator_account.voting_power)
                    .unwrap();
            }
        }

        moderator_account.has_voted = true;
        campaign.moderator_votes = campaign.moderator_votes
            .checked_add(1).unwrap();

        // at least 30% of all moderators(for now stakers only) must vote
        // for a campaign to be stopped successfully.
        let minimum_votes_required = (30 as u64)
            .checked_mul(ctx.accounts.config.active_stakers)
            .unwrap()
            .checked_div(100 as u64)
            .unwrap();
        let voters_this_round = campaign.moderator_votes;

        if campaign.not_valid_votes > campaign.is_valid_votes && voters_this_round > minimum_votes_required {
            campaign.is_valid_campaign = false;
        } else {
            campaign.is_valid_campaign = true;
        }

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Config::SIZE,
        seeds = ["config".as_bytes().as_ref(), ],
        bump 
    )]
    config: Box<Account<'info, Config>>,

    #[account(mut)]
    authority: Signer<'info>,
    native_token_mint: Account<'info, Mint>,
    system_program: Program<'info, System>,
    rent: Sysvar<'info, Rent>
}


#[derive(Accounts)]
pub struct StartCampaign<'info> {
    #[account(mut)]
    fundstarter: Signer<'info>,
    #[account(
        init, seeds = [b"campaign".as_ref(), fundstarter.key().as_ref()],
        bump, payer = fundstarter, space = 8 + Campaign::SIZE
    )]
    campaign: Account<'info, Campaign>,

    #[account(
        init, seeds = [b"vault".as_ref(), campaign.key().as_ref()], bump,
        payer = fundstarter, token::mint = token_mint, token::authority = campaign
    )]
    vault: Account<'info, TokenAccount>,
    #[account(
        init, seeds = [b"round".as_ref(), campaign.key().as_ref(), (1 as u64).to_le_bytes().as_ref()],
        bump, payer = fundstarter, space = 8 + Round::SIZE
    )]
    round: Account<'info, Round>,
    
    token_mint: Account<'info, Mint>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}


// Chained with the withdraw endpoint in a transaction? 
#[derive(Accounts)]
pub struct InitializeVoting<'info> {
    #[account(
        mut, seeds = [b"campaign".as_ref(), fundstarter.key().as_ref()],
        bump = campaign.bump, has_one = fundstarter, has_one = vault,
        constraint = campaign.active_round_address == round.key()
    )]
    campaign: Account<'info, Campaign>,
    #[account(
        init, seeds = [b"voting".as_ref(), round.key().as_ref()],
        bump, payer = fundstarter, space = 8 + RoundVote::SIZE,
        constraint = round.status == RoundStatus::RoundTargetMet.to_u8(),
        constraint = round.round_votes == Pubkey::default(),
        constraint = campaign.active_round != campaign.total_rounds @ErrorCode::CantExceedMaxRound
    )]
    round_votes: Account<'info, RoundVote>,

    #[account(mut)]
    fundstarter: Signer<'info>,
    #[account(mut)]
    round: Account<'info, Round>,
    vault: Account<'info, TokenAccount>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TallyVotes<'info> {
    #[account(
        seeds = ["config".as_bytes().as_ref()],
        bump = config.bump
    )]
    config: Account<'info, Config>,

    #[account(mut,constraint = campaign.active_round_address == round.key())]
    campaign: Account<'info, Campaign>,

    #[account(mut, has_one = round_votes, constraint = round.status == RoundStatus::RoundTargetMet.to_u8())]
    round: Account<'info, Round>,
 
    #[account(mut, constraint = round_votes.voting_ended == false @ErrorCode::VotingEnded)]
    round_votes: Account<'info, RoundVote>,
}


#[derive(Accounts)]
pub struct StartNextRound<'info> {
    #[account(mut)]
    fundstarter: Signer<'info>,
    #[account(
        mut, seeds = [b"campaign".as_ref(), fundstarter.key().as_ref()],
        bump = campaign.bump, has_one = fundstarter,
        constraint = campaign.active_round != campaign.total_rounds @ErrorCode::CantExceedMaxRound,
        constraint = campaign.can_start_next_round == true @ErrorCode::CantStartNextRound,
    )]
    campaign: Account<'info, Campaign>,

    #[account(
        constraint = campaign.active_round_address == current_round.key(),
        constraint = current_round.status == RoundStatus::RoundEnded.to_u8()@ ErrorCode::RoundHasntEnded
    )]
    current_round: Account<'info, Round>,

    #[account(
        init,
        seeds = [b"round".as_ref(), campaign.key().as_ref(), ((campaign.active_round + 1) as u64).to_le_bytes().as_ref()],
        bump, payer = fundstarter, space = 8 + Round::SIZE,
    )]
    next_round: Account<'info, Round>,
    system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct Donate<'info> {
    #[account(
        mut, has_one = vault, 
        constraint = campaign.active_round_address == round.key()
    )]
    campaign: Account<'info, Campaign>,

    #[account(mut)]
    vault: Account<'info, TokenAccount>,

    #[account(mut)]
    round: Account<'info, Round>,

    #[account(
        init, space = 8 + Donator::SIZE, payer = donator,
        seeds = [b"donator".as_ref(), round.key().as_ref(), donator.key().as_ref()],
        bump
    )]
    donator_account: Account<'info, Donator>,

    #[account(mut)]
    donator: Signer<'info>,

    #[account(
        mut,
        constraint = donator_token_account.mint == campaign.token_mint,
        constraint = donator_token_account.owner == donator.key()
    )]
    donator_token_account: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(
        mut, seeds=[b"campaign".as_ref(), fundstarter.key().as_ref()], bump = campaign.bump,
        has_one = fundstarter, has_one = vault,
        constraint = campaign.is_valid_campaign == true
    )]
    campaign: Account<'info, Campaign>,

    round: Account<'info, Round>,

    #[account(mut)]
    vault: Account<'info, TokenAccount>,

    #[account(mut)]
    fundstarter: Signer<'info>,

    #[account(
        mut, constraint = wallet_to_withdraw_to.mint == campaign.token_mint, 
        constraint = wallet_to_withdraw_to.owner == fundstarter.key()
    )]
    wallet_to_withdraw_to: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}


#[derive(Accounts)]
pub struct DonatorVotingInit<'info> {
    #[account(
        seeds = ["config".as_bytes().as_ref()],
        bump = config.bump
    )]
    config: Account<'info, Config>,

    #[account(mut,constraint = campaign.active_round_address == round.key())]
    campaign: Account<'info, Campaign>,

    #[account(constraint = round.status == RoundStatus::RoundTargetMet.to_u8())]
    round: Account<'info, Round>,
    #[account(mut)]
    donator: Signer<'info>,

    #[account(
        seeds = [b"donator".as_ref(), round.key().as_ref(), donator.key().as_ref()], 
        bump = donator_account.bump,
    )]
    donator_account: Account<'info, Donator>,

    #[account(
        init,
        payer = donator,
        space = 8 + NextRoundVoter::SIZE,
        seeds = ["voter".as_bytes().as_ref(), round.key().as_ref(), donator.key().as_ref()],
        bump
    )]
    voter_account: Account<'info, NextRoundVoter>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StakerVotingInit<'info> {
    #[account(
        seeds = ["config".as_bytes().as_ref()],
        bump = config.bump
    )]
    config: Account<'info, Config>,

    #[account(mut,constraint = campaign.active_round_address == round.key())]
    campaign: Account<'info, Campaign>,

    #[account(constraint = round.status == RoundStatus::RoundTargetMet.to_u8())]
    round: Account<'info, Round>,
    #[account(mut)]
    staker: Signer<'info>,

    #[account(
        seeds = ["staker".as_bytes().as_ref(), staker.key().as_ref()],
        bump,
    )]
    stake_account: Account<'info, StakeAccount>,

    #[account(
        init,
        payer = staker,
        space = 8 + NextRoundVoter::SIZE,
        seeds = ["voter".as_bytes().as_ref(), round.key().as_ref(), staker.key().as_ref()],
        bump
    )]
    voter_account: Account<'info, NextRoundVoter>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VoteNextRound<'info> {
    #[account(mut,constraint = campaign.active_round_address == round.key())]
    campaign: Account<'info, Campaign>,

    #[account(has_one = round_votes, constraint = round.status == RoundStatus::RoundTargetMet.to_u8())]
    round: Account<'info, Round>,

    #[account(
        mut,
        seeds = ["voter".as_bytes().as_ref(), round.key().as_ref(), voter.key().as_ref()],
        bump = voter_account.bump,
        constraint = voter_account.has_voted == false
    )]
    voter_account: Account<'info, NextRoundVoter>,

    voter: Signer<'info>,
 
    #[account(mut, constraint = round_votes.voting_ended == false @ErrorCode::VotingEnded)]
    round_votes: Account<'info, RoundVote>,
}

#[derive(Accounts)]
pub struct StakerModerationInit<'info> {
    #[account(
        seeds = ["config".as_bytes().as_ref()],
        bump = config.bump
    )]
    config: Account<'info, Config>,

    #[account(constraint = campaign.status != CampaignStatus::CampaignEnded.to_u8())]
    campaign: Account<'info, Campaign>,

    #[account(
        init,
        payer = staker,
        space = 8 + Moderator::SIZE,
        seeds = ["moderator".as_bytes().as_ref(), campaign.key().as_ref(), staker.key().as_ref()],
        bump
    )]
    moderator_account: Account<'info, Moderator>,

    #[account(mut)]
    staker: Signer<'info>,

    #[account(
        seeds = ["staker".as_bytes().as_ref(), staker.key().as_ref()],
        bump,
    )]
    stake_account: Account<'info, StakeAccount>,
    system_program: Program<'info, System>
}

#[derive(Accounts)]
pub struct Moderate<'info> {
    #[account(
        seeds = ["config".as_bytes().as_ref()],
        bump = config.bump
    )]
    config: Account<'info, Config>,
    #[account(
        mut,
        constraint = campaign.status != CampaignStatus::CampaignEnded.to_u8()
    )]
    campaign: Account<'info, Campaign>,
    #[account(
        mut,
        seeds = ["moderator".as_bytes().as_ref(), campaign.key().as_ref(), moderator.key().as_ref()],
        bump,
        constraint = moderator_account.has_voted == false
    )]
    moderator_account: Account<'info, Moderator>,
    moderator: Signer<'info>,
}


#[derive(Accounts)]
pub struct InitializeStaking<'info> {
    #[account(
        mut,
        seeds = ["config".as_bytes().as_ref()],
        bump = config.bump,
        has_one = admin,
        has_one = native_token_mint,
        constraint = config.staking_initialized == false
    )]
    config: Account<'info, Config>,
    #[account(mut)]
    admin: Signer<'info>,
    #[account(
        init,
        payer = admin,
        seeds = ["staking-pool".as_bytes().as_ref(), config.key().as_ref()],
        bump,
        token::mint = native_token_mint,
        token::authority = config,
    )]
    staking_pool: Account<'info, TokenAccount>,
    native_token_mint: Account<'info, Mint>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(
        mut,
        seeds = ["config".as_bytes().as_ref()],
        bump = config.bump,
        has_one = staking_pool,
        constraint = config.staking_initialized == true,
        constraint = config.native_token_mint == mint.key(),
    )]
    config: Account<'info, Config>,

    #[account(
        init,
        payer = staker,
        space = 8 + StakeAccount::SIZE,
        seeds = ["staker".as_bytes().as_ref(), staker.key().as_ref()],
        bump
    )]
    stake_account: Account<'info, StakeAccount>,

    #[account(
        mut,
        constraint = staker_token_account.owner == staker.key(),
        constraint = staker_token_account.mint == mint.key()
    )]
    staker_token_account: Account<'info, TokenAccount>,

    //#[account(
    //    seeds = ["staking-pool".as_bytes().as_ref(), config.key().as_ref()],
    //    bump,
    //)]
    #[account(mut)]
    staking_pool: Account<'info, TokenAccount>,

    #[account(mut)]
    staker: Signer<'info>,
    mint: Account<'info, Mint>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>
}


#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(
        mut,
        seeds = ["config".as_bytes().as_ref()],
        bump = config.bump,
        has_one = staking_pool
    )]
    config: Account<'info, Config>,

    #[account(mut)]
    staker: Signer<'info>,

    #[account(
        mut,
        seeds = ["staker".as_bytes().as_ref(), staker.key().as_ref()],
        bump,
        //has_one = staker,
        close = staker,
    )]
    stake_account: Account<'info, StakeAccount>,

    //#[account(
    //    seeds = ["staking-pool".as_bytes().as_ref(), config.key().as_ref()],
    //    bump,
    //)]
    #[account(mut)]
    staking_pool: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = staker_token_account.mint == staking_pool.mint,
        constraint = staker_token_account.owner == staker.key(),
    )]
    staker_token_account: Account<'info, TokenAccount>,
    token_program: Program<'info, Token>
}


#[account]
pub struct Campaign {
    // The user starting a campaign
    fundstarter: Pubkey,
    // The wallet that'll receive the tokens
    vault: Pubkey,
    // The campaign description, should not take > 200 bytes of storage
    description: String,
    // The amount of tokens the user is trying to raise
    target: u64,
    // Arweave cid
    cid: String,
    // Amount raised so far this campaign
    balance: u64,
    // Spl token mint: Could be SOL, USDT, BENE, etc
    token_mint: Pubkey,
    // Campaign status
    status: u8,
    can_start_next_round: bool,
    // Number of milestones/rounds
    total_rounds: u8,
    // What round of rounds
    active_round: u8,
    // Current round account
    active_round_address: Pubkey,

    is_valid_votes: u8,
    not_valid_votes: u8,
    // the number of moderators that have exercised their voting right so far
    moderator_votes: u64,
    is_valid_campaign: bool,
    // Bump of campaign PDA
    bump: u8,
}

const MAX_DESCRIPTION_SIZE: usize = 200;
const CID_SIZE: usize = 50;
const PUBKEY_SIZE: usize = 32;
const U8_SIZE: usize = 1;
const U64_SIZE: usize = 8;
const BOOL_SIZE: usize = 1;

impl Campaign {
    const SIZE: usize = (PUBKEY_SIZE * 4) + (U8_SIZE * 6)
        +(U64_SIZE * 3)        
        +(4 + MAX_DESCRIPTION_SIZE)
        +(4 + CID_SIZE)
        +(BOOL_SIZE * 2);
}

#[account]
pub struct Round {
    // Associated voting account
    round_votes: Pubkey,
    // What round of rounds
    round: u8,
    // target for this round
    target: u64,
    // amount raised this round
    balance: u64,
    // number of donators this round
    donators: u64,
    // Status
    status: u8,
}

impl Round {
    const SIZE: usize = 32 + 1 + 8 + 8 + 8 + 1;
}


#[account]
pub struct RoundVote {
    // continue campaign votes
    continue_campaign: u8,
    // terminate campaign votes
    terminate_campaign: u8,
    
    // number of donators that voted
    donators_voted: u64,
    // number of stakers that voted
    stakers_voted: u64,

    start_time: i64,
    voting_ended: bool,
}

impl RoundVote {
    const SIZE: usize = 1 + 1 + 8 + 8 + 8 + 1;
}

#[account]
pub struct Donator {
    amount: u64,
    round: u8,
    bump: u8
}

impl Donator {
    const SIZE: usize = 8 + 1 + 1;
}

#[account]
pub struct StakeAccount {
    stake_time: i64,
    deposit: u64,
    reward: u64,
}

impl StakeAccount {
    const SIZE: usize = 8 + 8 + 8;
}

#[account]
pub struct NextRoundVoter {
    voting_power: u8,
    has_voted: bool,
    voter_type: u8,
    bump: u8
}

impl NextRoundVoter {
    const SIZE: usize = 1 + 1 + 1 + 1;
}

#[derive(Clone, Copy, PartialEq, AnchorDeserialize, AnchorSerialize)]
pub enum VoterType {
    Donator,
    Staker
}

impl VoterType {
    fn from(val: u8) -> std::result::Result<VoterType, Error> {
        match val {
            1 => Ok(VoterType::Donator),
            2 => Ok(VoterType::Staker),
            invalid_number => {
                msg!("Invalid voter type: {}", invalid_number);
                Err(ErrorCode::InvalidVoterType.into())
            }
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            VoterType::Donator => 1,
            VoterType::Staker => 2,
        }
    }
}

#[account]
pub struct Moderator {
    voting_power: u8,
    has_voted: bool,
    moderator_type: u8,
}

impl Moderator {
    const SIZE: usize = 1 + 1 + 1;
}

#[derive(Clone, Copy, PartialEq, AnchorDeserialize, AnchorSerialize)]
pub enum ModeratorType {
    Staker
}

impl ModeratorType {
    fn from(val: u8) -> std::result::Result<ModeratorType, Error> {
        match val {
            1 => Ok(ModeratorType::Staker),
            invalid_number => {
                msg!("Invalid moderator type: {}", invalid_number);
                Err(ErrorCode::InvalidModeratorType.into())
            }
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            ModeratorType::Staker => 1,
        }
    }
}


#[derive(Clone, Copy, PartialEq, AnchorDeserialize, AnchorSerialize)]
pub enum RoundStatus {
    DonationsOpen,
    RoundTargetMet,
    RoundEnded,
}

impl RoundStatus {
    fn from(val: u8) -> std::result::Result<RoundStatus, Error> {
        match val {
            1 => Ok(RoundStatus::DonationsOpen),
            2 => Ok(RoundStatus::RoundTargetMet),
            3 => Ok(RoundStatus::RoundEnded),
            invalid_number => {
                msg!("Invalid state: {}", invalid_number);
                Err(ErrorCode::InvalidStatus.into())
            }
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            RoundStatus::DonationsOpen => 1,
            RoundStatus::RoundTargetMet => 2,
            RoundStatus::RoundEnded => 3,
        }
    }
}

#[derive(Clone, Copy, PartialEq, AnchorDeserialize, AnchorSerialize)]
pub enum CampaignStatus {
    CampaignActive,
    CampaignTargetMet,
    CampaignEnded,
}

impl CampaignStatus {
    fn from(val: u8) -> std::result::Result<CampaignStatus, Error> {
        match val {
            1 => Ok(CampaignStatus::CampaignActive),
            2 => Ok(CampaignStatus::CampaignTargetMet),
            3 => Ok(CampaignStatus::CampaignEnded),
            invalid_number => {
                msg!("Invalid state: {}", invalid_number);
                Err(ErrorCode::InvalidStatus.into())
            }
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            CampaignStatus::CampaignActive => 1,
            CampaignStatus::CampaignTargetMet => 2,
            CampaignStatus::CampaignEnded => 3,
        }
    }
}

#[account]
#[derive(Default)]
pub struct Config {
    admin: Pubkey,
    native_token_mint: Pubkey,
    donation_fee: u64,
    staking_initialized: bool,   
    active_stakers: u64,
    total_amount_staked: u64,
    round_voting_period_in_days: u8,
    minimum_required_vote_percentage: u8,
    donator_voting_rights: u8,
    staker_voting_rights: u8,
    staker_moderation_rights: u8,
    staking_pool: Pubkey,
    bump: u8,
}

impl Config {
    const SIZE: usize = (3 * PUBKEY_SIZE) + (3 * U64_SIZE)
        +(6 * U8_SIZE) + (1 * BOOL_SIZE);
    //const SIZE: usize = 2000;
}

#[error_code]
pub enum ErrorCode {
    #[msg("Target set for campaign must be greater than 0")]
    InvalidTarget,
    #[msg("Maxed out space for campaign description")]
    DescriptionTooLong,
    #[msg("Invalid campaign status")]
    InvalidStatus,
    #[msg("You tried to donate to an inactive campaign")]
    CampaignInactive,
    #[msg("No go ahead to start the next round")]
    CantStartNextRound,
    #[msg("You can't exceed the max number of funding rounds")]
    CantExceedMaxRound,
    #[msg("Can't exceed campaign target")]
    CantExceedCampaignTarget,
    #[msg("This round is not accepting donations")]
    RoundClosedToDonations,
    #[msg("Voting period has ended")]
    VotingEnded,
    #[msg("Invalid voter type")]
    InvalidVoterType,
    #[msg("Invalid moderator type ")]
    InvalidModeratorType,
    #[msg("Can't tally votes while voting is still active")]
    VotingStillActive,
    #[msg("Can't start next round until we tally votes and end the current round")]
    RoundHasntEnded,
}



// Validate all bump seeds