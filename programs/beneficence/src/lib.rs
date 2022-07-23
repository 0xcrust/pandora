use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount, Transfer, CloseAccount};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");


#[program]
pub mod crowdfunding_platform {
    use super::*;

    pub fn start_campaign(
        ctx: Context<StartCampaign>,
        description: String,
        target: u64,
        token_mint: Pubkey,
    ) -> Result<()> {
        require!(target > 0, CrowdFundError::InvalidTarget);
        require!(
            description.chars().count() <= MAX_DESCRIPTION_SIZE,
            CrowdFundError::DescriptionTooLong
        );

        let campaign_state = &mut ctx.accounts.campaign_state;

        campaign_state.fundstarter = ctx.accounts.fundstarter.key();
        campaign_state.vault = ctx.accounts.vault.key();
        campaign_state.description = description;
        campaign_state.target = target;
        campaign_state.balance = 0;
        campaign_state.token_mint = token_mint;
        campaign_state.status = Status::DonationsOpen.to_u8();
        campaign_state.voting_account = ctx.accounts.vote_account.key();
        campaign_state.bump = *ctx.bumps.get("campaign_state").unwrap();

        let voting_account = &mut ctx.accounts.vote_account;
        voting_account.authority = campaign_state.key();

        Ok(())
    }

    pub fn donate(ctx: Context<Donate>, amount: u64) -> Result<()> {
        let current_status = Status::from(ctx.accounts.campaign_state.status)?;
        require!(
            current_status == Status::DonationsOpen,
            CrowdFundError::ClosedToDonations
        );

        let donator_account = &mut ctx.accounts.donator_account;
        let campaign_state = &mut ctx.accounts.campaign_state;
        let donating_wallet = ctx.accounts.donator_wallet.to_owned();
        let vault = &mut ctx.accounts.vault.to_owned();
        let donator = ctx.accounts.donator.to_owned();
        let token_program = ctx.accounts.token_program.to_owned();
        let token_amount = amount;

        let transfer_instruction = Transfer {
            from: donating_wallet.to_account_info(),
            to: vault.to_account_info(),
            authority: donator.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(token_program.to_account_info(), transfer_instruction);

        anchor_spl::token::transfer(cpi_ctx, token_amount)?;
        campaign_state.balance = campaign_state.balance.checked_add(token_amount).unwrap();

        donator_account.address = ctx.accounts.donator.key();
        donator_account.amount = amount; 

        vault.reload()?;

        assert_eq!(campaign_state.balance, vault.amount);

        if campaign_state.balance >= campaign_state.target {
            msg!("campaign goal met!");
            campaign_state.status = Status::DonationsClosed.to_u8();
        }

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let campaign_state = &mut ctx.accounts.campaign_state;
        if Status::from(campaign_state.status)? != Status::CampaignEnded {
            campaign_state.status = Status::CampaignEnded.to_u8();
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
            authority: campaign_state.to_account_info(),
        };

        let state_seeds = &["campaign".as_bytes().as_ref(), fundstarter.key.as_ref(), &[campaign_state.bump]];
        let signer = &[&state_seeds[..]];

        let cpi_ctx = CpiContext::new(token_program.to_account_info(), transfer_instruction)
            .with_signer(signer);
        anchor_spl::token::transfer(cpi_ctx, amount_to_withdraw)?;

        campaign_state.balance = campaign_state
            .balance
            .checked_sub(amount_to_withdraw)
            .unwrap();

        let should_close = {
            funds_pot.reload()?;
            funds_pot.amount == 0
        };

        if should_close {
            let close_instruction = CloseAccount {
                account: funds_pot.to_account_info(),
                destination: fundstarter.to_account_info(),
                authority: campaign_state.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(token_program.to_account_info(), close_instruction)
                .with_signer(signer);
            anchor_spl::token::close_account(cpi_ctx)?;
        }

        Ok(())
    }
}


#[derive(Accounts)]
pub struct StartCampaign<'info> {
    #[account(mut)]
    fundstarter: Signer<'info>,
    #[account(
        init, seeds = [b"campaign".as_ref(), fundstarter.key().as_ref()],
        bump, payer = fundstarter, space = 8 + Campaign::SIZE
    )]
    campaign_state: Account<'info, Campaign>,

    #[account(
        init, seeds = [b"vault".as_ref(), fundstarter.key().as_ref()], bump,
        payer = fundstarter, token::mint = token_mint, token::authority = campaign_state
    )]
    vault: Account<'info, TokenAccount>,

    #[account(
        init, seeds = [b"voting".as_ref(), fundstarter.key().as_ref()],
        bump, payer = fundstarter, space = 8 + Vote::SIZE
    )]
    vote_account: Account<'info, Vote>,

    token_mint: Account<'info, Mint>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Donate<'info> {
    #[account(
        mut, seeds=[b"campaign".as_ref(), fundstarter.key().as_ref()], bump,
        has_one = fundstarter, has_one = token_mint
    )]
    campaign_state: Account<'info, Campaign>,

    #[account(
        mut,
        seeds=[b"vault".as_ref(), fundstarter.key().as_ref()],
        bump
    )]
    vault: Account<'info, TokenAccount>,

    #[account(
        init, space = 8 + Donator::SIZE, payer = donator,
        seeds = [b"donator".as_ref(), campaign_state.key().as_ref(), donator.key().as_ref()],
        bump
    )]
    donator_account: Account<'info, Donator>,

    #[account(mut)]
    donator: Signer<'info>,
    /// CHECK: we do not read or write to or from this account
    fundstarter: AccountInfo<'info>,
    token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint=donator_wallet.mint == token_mint.key(),
        constraint=donator_wallet.owner == donator.key()
    )]
    donator_wallet: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}


