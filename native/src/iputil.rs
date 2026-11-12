//! IP 工具模块（Task 4 完整实现；Task 3 先落 Setting 所需的最小桩）。

/// 当前对外服务的 IP 地址。
///
/// TODO(Task 4): 按存储的 ipFamily / netInterfaceName 枚举真实网卡地址
/// （排除回环地址与 `loopback|vmware|internal|lo|vEthernet` 命名，过滤后为空
/// 回退全表；ipv6 加 `[]`；无可用地址回退 `127.0.0.1` / `::1`）。
/// 当前恒回 `"127.0.0.1"`，保证 Setting 的 url / 端口逻辑可测。
pub fn get_ip() -> String {
    "127.0.0.1".to_string()
}
