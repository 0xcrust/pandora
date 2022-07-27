use anchor_lang::{prelude::*, solana_program::clock};
use anchor_spl::token::{CloseAccount, Mint, Token, TokenAccount, Transfer};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");


#[program]
pub mod crowdfunding_platform {

    use super::*;

    pub fn initialize_beneficence(_ctx: Context<Initialize>) -> Result<()> {
        msg!("lol");
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
            ErrorCode::RoundTargetExceedsCampaignLimits
        );

        let campaign = &mut ctx.accounts.campaign;
        campaign.fundstarter = ctx.accounts.fundstarter.key();
        campaign.vault = ctx.accounts.vault.key();
        campaign.description = description;
        campaign.target = target;
        campaign.balance = 0;
        campaign.token_mint = ctx.accounts.token_mint.key();
        campaign.status = CampaignStatus::CampaignActive.to_u8();
        campaign.total_rounds = number_of_funding_rounds;
        campaign.active_round = 1;
        campaign.can_start_next_round = false;
        campaign.active_round_address = ctx.accounts.round.key();
        campaign.cid = cid;
        campaign.bump = *ctx.bumps.get("campaign").unwrap();

        
        let round = &mut ctx.accounts.round;
        round.voting_account = Pubkey::default();
        round.round = 1;
        round.target = initial_target;
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
        let donating_wallet = ctx.accounts.donator_wallet.to_owned();
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

        donator_account.donator = ctx.accounts.donator.key();
        donator_account.amount = amount;
        donator_account.round = campaign.active_round;

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
        let voting_account = &mut ctx.accounts.voting_account;
        voting_account.continue_campaign_votes = 0;
        voting_account.terminate_campaign_votes = 0;
        voting_account.voters = 0;
        voting_account.voting_ended = false;

        let round = &mut ctx.accounts.round;
        round.voting_account = ctx.accounts.voting_account.key();
        Ok(())
    }

    pub fn vote(ctx: Context<VoteNextRound>, continue_campaign: bool) -> Result<()> {
        let campaign_status = CampaignStatus::from(ctx.accounts.campaign.status)?;
        let round_status = RoundStatus::from(ctx.accounts.round.status)?;
        require!(
            campaign_status == CampaignStatus::CampaignActive,
            ErrorCode::CampaignInactive
        );
        require!(
            round_status == RoundStatus::RoundTargetMet,
            ErrorCode::StartedVoteWithoutTargetMet
        );

        let voting_account = &mut ctx.accounts.voting_account;

        let donator = &mut ctx.accounts.voter_account;
        // let vote_weight = donator.amount;
        // continue_votes + terminate_votes == total amount donated by those who voted
        // Results in percentage = continue_votes / total amount of donators that voted

        // TODO: Let votes count as a factor of the amount a donator donates?
        match continue_campaign {
            true => {
                // voting_account.continue_votes = 
                    
                voting_account.continue_campaign_votes =
                    voting_account.continue_campaign_votes.checked_add(1).unwrap();
            }
            false => {
                voting_account.terminate_campaign_votes =
                    voting_account.terminate_campaign_votes.checked_add(1).unwrap();
            }
        }

        donator.has_voted = true;
        let campaign = &mut ctx.accounts.campaign;
        let round = &mut ctx.accounts.round;


        // TODO: Talk with ShreyJay and decide what qualifies as a valid vote(i.e how
        // do we know that the vote has been decided?)

        // Do we start voting immediately after the round ends or does the
        // fundstarter need to withdraw and offer proof of use first?
        if 1 == 1 {
            round.status = RoundStatus::RoundEnded.to_u8();
            voting_account.voting_ended = true;
            campaign.can_start_next_round = true;
            todo!("Close voting account and round account?");
        }

        Ok(())
    }

    pub fn start_next_round(ctx: Context<StartNextRound>, target: u64) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        require!(
            campaign.balance + target <= campaign.target,
            ErrorCode::RoundTargetExceedsCampaignLimits
        );

        campaign.active_round_address = ctx.accounts.round.key();
        campaign.active_round = campaign.active_round.checked_add(1).unwrap();
        campaign.can_start_next_round = false;

        let round = &mut ctx.accounts.round;
        round.round = campaign.active_round;
        round.target = target;
        round.balance = 0;
        round.donators = 0;
        round.status = RoundStatus::DonationsOpen.to_u8();
    
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        if CampaignStatus::from(campaign.status)? != CampaignStatus::CampaignEnded {
            campaign.status = CampaignStatus::CampaignEnded.to_u8();
        }

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

        campaign.balance = campaign.balance.checked_sub(amount_to_withdraw).unwrap();

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
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.staker.to_account_info(),
                }
            ),
            amount
        )?;

        stake_account.vault = ctx.accounts.vault.key();
        stake_account.staker = ctx.accounts.staker.key(); 
        stake_account.stake_time = clock.unix_timestamp;
        stake_account.deposit = amount;
        stake_account.reward = 0;

        Ok(())
    }

    pub fn unstake(ctx: Context<Stake>) -> Result<()> {
        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.staker_token_account.to_account_info(),
                    authority: ctx.accounts.stake_account.to_account_info(),
                }
            ),
            ctx.accounts.stake_account.deposit
        )?;
        
        let staker = ctx.accounts.staker.to_owned();
        let stake_account_bump = *ctx.bumps.get("stake_account").unwrap();
        let stake_account_seeds = &[
            "stake".as_bytes().as_ref(), 
            staker.key.as_ref(), 
            &[stake_account_bump]
        ];
        let signer = &[&stake_account_seeds[..]];

        anchor_spl::token::close_account(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                CloseAccount {
                    account: ctx.accounts.vault.to_account_info(),
                    destination: ctx.accounts.staker.to_account_info(),
                    authority: ctx.accounts.stake_account.to_account_info()
                }
            ).with_signer(signer)
        )?;

        Ok(())
    }
}


