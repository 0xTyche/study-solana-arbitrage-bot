use create::config::Config;
use create::refresh::intialize_pool_data;
use create::transaction::build_and_send_transaction;
use anyhow::Context;

use solana_client::rpc_client::RpcClient;
use solana_sdk::address_lookup_table::state::AddressLookupTable;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub async fn run(config_path: &str) -> anyhow::Result<()> {
    let config = Config::load(config_path)?;
    info!("Starting bot with config: {:?}", config);

    let rpc_clients = Arc::new(RpcClient::new(config.rpc.url.clone()));
    let sending_rpc_clients = if let Some(spam_config) = &config.spam{
        if spam_config.enabled{
            spam_config
                .sending_rpc_clients
                .iter()
                .map(|url| Arc::new(RpcClient::new(url.clone())))
                .collect::<Vec<_>>()
        }
        else{
            vec![rpc_clients.clone()]
        }
    };

    let wallet_kp = 
        load_keypair(&config.wallet.private_key).context("Failed to load wallet keypair")?;
    // 打印钱包公钥
    info!("Loaded wallet keypair: {:?}", wallet_kp.pubkey());

    // 获取当前链上最后一个块
    let initial_blockhash = rpc_client.get_latest_blockhash()?;

    /**
    * 为了共享可变的最后一个blockhash，使用Arc和Mutex来包装它
    */
    let cached_blockhash = Arc::new(Mutex::new(initial_blockhash));
    tokio::spawn(async move{
        blockhash_refresher(blockhash_client, blockhash_cache, refresh_interval).await;
    })
    // 每个block的刷新时间大概是150s，这个刷新间隔较为合理
    let refresh_interval = Duration::from_secs(10);

    // 创建一个异步任务用于后台刷新
    let blockhash_client = rpc_client.clone();
    let blockhash_cache = cached_blockhash.clone();
    tokio::spawn(async move {
        blockhash_refresher(blockhash_client, blockhash_cache, refresh_interval).await;
    });

    for mint_config in &config.routing.mint_config_list{
        info!("Processing mint: {:?}", mint_config.mint);

        // 初始化代币池子信息
        let pool_data = initialize_pool_data(
            &mint_config.mint,
            &wallet_kp.pubkey().to_string(),
            mint_config.raydium_pool_list.as_ref(),
            mint_config.raydium_cp_pool_list.as_ref(),
            mint_config.pump_pool_list.as_ref(),
            mint_config.meteora_dlmm_pool_list.as_ref(),
            mint_config.whirlpool_pool_list.as_ref(),
            mint_config.raydium_clmm_pool_list.as_ref(),
            rpc_client.clone(),
        )
        .await?;

        // 将池子数据包装成Arc和Mutex
        let mint_pool_data = Arc::new(Mutex::new(pool_data));
        
        let config_clone = config.clone();
        let mint_config_clone = mint_config.clone();
        let sending_rpc_clients_clone = sending_rpc_clients.clone();
        let cache_blockhash_clone = cached_blockhash.clone();
        let wallet_bytes = wallet_kp.to_bytes();
        let wallet_kp_clone = Keypair::from_bytes(&wallet_bytes).unwrap();

        let mut lookup_table_accounts = mint_config_clone.lookup_table_accounts.unwrap_or_default();

        // 添加ALT地址
        lookup_table_accounts.push("CCXTHFHXar7fTEQWrcC7iAWcJSJRbYBjwgtGxBKSz6rt".to_string());
        
        // 将ALT中存储的的地址反序列化出来后，添加到lookup_table_accounts_list中
        let mut lookup_table_accounts_list = vec![];

        for lookup_table_account in lookup_table_accounts {
            match Pubkey::from_str(&lookup_table_account) {
                Ok(pubkey) => {
                    match rpc_client.get_account(&pubkey) {
                        Ok(account) => {
                            match AddressLookupTable::deserialize(&account.data) {
                                Ok(lookup_table) => {
                                    let lookup_table_account = AddressLookupTableAccount {
                                        key: pubkey,
                                        addresses: lookup_table.addresses.into_owned(),
                                    };
                                    lookup_table_accounts_list.push(lookup_table_account);
                                    info!("   Successfully loaded lookup table: {}", pubkey);
                                }
                                Err(e) => {
                                    error!(
                                        "   Failed to deserialize lookup table {}: {}",
                                        pubkey, e
                                    );
                                    continue; // Skip this lookup table but continue processing others
                                }
                            }
                        }
                        Err(e) => {
                            error!("   Failed to fetch lookup table account {}: {}", pubkey, e);
                            continue; // Skip this lookup table but continue processing others
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "   Invalid lookup table pubkey string {}: {}",
                        lookup_table_account, e
                    );
                    continue; // Skip this lookup table but continue processing others
                }
            }
        }
        if lookup_table_accounts_list.is_empty() {
            warn!("   Warning: No valid lookup tables were loaded");
        } else {
            info!(
                "   Loaded {} lookup tables successfully",
                lookup_table_accounts_list.len()
            );
        }

        // 创建一个异步任务用于后台处理交易
        tokio::spawn(async move {
            // 从设置中获取定义的毫秒级别的时间间隔
            let process_delay = Duration::from_millis(mint_config_clone.process_delay);

            loop {
                // 获取最新的blockhash
                let latest_blockhash = {
                    let guard = cached_blockhash_clone.lock().await;
                    *guard
                };

                // 获取池子数据
                let guard = mint_pool_data.lock().await;

                // 构建并发送交易
                match build_and_send_transaction(
                    &wallet_kp_clone,
                    &config_clone,
                    &*guard, // Dereference the guard here
                    &sending_rpc_clients_clone,
                    latest_blockhash,
                    &lookup_table_accounts_list,
                )
                .await
                {
                    Ok(signatures) => {
                        info!(
                            "Transactions sent successfully for mint {}",
                            mint_config_clone.mint
                        );
                        for signature in signatures {
                            info!("  Signature: {}", signature);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Error sending transaction for mint {}: {}",
                            mint_config_clone.mint, e
                        );
                    }
                }

                tokio::time::sleep(process_delay).await;
            }
        });
    }
    // 防止主线程退出，保持tokio runtime持续运行
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

    }

}

// blockhash refresher 方法实现
