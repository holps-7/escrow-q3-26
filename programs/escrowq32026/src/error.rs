use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("The escrow has expired")]
    EscrowExpired,
    #[msg("Deposit amount must be greater than zero")]
    InvalidDepositAmount,
    #[msg("Receive amount must be greater than zero")]
    InvalidReceiveAmount,
    #[msg("Expiration must be in the future and can only be extended")]
    InvalidExpiration,
}