#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = Config::SIZE,
        seeds = ["beneficence".as_bytes().as_ref()],
        bump 
    )]
    config: Account<'info, Config>,

    #[account(mut)]
    authority: Signer<'info>,
    system_program: Program<'info, System>
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
        init, seeds = [b"round".as_ref(), campaign.key().as_ref(), &[1]],
        bump, payer = fundstarter, space = 8 + Round::SIZE
    )]
    round: Account<'info, Round>,
    
    token_mint: Account<'info, Mint>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

// Called by CPI?
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
        bump, payer = vault, space = 8 + RoundVote::SIZE,
        constraint = round.status == RoundStatus::RoundTargetMet.to_u8() @ErrorCode::StartedVoteWithoutTargetMet,
        constraint = round.voting_account == Pubkey::default() @ErrorCode::VotingAlreadyInitialized,
        constraint = campaign.active_round != campaign.total_rounds @ErrorCode::CantExceedTotalRound
    )]
    voting_account: Account<'info, RoundVote>,

    /// CHECK: We do not read or write to or from this account
    fundstarter: UncheckedAccount<'info>,
    #[account(mut)]
    round: Account<'info, Round>,
    #[account(mut)]
    vault: Account<'info, TokenAccount>,
    system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct StartNextRound<'info> {
    /// CHECK: We do not write or read from this account
    fundstarter: Signer<'info>,
    #[account(
        mut, seeds = [b"campaign".as_ref(), fundstarter.key().as_ref()],
        bump = campaign.bump, has_one = fundstarter, has_one = vault,
    )]
    campaign: Account<'info, Campaign>,
    #[account(mut)]
    vault: Account<'info, TokenAccount>,

    #[account(
        init,
        seeds = [b"round".as_ref(), campaign.key().as_ref(), &[campaign.active_round + 1]],
        bump, payer = vault, space = 8 + Round::SIZE,
        constraint = campaign.active_round != campaign.total_rounds @ErrorCode::CantExceedTotalRound,
        constraint = campaign.can_start_next_round == true @ErrorCode::PermissionToStartNextRoundMissing,
    )]
    round: Account<'info, Round>,
    system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct Donate<'info> {
    #[account(
        mut, seeds=[b"campaign".as_ref(), fundstarter.key().as_ref()], bump = campaign.bump,
        has_one = fundstarter, has_one = vault, constraint = campaign.active_round_address == round.key()
    )]
    campaign: Account<'info, Campaign>,
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
    /// CHECK: We check that it's the same fundstarter in our campaign state
    fundstarter: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = donator_wallet.mint == campaign.token_mint @ErrorCode::NonMatchingMints,
        constraint = donator_wallet.owner == donator.key() @ErrorCode::WalletNotOwnedByDonator,
    )]
    donator_wallet: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(
        mut, seeds=[b"campaign".as_ref(), fundstarter.key().as_ref()], bump = campaign.bump,
        has_one = fundstarter, has_one = token_mint, has_one = vault,
    )]
    campaign: Account<'info, Campaign>,

    round: Account<'info, Round>,

    #[account(mut)]
    vault: Account<'info, TokenAccount>,

    #[account(mut)]
    fundstarter: Signer<'info>,
    token_mint: Account<'info, Mint>,

    #[account(
        mut, constraint = wallet_to_withdraw_to.mint == token_mint.key() @ErrorCode::NonMatchingMints,
        constraint = wallet_to_withdraw_to.owner == fundstarter.key() @ErrorCode::WalletNotOwnedByDonator,
    )]
    wallet_to_withdraw_to: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}


