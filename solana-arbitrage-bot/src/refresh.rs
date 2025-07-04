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
        for pool_address in pools {
            let raydium_cp_pool_pubkey = Pubkey::from_str(pool_address)?;

            match rpc_client.get_account(&raydium_cp_pool_pubkey){
                Ok(account)=>{
                    // 检查账户是否由 raydium CP program 控制
                    if account.owner != raydium_cp_program_id(){
                        error!(
                            "Error: Raydium CP pool account is not owned by the Raydium CP program. Expected: {}, Actual: {}",
                            raydium_cp_program_id(), account.owner
                        );
                        return Err(anyhow::anyhow!("Raydium CP pool account is not owned by the Raydium CP program"));
                    }
                    // 解析 raydium cp pool数据
                    match RaydiumCpAmmInfo::load_checked(&account.data){
                        Ok(amm_info)=>{
                            if amm_info.token_0_mint != pool.data.mint && amm_info.token_1_mint != pool.data.mint{
                                error!(
                                    "Mint {} is not present in the raydium cp pool {}, skipping", pool_data.mint, raydium_cp_pool_pubkey
                                );
                                return Err(anyhow::anyhow!("Invalid raydium cp pool {}", raydium_cp_pool_pubkey));
                            }

                            // 经过上面判断可以考虑符合要求了，判断左侧是sol还是右侧是sol
                            let (sol_vault, token_vault) = if sol_mint() == amm_info.token_0_mint {
                                (amm_info.token_0_vault, amm_info.token_1_vault)
                            }
                            else if sol_mint() == amm_info.token_1_mint {
                                (amm_info.token_1_vault, amm_info.token_0_vault)
                            }
                            else {
                                error!(
                                    "SOL is not present in the raydium cp pool {}, skipping", raydium_cp_pool_pubkey
                                );
                                return Err(anyhow::anyhow!("Invalid raydium cp pool {}", raydium_cp_pool_pubkey));
                            };

                            pool_data.add_raydium_cp_pool(
                                pool_address,
                                &token_vault.to_string(),
                                &sol_vault.to_string(),
                                &amm_info.amm_config.to_string(),
                                &amm_info.abservation_key.to_string(),
                            )?;

                            info!("Raydium CP pool added: {}", pool_address);
                            info!("    Token vault: {}", amm_info.token_0_mint.to_string());
                            info!("    Sol vault: {}", amm_info.token_1_mint.to_string());
                            info!("    Amm config", amm_info.amm_config.to_string());
                            info!("    Observation key: {}\n", amm_info.abservation_key.to_string());
                        }
                        Err(e) => {
                            error!(
                                "Error parsing AmmInfo from Raydium CP pool {}: {:?}",
                                raydium_cp_pool_pubkey, e
                            );
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Error fetching Raydium CP pool account {}: {:?}",
                        raydium_cp_pool_pubkey, e
                    );
                    return Err(anyhow::anyhow!("Error fetching Raydium CP pool account"));
                }
            }
        }
    }
    if let Some(pool) = dlmm_pools {
        for pool_address in pools{
            let dlmm_pool_pubkey = Pubkey::from_str(pool_address)?;

            match rpc_client.get_account(&dlmm_pool_pubkey){
                Ok(account)=>{
                    // 检查账户是否由 dlmm program 控制，抛出提示，说明我们期望的dlmm的program id，以及当前的account的owner
                    if account.owner != dlmm_program_id(){
                        error!(
                            "Error: DLMM pool account is not owned by the DLMM program. Expected: {}, Actual: {}",  dlmm_program_id(), account.owner
                        )；
                        return Err(anyhow::anyhow!("DLMM pool account is not owned by the DLMM program"));
                    }

                    match DlmmInfo::load_checked(&account.data){
                        Ok(amm_info)=>{
                            let sol_mint = sol_mint();
                            let (token_vault, sol_vault) = 
                                amm_info.get_token_and_sol_vaults(pool_data.mint, sol_mint);
                            let bin_arrays = match amm_info.calculate_bin_arrays(&dlmm_pool_pubkey){
                                Ok(arrays)=> arrays, 
                                Err(e)=>{
                                    error!("Error calculating bin arrays for DLMM pool {}: {:?}", dlmm_pool_pubkey, e);
                                    return Err(e);
                                }
                            };

                            let bin_arrays_strings: Vec<String> = 
                                bin_arrays.iter().map(|pubkey| pubkey.to_string()).collect();
                            let bin_arrays_str_refs: Vec<&str> = 
                                bin_arrays_strings.iter().map(|s| s.as_str()).collect();

                            pool_data.add_dlmm_pool(
                                pool_address,
                                &token_vault.to_string(),
                                &sol_vault.to_string(),
                                &amm_info.oracle.to_string(),
                                bin_array_str_refs,
                            )?;
                            
                            info!("DLMM pool added: {}", pool_address);
                            info!("    Token X Mint: {}", amm_info.token_x_mint.to_string());
                            info!("    Token Y Mint: {}", amm_info.token_y_mint.to_string());
                            info!("    Token vault: {}", token_vault.to_string());
                            info!("    Sol vault: {}", sol_vault.to_string());
                            info!("    Oracle: {}", amm_info.oracle.to_string());
                            info!("    Active ID: {}", amm_info.active_id);
                            
                            // dlmm 会有多个bin array，所以需要打印出来
                            for(i, array) in bin_array_strings.iter().enumerate(){
                                info!("    Bin array {}: {}", i, array);
                            }
                            info!("");
                        }
                        Err(e) =>{
                            error!("Error parsing AmmInfo from DLMM pool {}: {:?}", dlmm_pool_pubkey, e);
                            return Err(e);
                        }
                    }
                }
                Err(e)=>{
                    error!("Error fetching DLMM pool account {}: {:?}", dlmm_pool_pubkey, e);
                    return Err(anyhow::anyhow!("Error fetching DLMM pool account"));
                }
            }
        }

    }
    if let Some(pool) = whirlpool_pools {
        // Whirlpool 是由 Orca 推出的 集中式流动性做市协议（Concentrated Liquidity AMM），类似于 Uniswap V3
        for pool_address in pools{
            let whirlpool_pool_pubkey = Pubkey::from_str(pool_address)?;

            match rpc_client.get_account(&whirlpool_pool_pubkey){
                Ok(account)=>{
                    // 检查账户是否由 whirlpool program 控制
                    if account.owner != whirlpool_program_id(){
                        error!(
                            "Error: Whirlpool pool account is not owned by the Whirlpool program. Expected: {}, Actual: {}",
                            whirlpool_program_id(), account.owner
                        );
                        return Err(anyhow::anyhow!("Whirlpool pool account is not owned by the Whirlpool program"));
                    }
                    // 解析数据
                    match Whirlpool::load_checked(&account.data){
                        Ok(Whirlpool)=>{
                            if whirlpool.token_mint_a != pool_data.mint &&
                                whirlpool.token_mint_b != pool_data.mint{
                                error!(
                                    "Mint {} is not present in the whirlpool pool {}, skipping", pool_data.mint, whirlpool_pool_pubkey
                                );
                                return Err(anyhow::anyhow!("Invalid whirlpool pool {}", whirlpool_pool_pubkey));
                            }
                            let sol_mint = sol_mint();
                            let (sol_vault, token_vault) = if sol_mint == whirlpool.token_mint_a{
                                (whirlpool.token_vault_a, whirlpool.token_vault_b)
                            }
                            else if sol_mint == whirlpool.token_mint_b{
                                (whirlpool.token_vault_b, whirlpool.token_vault_a)
                            }
                            else{
                                error!("SOL is not present in the whirlpool pool {}, skipping", whirlpool_pool_pubkey);
                            };
                            // 通过种子和程序派生出Whirlpool池子的Oracle地址 PDA程序，返回PDA 地址 和 bump seed 由于我们只关心
                            let whirlpool_oracle = Pubkey::find_program_address(
                                &[b"oracle", whirlpool.pool_pubkey.as_ref()],
                                &whirlpool_program_id(),
                            ).0;

                            // 从链上获取当前的whirlpool池子的tick array 地址
                            let whirlpool_tick_arrays = update_tick_array_accounts_for_onchain(
                                &whirlpool,
                                &whirlpool_pool_pubkey,
                                &whirlpool_program_id(),
                            );

                            // 将tick array 地址转换为字符串
                            let tick_array_strings: Vec<String> = whirlpool_tick_arrays
                                .iter()
                                .map(|meta| meta.pubkey.to_string())
                                .collect();
                            
                            // 将tick array 地址转换为字符串引用
                            let tick_array_str_refs: Vec<&str> =
                                tick_array_strings.iter().map(|s| s.as_str()).collect();

                            pool_data.add_whirlpool_pool(
                                pool_address,
                                &whirlpool_oracle.to_string(),
                                &token_vault.to_string(),
                                &sol_vault.to_string(),
                                tick_array_str_refs,
                            )?;

                            info!("Whirlpool pool added: {}", pool_address);
                            info!("    Token mint A: {}", whirlpool.token_mint_a.to_string());
                            info!("    Token mint B: {}", whirlpool.token_mint_b.to_string());
                            info!("    Token vault: {}", token_vault.to_string());
                            info!("    Sol vault: {}", sol_vault.to_string());
                            info!("    Oracle: {}", whirlpool_oracle.to_string());

                            for (i, array) in tick_array_strings.iter().enumerate() {
                                info!("    Tick Array {}: {}", i, array);
                            }
                            info!("");
                        }
                        Err(e) => {
                            error!(
                                "Error parsing Whirlpool data from pool {}: {:?}",
                                whirlpool_pool_pubkey, e
                            );
                            return Err(anyhow::anyhow!("Error parsing Whirlpool data"));
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Error fetching Whirlpool pool account {}: {:?}",
                        whirlpool_pool_pubkey, e
                    );
                    return Err(anyhow::anyhow!("Error fetching Whirlpool pool account"));
                }
            }

        }


    }
    if let Some(pool) = raydium_clmm_pools {
        for pool_address in pools{
            let raydium_clmm_program_id = raydium_clmm_program_id();

            match rpc_client.get_account(&Pubkey::from_str(pool_address)?){
                Ok(account)=>{
                    // 检查账户是否由raydium clmm 控制
                    if account.owner != raydium_clmm_program_id{
                        error!(
                            "Raydium CLMM pool {} is not owned by the Raydium CLMM program, skipping",
                            pool_address
                        );
                        continue;
                    }

                    match PoolState::load_checked(&account.data){
                        Ok(raydium_clmm)=>{
                            if raydium_clmm.token_mint_0 != pool_data.mint &&
                                raydium_clmm.token_mint_1 != pool_data.mint
                            {
                                error!(
                                    "Mint {} is not present in the raydium clmm pool {}, skipping", pool_data.mint, pool_address
                                );
                                continue;
                            }
                            let sol_mint = sol_mint();
                            let (token_vault, sol_vault) = if sol_mint == raydium_clmm.token_mint_0
                            {
                                (raydium_clmm.token_vault_1, raydium_clmm.token_vault_0)
                            } else if sol_mint == raydium_clmm.token_mint_1 {
                                (raydium_clmm.token_vault_0, raydium_clmm.token_vault_1)
                            } else {
                                error!("SOL is not present in Raydium CLMM pool {}", pool_address);
                                continue;
                            };

                            let tick_array_pubkeys = get_tick_array_pubkeys(
                                &Pubkey::from_str(pool_address)?,
                                raydium_clmm.tick_current,
                                raydium_clmm.tick_spacing,
                                &[-1, 0, 1],
                                &raydium_clmm_program_id,
                            )?;

                            let tick_array_strings: Vec<String> = tick_array_pubkeys
                                .iter()
                                .map(|pubkey| pubkey.to_string())
                                .collect();

                            let tick_array_str_refs: Vec<&str> =
                                tick_array_strings.iter().map(|s| s.as_str()).collect();

                            pool_data.add_raydium_clmm_pool(
                                pool_address,
                                &raydium_clmm.amm_config.to_string(),
                                &raydium_clmm.observation_key.to_string(),
                                &token_vault.to_string(),
                                &sol_vault.to_string(),
                                tick_array_str_refs,
                            )?;
                            info!("Raydium CLMM pool added: {}", pool_address);
                            info!(
                                "    Token mint 0: {}",
                                raydium_clmm.token_mint_0.to_string()
                            );
                            info!(
                                "    Token mint 1: {}",
                                raydium_clmm.token_mint_1.to_string()
                            );
                            info!("    Token vault: {}", token_vault.to_string());
                            info!("    Sol vault: {}", sol_vault.to_string());
                            info!("    AMM config: {}", raydium_clmm.amm_config.to_string());
                            info!(
                                "    Observation key: {}",
                                raydium_clmm.observation_key.to_string()
                            );

                            for (i, array) in tick_array_strings.iter().enumerate() {
                                info!("    Tick Array {}: {}", i, array);
                            }
                            info!("");

                        }
                        Err(e) => {
                            error!(
                                "Error parsing Raydium CLMM data from pool {}: {:?}",
                                pool_address, e
                            );
                            continue;
                        }
                    }

                }
                Err(e)=>{
                    error!("Error fetching Raydium CLMM pool account {}: {:?}", pool_address, e);
                    continue;
                }
            }
        }
    }
    Ok(pool_data)
}