#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(
        mut, seeds=[b"campaign".as_ref(), fundstarter.key().as_ref()], bump,
        has_one = fundstarter, has_one = token_mint
    )]
    campaign_state: Account<'info, Campaign>,

    #[account(mut, seeds=[b"vault".as_ref(), fundstarter.key().as_ref()], bump)]
    vault: Account<'info, TokenAccount>,

    #[account(mut)]
    fundstarter: Signer<'info>,
    token_mint: Account<'info, Mint>,

    #[account(
        mut, constraint=wallet_to_withdraw_to.mint == token_mint.key(),
        constraint=wallet_to_withdraw_to.owner == fundstarter.key()
    )]
    wallet_to_withdraw_to: Account<'info, TokenAccount>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}


#[derive(Accounts)]
pub struct VoteContext<'info> {
    #[account(
        mut, 
        seeds = [b"donator".as_ref(), campaign_state.key().as_ref(), donator_account.address.as_ref()], 
        bump
    )]
    donator_account: Account<'info, Donator>,

    #[account(
        mut,
        seeds=[b"campaign".as_ref(), campaign_state.fundstarter.as_ref()],
        bump,
    )]
    campaign_state: Account<'info, Campaign>,

    #[account( 
        mut,
        seeds = [b"voting".as_ref(), campaign_state.fundstarter.as_ref()],
        bump,
        constraint = vote_account.authority == campaign_state.key()
    )]
    vote_account: Account<'info, Vote>
}


#[derive(Accounts)]
pub struct RefundContext<'info> {
    #[account(
        mut, 
        seeds = [b"donator".as_ref(), campaign_state.key().as_ref(), donator_account.address.as_ref()], 
        bump
    )]
    donator_account: Account<'info, Donator>,

    #[account(
        mut,
        seeds=[b"campaign".as_ref(), campaign_state.fundstarter.as_ref()],
        bump,
    )]
    campaign_state: Account<'info, Campaign>,

    #[account( 
        mut,
        seeds = [b"voting".as_ref(), campaign_state.fundstarter.as_ref()],
        bump,
        constraint = vote_account.authority == campaign_state.key()
    )]
    vote_account: Account<'info, Vote>
}

#[account]
pub struct Vote {
    authority: Pubkey,
}

impl Vote {
    const SIZE: usize = 32;
}

#[account]
pub struct Donator {
    address: Pubkey,
    amount: u64,
}

impl Donator {
    const SIZE: usize = 32 + 8;
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

    // The current balance of the user's campaign
    balance: u64,

    // Spl token mint: Could be SOL, USDT, BENE, etc
    token_mint: Pubkey,

    // Bump of campaign PDA
    bump: u8,

    // Campaign status
    status: u8,

    // TODO: set its authority to the smart contract. Implement as a pda 
    voting_account: Pubkey,
}

const MAX_DESCRIPTION_SIZE: usize = 200; 

impl Campaign {
    const SIZE: usize = (32 * 4) + (MAX_DESCRIPTION_SIZE + 4) + (8 * 2) + (1 * 2); 
}

#[derive(Clone, Copy, PartialEq, AnchorDeserialize, AnchorSerialize)]
pub enum Status {
    DonationsOpen,
    DonationsClosed,
    CampaignEnded,
}

impl Status {
    fn from(val: u8) -> std::result::Result<Status, CrowdFundError> {
        match val {
            1 => Ok(Status::DonationsOpen),
            2 => Ok(Status::DonationsClosed),
            3 => Ok(Status::CampaignEnded),
            invalid_number => {
                msg!("Invalid state: {}", invalid_number);
                Err(CrowdFundError::InvalidStatus)
            }
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            Status::DonationsOpen => 1,
            Status::DonationsClosed => 2,
            Status::CampaignEnded => 3,
        }
    }
}

#[error_code]
pub enum CrowdFundError {
    #[msg("Target set for campaign must be greater than 0")]
    InvalidTarget,
    #[msg("Maxed out space for campaign description")]
    DescriptionTooLong,
    #[msg("Invalid campaign status")]
    InvalidStatus,
    #[msg("You tried to donate to a closed campaign")]
    ClosedToDonations,
}



// ADD CAMPAIGN END DATE?

// ADD ACCOUNT TO KEEP TRACK OF TOP 5/10 DONATORS(THEY GET TO HAVE A SAY
// IN THE VOTING PROCESS OF WHETHER OR NOT A CAMPAIGN GOES TO THE NEXT MILESTONE)

// IMPLEMENT VOTING LOGIC
// ADD DATA TO TRACK MILESTONE 

// IMPLEMENT REFUND AND DONATOR VOTING ENDPOINT. CREATE ACCOUNT TO KEEP TRACK OF DONATIONS

