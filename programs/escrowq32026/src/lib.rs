pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("EBXQ5AMy7zqoFV6oXzqvR9KftoTFhuDhsAX9dNQhZnrZ");

// Two parties — a maker and a taker — can swap tokens without trusting each other or a third party.
// The maker deposits token A into a program-controlled vault and specifies how much of token B they want in return.
// Any taker who holds token B can complete the swap atomically. If no taker appears, the maker can reclaim their tokens at any time.

// Maker deposits token A  →  vault (PDA-owned)
//                                       ↓  taker sends token B to maker      (before expiration)
//                                       ↓  vault releases token A to taker
//                                       ↓  escrow + vault accounts closed, rent returned
//                                       ↓  …or after expiration: maker gets refunded from the vault

#[program]
pub mod escrowq32026 {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn make(
        ctx: Context<Make>,
        seed: u64,
        deposit: u64,
        receive: u64,
        expiration: i64,
    ) -> Result<()> {
        ctx.accounts
            .init_escrow(seed, receive, &ctx.bumps, expiration)?;
        ctx.accounts.deposit(deposit)
    }

    //take instruction
    //TODO:

    #[instruction(discriminator = 2)]
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        ctx.accounts.refund_and_close_vault()
    }

    #[instruction(discriminator = 3)]
    pub fn take(ctx: Context<Take>) -> Result<()> {
        ctx.accounts.transfer()?;
        ctx.accounts.transfer_and_close()?;
        
        Ok(())
    }

    #[instruction(discriminator = 4)]
    pub fn update(ctx: Context<Update>, receive: Option<u64>, expiration: i64) -> Result<()> {
        ctx.accounts.update(receive, expiration)
    }
}
