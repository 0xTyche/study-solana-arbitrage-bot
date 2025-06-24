## 链上套利的限制

1.夹子机器人之间的竞争
- nodes 和 validators 可以抢先交易
- 交易顺序可以被操纵
- 对执行时间的控制有限

2.技术限制
- 复杂计算受限于 Compute Unit（计算单元），cu是 Solana 上衡量每笔交易计算资源消耗的单位，如果该笔交易超过了cu上限就无法执行
- 多跳交易受交易大小限制
- 相比链下解决方案，延迟更高
  - 链上程序需等待 slot 排程、交易提交、网络传播、区块确认，因此 存在天然的延迟劣势。

3.推荐做法
- 使用链下检测套利机会
- 通过支持 MEV 的 RPC 服务商提交交易
- 建议集成 Jito-MEV 以提高执行效果

## 核心组件设计
1. 📡 价格监控系统（Price Monitoring System）
- 实时监控多个 DEX（如 Raydium、Orca、Meteora、Jupiter）上的代币价格
- 通过 WebSocket 获取即时行情更新
- 计算每笔交易的价格冲击（Price Impact）
- 分析交易池的流动性深度，以判断成交能力

2. 🧠 策略类型（Strategy Types）
A. 🚀 二跳套利（Two-Hop Arbitrage）

交易分析示例：
输入：0.196969275 Token A  
↓ [Meteora DEX]  
中间输出：146.90979292 Token B  
↓ [Raydium DEX]  
最终输出：0.202451396 Token A  
利润：约 2.78%

B. 🔺 三角套利（Triangle Arbitrage）

套利路径示意：
Token A → Token B [Meteora]  
Token B → Token C [Meteora]  
Token C → Token A [Raydium]

C. 🔁 跨 DEX 套利（Multi-DEX Arbitrage）

Whirlpool + Orca 路由示例：
输入：0.314737179 Token A  
↓ [Orca]  
中间输出：118.612731091 Token B  
↓ [Whirlpool]  
最终输出：0.316606012 Token A  
利润：约 0.59%

3. ⚙️ 执行逻辑（Execution Methods）
📌 优先队列调度（Priority Queue）
仅执行满足最低利润阈值的交易（例如 ≥ 0.5%）

对每笔交易进行Gas 成本估算和滑点计算

🧮 路由优化（Route Optimization）

基于以下因素选择最佳 DEX 路由：
- 流动性深度
- 历史成交成功率
- 交易成本效率（Gas 使用）

交易结构构造

```rust
// 示例结构
const route = {
  steps: [
    { dex: "Meteora", tokenIn: "A", tokenOut: "B" },
    { dex: "Raydium", tokenIn: "B", tokenOut: "A" }
  ],
  expectedProfit: "2.78%",
  gasEstimate: 200000
};
```

##  阅读指导
main.rs → bot.rs
dex/raydium（或任一熟悉 DEX）→ transaction.rs
pools.rs → refresh.rs

扩展：
dlmm/, pump/, whirlpool/, kamino.rs

## 创建ALT
```bash
root@racknerd-9da1d08:~/home/Solana-Arbitrage-Bot# solana address-lookup-table create --authority $(solana address)

Signature: 2FERiXyGaT4Sanr1vcAjh91nASCXJMnmWHm19VMyRVfnJQaRFAjdNPLkAhnrVbpGzC3kbBhLwsrajergaruyZynt
Lookup Table Address: CCXTHFHXar7fTEQWrcC7iAWcJSJRbYBjwgtGxBKSz6rt

root@racknerd-9da1d08:~/home/Solana-Arbitrage-Bot# 

# 添加ALT扩展地址,该地址已经添加到ALT当中
root@racknerd-9da1d08:~/home/Solana-Arbitrage-Bot# solana address-lookup-table extend CCXTHFHXar7fTEQWrcC7iAWcJSJRbYBjwgtGxBKSz6rt --addresses D4udSzAbdyKpRYCvA7iaoGZnu13cXx5VMxx7DP4rBus1

Signature: 37BPnWF19KTFQFbawCr6ZKhPRw6aNyTb22SmxDNfCMoDbAw58bwTUZDozN3P5K8TRCYWgUp5NraPVQtU9KoLcvCi

```

## 他人套利成功例子-2300倍收益

https://solscan.io/tx/3Aqpq9mcpnNPgL5kgLhocDMMAVqaDpPygyiTDtFwBw7uJmasRPL5Nz1H62ehvwvpVdpueTHtBiCCAvsbgUck2YMg

## raydium amm 链上池子信息举例 wsol-usdc

Link：https://solscan.io/account/3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv#anchorData

## whirlpool ticks and price
https://dev.orca.so/Architecture%20Overview/Price%20&%20Ticks