#[derive(Accounts)]
pub struct InitDonatorVoting<'info> {
    #[account(mut,constraint = campaign.active_round_address == round.key())]
    campaign: Account<'info, Campaign>,

    #[account(constraint = round.status == RoundStatus::RoundTargetMet.to_u8())]
    round: Account<'info, Round>,
    #[account(mut)]
    donator: Signer<'info>,

    #[account(
        seeds = [b"donator".as_ref(), round.key().as_ref(), donator.key().as_ref()], 
        bump,
    )]
    donator_account: Account<'info, Donator>,

    #[account(
        init,
        payer = donator,
        space = NextRoundVoter::SIZE,
        seeds = ["vote".as_bytes().as_ref(), round.key().as_ref(), donator.key().as_ref()],
        bump
    )]
    donator_vote_account: Account<'info, NextRoundVoter>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitStakerVoting<'info> {
    #[account(mut,constraint = campaign.active_round_address == round.key())]
    campaign: Account<'info, Campaign>,

    #[account(constraint = round.status == RoundStatus::RoundTargetMet.to_u8())]
    round: Account<'info, Round>,
    #[account(mut)]
    staker: Signer<'info>,

    #[account(
        seeds = ["stake".as_bytes().as_ref(), staker.key().as_ref()],
        bump,
    )]
    stake_account: Account<'info, StakeAccount>,

    #[account(
        init,
        payer = staker,
        space = NextRoundVoter::SIZE,
        seeds = ["vote".as_bytes().as_ref(), round.key().as_ref(), staker.key().as_ref()],
        bump
    )]
    staker_vote_account: Account<'info, NextRoundVoter>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VoteNextRound<'info> {
    #[account(mut,constraint = campaign.active_round_address == round.key())]
    campaign: Account<'info, Campaign>,

    #[account(has_one = voting_account, constraint = round.status == RoundStatus::RoundTargetMet.to_u8())]
    round: Account<'info, Round>,

    #[account(
        mut,
        seeds = ["vote".as_bytes().as_ref(), round.key().as_ref(), voter.key().as_ref()],
        bump,
        constraint = voter_account.has_voted == false
    )]
    voter_account: Account<'info, NextRoundVoter>,

    voter: Signer<'info>,
 
    #[account(mut, constraint = voting_account.voting_ended == false @ErrorCode::VotingEnded)]
    voting_account: Account<'info, RoundVote>,
}


#[derive(Accounts)]
pub struct StakerInitModerator<'info> {
    #[account(constraint = campaign.status != CampaignStatus::CampaignEnded.to_u8())]
    campaign: Account<'info, Campaign>,

    #[account(
        init,
        payer = staker,
        space = Moderator::SIZE,
        seeds = ["moderator".as_bytes().as_ref(), campaign.key().as_ref(), staker.key().as_ref()],
        bump
    )]
    moderator_account: Account<'info, Moderator>,

    #[account(mut)]
    staker: Signer<'info>,

    #[account(
        seeds = ["stake".as_bytes().as_ref(), staker.key().as_ref()],
        bump,
    )]
    stake_account: Account<'info, StakeAccount>,
    system_program: Program<'info, System>
}

#[derive(Accounts)]
pub struct Moderate<'info> {
    #[account(mut)]
    campaign: Account<'info, Campaign>,
    #[account(
        mut,
        seeds = ["moderator".as_bytes().as_ref(), campaign.key().as_ref(), user.key().as_ref()],
        bump
    )]
    moderator_account: Account<'info, Moderator>,
    user: Signer<'info>,


}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(
        init,
        payer = staker,
        space = StakeAccount::SIZE,
        seeds = ["stake".as_bytes().as_ref(), staker.key().as_ref()],
        bump
    )]
    stake_account: Account<'info, StakeAccount>,

    #[account(
        mut,
        constraint = staker_token_account.owner == staker.key(),
        constraint = staker_token_account.mint == mint.key()
    )]
    staker_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = staker,
        seeds = ["stake-vault".as_bytes().as_ref(), staker.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = stake_account
    )]
    vault: Account<'info, TokenAccount>,

    #[account(mut)]
    staker: Signer<'info>,
    #[account(constraint = mint.key() == NATIVE_TOKEN_MINT.parse::<Pubkey>().unwrap())]
    mint: Account<'info, Mint>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>
}


