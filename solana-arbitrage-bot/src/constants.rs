use solana_program::pubkey::Pubkey;
use std::str::FromStr;

pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

// 获取SOL的mint地址
pub fn sol_mint() -> Pubkey {
    Pubkey::from_str(SOL_MINT).unwrap()
}