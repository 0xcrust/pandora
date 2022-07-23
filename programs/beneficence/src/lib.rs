use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

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
        let campaign_state = &mut ctx.accounts.campaign_state;

        campaign_state.fundstarter = ctx.accounts.fundstarter.key();
        require!(
            description.chars().count() <= MAX_DESCRIPTION_LEN,
            CrowdFundError::DescriptionTooLong
        );
        campaign_state.vault = ctx.accounts.vault.key();
        campaign_state.description = description;
        campaign_state.target = target;
        campaign_state.balance = 0;
        campaign_state.token_mint = token_mint;
        campaign_state.status = Status::DonationsOpen.to_u8();
        campaign_state.bump = *ctx.bumps.get("campaign_state").unwrap();
        Ok(())
    }

}

#[derive(Accounts)]
pub struct StartCampaign<'info> {
    #[account(mut)]
    fundstarter: Signer<'info>,
    #[account(
        init, seeds = [b"campaign".as_ref(), fundstarter.key().as_ref()],
        bump, payer = fundstarter, space = 8 + Campaign::LEN
    )]
    campaign_state: Account<'info, Campaign>,

    #[account(
        init, seeds = [b"vault".as_ref(), fundstarter.key().as_ref()], bump,
        payer = fundstarter, token::mint = token_mint, token::authority = campaign_state
    )]
    vault: Account<'info, TokenAccount>,

    token_mint: Account<'info, Mint>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
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

    // The current balance of the user's fundraising account
    balance: u64,

    // The mint of the token the user is trying to raise
    token_mint: Pubkey,

    // Bump of campaign PDA
    bump: u8,

    // Status of fundraising campaign
    status: u8,
}

const MAX_DESCRIPTION_LEN: usize = 200; 

impl Campaign {
    const LEN: usize= (32 * 3) + (MAX_DESCRIPTION_LEN + 4) + (8 * 2) + (1 * 2); 
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