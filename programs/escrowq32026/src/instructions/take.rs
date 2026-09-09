use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{ESCROW_SEED, error::EscrowError, Escrow};

#[derive(Accounts)]
pub struct Take<'info> {
  #[account(mut)]
  pub taker: Signer<'info>,

  #[account(mut)]
  pub maker: SystemAccount<'info>,
  
  #[account(
    mint::token_program = token_program
  )]
  pub mint_a: InterfaceAccount<'info, Mint>,
  
  #[account(
    mint::token_program = token_program
  )]
  pub mint_b: InterfaceAccount<'info, Mint>,

  #[account(
    init_if_needed,
    payer = taker,
    associated_token::mint = mint_b,
    associated_token::authority = maker,
    associated_token::token_program = token_program
  )]
  pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,
  
  #[account(
    init_if_needed,
    payer = taker,
    associated_token::mint = mint_a,
    associated_token::authority = taker,
    associated_token::token_program = token_program
  )]
  pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,
  
  #[account(
    mut,
    associated_token::mint = mint_b,
    associated_token::authority = taker,
    associated_token::token_program = token_program
  )]
  pub taker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    mut,
    close = maker,
    has_one = maker,
    has_one = mint_a,
    has_one = mint_b,
    seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
    bump = escrow.bump
  )]
  pub escrow: Account<'info, Escrow>,

  #[account(
    mut,
    associated_token::mint = mint_a,
    associated_token::authority = escrow,
    associated_token::token_program = token_program
  )]
  pub vault: InterfaceAccount<'info, TokenAccount>,

  pub associated_token_program: Program<'info, AssociatedToken>,
  pub token_program: Interface<'info, TokenInterface>,
  pub system_program: Program<'info, System>,
}

impl<'info> Take<'info> {
  pub fn transfer(&mut self) -> Result<()> {
    require!(self.escrow.expiration > Clock::get()?.unix_timestamp, EscrowError::EscrowExpired);

    let transfer_accounts = TransferChecked {
      from: self.taker_ata_b.to_account_info(),
      mint: self.mint_b.to_account_info(),
      to: self.maker_ata_b.to_account_info(),
      authority: self.taker.to_account_info()
    };

    let cpi_context = CpiContext::new(
        self.token_program.key(),
        transfer_accounts
    );

    transfer_checked(cpi_context, self.escrow.receive, self.mint_b.decimals)?;

    Ok(())
  }

  pub fn transfer_and_close(&mut self) -> Result<()> {

    let signer_seeds: &[&[&[u8]]; 1] = &[&[
      ESCROW_SEED, 
      self.maker.key.as_ref(), 
      &self.escrow.seed.to_le_bytes()[..],
      &[self.escrow.bump]
    ]];

    let transfer_account = TransferChecked{
      from: self.vault.to_account_info(),
      mint: self.mint_a.to_account_info(),
      to: self.taker_ata_a.to_account_info(),
      authority: self.escrow.to_account_info()
    };

    let transfer_cpi_ctx = CpiContext::new_with_signer(self.token_program.key(), transfer_account, signer_seeds);

    transfer_checked(transfer_cpi_ctx, self.vault.amount, self.mint_a.decimals)?;

    let close_accounts = CloseAccount {
      account: self.vault.to_account_info(),
      destination: self.maker.to_account_info(),
      authority: self.escrow.to_account_info(),
    };

    let close_cpi_ctx = CpiContext::new_with_signer(self.token_program.key(), close_accounts, signer_seeds);

    close_account(close_cpi_ctx)?;

    Ok(())
  }
}