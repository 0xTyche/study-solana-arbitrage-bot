use serde::{Deserialize, Deserializer};
use std::{env, fs::File, io::Read};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub bot: BotConfig,
    pub routing: RoutingConfig,
    pub rpc: RpcConfig,
    pub spam: Option<SpamConfig>,
    pub wallet: WalletConfig,
    pub kamino_flashload: Option<KaminoFlashloadConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BotConfig {
    pub compute_unit_limit: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RoutingConfig {
    pub mint_config_list: Vec<MintConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MintConfig {
    // mint代币类型定义
    pub mint: String,
    pub raydium_pool_list: Option<Vec<String>>,
    pub meteora_dlmm_pool_list: Option<Vec<String>>,
    pub raydium_cp_pool_list: Option<Vec<String>>,
    pub pump_pool_list: Option<Vec<String>>,
    pub whirlpool_pool_list: Option<Vec<String>>,
    pub raydium_clmm_pool_list: Option<Vec<String>>,
    pub lookup_table_list: Option<Vec<String>>,
    // 处理延迟、限制套利路径执行频率、定时更新某个mint的dex信息、异步任务处理节流
    pub process_delay: u64
}

#[derive(Debug, Deserialize, Clone)]
pub struct RpcConfig {
    // 提高兼容性，可以在文件中写入，也可以在环境中配置
    #[serde(deserialize_with = "serde_string_or_env")]
    pub url: String,
}

// 用多个 RPC 发 spam 式套利交易”的参数
#[derive(Debug, Deserialize, Clone)]
pub struct SpamConfig {
    pub enabled: bool,
    // 多个solana rpc 节点
    pub sending_rpc_urls: Vec<String>,
    // 设置每个交易的优先费
    pub compute_unit_price: u64,
    pub max_retries: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WalletConfig {
    #[serde(deserialize_with = "serde_string_or_env")]
    pub private_key: String,
}

// 用于判断是否启用kamino闪电贷
#[derive(Debug, Deserialize, Clone)]
pub struct KaminoFlashloanConfig {
    pub enabled: bool,
}

// 自定义反序列化函数
pub fn serde_string_or_env<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    // 从deserializer中读取反序列化值
    let value_or_env = String::deserialize(deserializer)?;
    // 判断数据的第一个字符是否为$
    // 闭包，直接抛出错误，而不返回具体的error信息
    let value = match value_or_env.chars().next() {
        Some('$') => env::var(&value_or_env[1..])
        .unwrap_or_else(|_| panic!("{} is not a valid environment variable", &value_or_env[1...])),
        _ => value_or_env,
    };
    Ok(value)
}

imp Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
}