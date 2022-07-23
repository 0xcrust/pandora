use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod crowdfunding_platform {
    use super::*;

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


impl Campaign {
    const LEN: usize= (32 * 3) + (200 + 4) + (8 * 2) + (1 * 2); 
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