#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    staker: Signer<'info>,

    #[account(
        mut,
        seeds = ["stake".as_bytes().as_ref(), staker.key().as_ref()],
        bump,
        has_one = staker,
        has_one = vault,
        close = staker,
    )]
    stake_account: Account<'info, StakeAccount>,

    #[account(mut)]
    vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = staker_token_account.mint == vault.mint,
        constraint = staker_token_account.owner == staker.key(),
    )]
    staker_token_account: Account<'info, TokenAccount>
}

const NATIVE_TOKEN_MINT: &str = "75Anj2RvhC5j8b2DniGoPSotBcst88fMt6Yo8xLATYJA";
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
    // The current balance of the user's campaign
    balance: u64,
    // Spl token mint: Could be SOL, USDT, BENE, etc
    token_mint: Pubkey,
    // Bump of campaign PDA
    bump: u8,
    // Campaign status
    status: u8,
    can_start_next_round: bool,
    // Number of milestones/rounds
    total_rounds: u8,
    // What round of rounds
    active_round: u8,
    // Current round account
    active_round_address: Pubkey,

    thumbs_up_votes: u8,
    thumbs_down_votes: u8,
    is_valid_campaign: bool,
}

const MAX_DESCRIPTION_SIZE: usize = 200;
impl Campaign {
    const SIZE: usize = (32 * 4) + (MAX_DESCRIPTION_SIZE + 4) + (8 * 3) + (1 * 3);
}

#[account]
pub struct Round {
    // Associated voting account
    voting_account: Pubkey,
    // What round of rounds
    round: u8,
    // target for this round
    target: u64,
    // amount raised this round
    balance: u64,
    // number of donators this round
    donators: u8,
    // Status
    status: u8,
}

impl Round {
    const SIZE: usize = 32 + 8 * 2 + 1 + 1;
}


#[account]
pub struct RoundVote {
    // Checked before starting next_round
    continue_campaign_votes: u8,
    terminate_campaign_votes: u8,
    voters: u8,
    voting_ended: bool,
}

impl RoundVote {
    const SIZE: usize = 8 + 8 + 1 + 8;
}

#[account]
pub struct Donator {
    donator: Pubkey,
    amount: u64,
    round: u8,
}

impl Donator {
    const SIZE: usize = 32 + 8 + 1;
}

#[account]
pub struct StakeAccount {
    vault: Pubkey,
    staker: Pubkey,
    stake_time: i64,
    deposit: u64,
    reward: u64,
}

impl StakeAccount {
    const SIZE: usize = 32 + 8 + 8;
}

#[account]
pub struct NextRoundVoter {
    voting_power: u64,
    has_voted: bool
}

impl NextRoundVoter {
    const SIZE: usize = 8 + 1;
}

#[account]
pub struct Moderator {
    voting_power: u64,
    has_voted: bool
}

impl Moderator {
    const SIZE: usize = 8 + 1;
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
    nonce: u8,
    signer: Pubkey,
    admin: Pubkey,
    native_token_mint: Pubkey,
    donation_fee: u64,
    voting_period: i64,
    minimum_required_vote: u64,
    active_stakers: u8,
    staking_initialized: bool,   
}

impl Config {
    const SIZE: usize = 2000;
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
    PermissionToStartNextRoundMissing,
    #[msg("You can't exceed the predetermined number of funding rounds")]
    CantExceedTotalRound,
    #[msg("Wrong ATA mint")]
    NonMatchingMints,
    #[msg("Wallet not owned by signer")]
    WalletNotOwnedByDonator,
    #[msg("Signer doesn't have a donator account for this round")]
    InvalidDonator,
    #[msg("Attempt to make a duplicate vote by the same donator")]
    AlreadyVoted,
    #[msg("Setting this round target would exceed the target set for the entire campaign")]
    RoundTargetExceedsCampaignLimits,
    #[msg("This round is not accepting donations")]
    RoundClosedToDonations,
    #[msg("Can't vote till RoundTarget has been met")]
    StartedVoteWithoutTargetMet,
    #[msg("Voting account already exists for this round")]
    VotingAlreadyInitialized,
    #[msg("Voting period has ended")]
    VotingEnded,
}




// make people create voting accounts before they can vote. more expensive?
// 