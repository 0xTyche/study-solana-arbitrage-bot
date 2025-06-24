use crate::constants::sol_mint;
use crate::dex::dlmm::{constants::dlmm_program_id, dlmm_info::DlmmInfo};
use crate::dex::pump::{pump_fee_wallet, pump_program_id, PumpAmmInfo};
use crate::dex::raydium::{
    get_tick_array_pubkeys, raydium_clmm_program_id, raydium_cp_program_id, raydium_program_id,
    PoolState, RaydiumAmmInfo, RaydiumCpAmmInfo,
};
use crate::dex::whirlpool::{
    constants::whirlpool_program_id, state::Whirlpool, update_tick_array_accounts_for_onchain,
};
use crate::pools::*;
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use spl_associated_token_account;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info};

// 初始化池子的数据
pub async fn initialize_pool_data(
    mint: &str,
    wallet_account: &str,
    raydium_pools: Option<&Vec<String>>,
    raydium_cp_pools: Option<&Vec<String>>,
    pump_pools: Option<&Vec<String>>,
    dlmm_pools: Option<&Vec<String>>,
    whirlpool_pools: Option<&Vec<String>>,
    raydium_clmm_pools: Option<&Vec<String>>,
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<MintPoolData> {
    info!("Initializing pool data for mint: {}", mint);

    let mut pool_data = MintPoolData::new(mint, wallet_account);
    info!("Pool data initialized for mint: {}", mint);

    if let Some(pools) = pump_pools {
        for pool_address in pools {
            let pump_pool_pubkey = Pubkey::from_str(pool_address)?;

            match rpc_client.get_account(&pump_pool_pubkey){
                Ok(account) => {
                    // 检查账户是否由 pumpfun 控制
                    if account.owner != pump_program_id(){
                        error!(
                            "Error: Pump pool account is not owned by the Pump program. Expected: {}, Actual: {}",
                            pump_program_id(), account.owner
                        );
                        return Err(anyhow::anyhow!(
                            "Pump pool account is not owned by the Pump program"
                        ));
                    }
                    // 解析 PumpAmmInfo 数据
                    match PumpAmmInfo::load_checked(&account.data)
                    {
                        Ok(amm_info) => {
                            // 判断池子哪边是sol 哪边是token，sol/token、token/sol、token/token
                            let (sol_vault, token_vault) = if sol_mint() == amm_info.base_mint{
                                (
                                    amm_info.pool_base_token_account,
                                    amm_info.pool_quote_token_account
                                )
                            }
                            else if sol_mint() == amm_info.quote_mint {
                                (
                                    amm_info.pool_quote_token_account,
                                    amm_info.pool_base_token_account,
                                )
                            } else {
                                (
                                    amm_info.pool_base_token_account,
                                    amm_info.pool_quote_token_account
                                )
                            };
                            // 构建手续费地址 Pump 官方的收款钱包 + quote token 的 ATA 地址。
                            let fee_token_wallet =
                                spl_associated_token_account::get_associated_token_address(
                                    &pump_fee_wallet(),
                                    &amm_info.quote_mint,
                                );
                            
                            // 将池子的数据添加到我的池子管理列表当中
                            pool_data.add_pump_pool(
                                pool_address,
                                &token_vault.to_string(),
                                &sol_vault.to_string(),
                                &fee_token_wallet.to_string(),
                            )?;
                            // 打印池子的数据
                            info!("Pump pool added: {}", pool_address);
                            info!("    Base mint: {}", amm_info.base_mint.to_string());
                            info!("    Quote mint: {}", amm_info.quote_mint.to_string());
                            info!("    Token vault: {}", token_vault.to_string());
                            info!("    Sol vault: {}", sol_vault.to_string());
                            info!("    Fee token wallet: {}", fee_token_wallet.to_string());
                            info!("    Initialized Pump pool: {}\n", pump_pool_pubkey);
                            }
                            Err(e) => {
                                error!(
                                    "Error parsing AmmInfo from Pump pool {}: {:?}",
                                    pump_pool_pubkey, e
                                );
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Error fetching Pump pool account {}: {:?}",
                            pump_pool_pubkey, e
                        );
                        return Err(anyhow::anyhow!("Error fetching Pump pool account"));
                    }
                }
            }
        }
    }

    if let Some(pool) = raydium_pools {
        for pool_address in pools {
            // 获取raydium池子的公钥
            let raydium_pool_pubkey = Pubkey::from_str(pool_address)?;
            // 获取raydium池子的数据
            match rpc_client.get_account(&raydium_pool_pubkey){
                Ok(account)=>{
                // 检查账户是否由 raydium 控制
                if account.owner != raydium_program_id(){
                    error!(
                        "Error: Raydium pool account is not owned by the Raydium program. Expected: {}, Actual: {}",
                        raydium_program_id(), account.owner
                    );
                    return Err(anyhow::anyhow!("Raydium pool account is not owned by the Raydium program"));
                }
                // 解析 RaydiumAmmInfo 数据
                match RaydiumAmmInfo::load_checked(&account.data){
                    Ok(amm_info)=>{
                        // 检查池子，如果既不是sol池也不是usdc那么直接抛出错误
                        if amm_info.coin_mint != sol_mint() && amm_info.pc_mint != usdc_mint(){
                            error!(
                                "SOL is not present in the raydiumpool: {}", pool_address
                            );        
                            return Err(anyhow::anyhow!("SOL is not present in the raydium pool", raydium_pool_pubkey));
                        }
                        // 如果是sol池子
                        let (sol_vault, token_vault) = if sol_mint() == amm_info.coin_mint{
                            (
                                amm_info.coin_vault,
                                amm_info.pc_vault
                            )
                        }
                        else{
                            (
                                amm_info.pc_vault, 
                                amm_info.coin_vault
                            )
                        }
                    }

                    pool_data.add_raydium_pool(
                        pool_address,
                        &token_vault.to_string(),
                        &sol_vault.to_string(),
                    )?;
                    info!("Raydium pool added: {}", pool_address);
                    info!("    Coin mint: {}", amm_info.coin_mint.to_string());
                    info!("    PC mint: {}", amm_info.pc_mint.to_string());
                    info!("    Token vault: {}", token_vault.to_string());
                    info!("    Sol vault: {}", sol_vault.to_string());
                    info!("    Initialized Raydium pool: {}\n", raydium_pool_pubkey);
                    }
                    Err(e)=>{
                        error!("Error parsing AmmInfo from Raydium pool {}: {:?}", raydium_pool_pubkey, e);
                        return Err(e);
                    };
                }
            }
            Err(e)=>{
                // 如果没能拿到池子的数据就在这里抛出错误
                error!("Error fetching Raydium pool account {}: {:?}", raydium_pool_pubkey, e);
                return Err(anyhow::anyhow!("Error fetching Raydium pool account"));
            }
        }
    }
    }
    if let Some(pool) = raydium_cp_pools  {

    }
    if let Some(pool) = dlmm_pools {

    }
    if let Some(pool) = whirlpool_pools {

    }
    if let Some(pool) = raydium_clmm_pools {

    }
    if let Some(pool) = raydium_clmm_pools {

    }
}