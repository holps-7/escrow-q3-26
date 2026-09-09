use anchor_lang::prelude::*;

use crate::{constants::ESCROW_SEED, error::EscrowError, state::Escrow};

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    #[account(
        mut,
        has_one = maker,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,
}

impl<'info> Update<'info> {
    //Update the escrow expiration time 
    pub fn update(&mut self, receive: Option<u64>, expiration: i64) -> Result<()> {
        require!(self.escrow.expiration > Clock::get()?.unix_timestamp, EscrowError::EscrowExpired);

        if let Some(receive) = receive {
            require!(receive > 0, EscrowError::InvalidReceiveAmount);
            self.escrow.receive = receive;
        }
        self.escrow.expiration = expiration;
        Ok(())
    }
}
