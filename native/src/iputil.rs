//! 网卡地址枚举；IPv6 地址在 URL 中加方括号。

use std::net::IpAddr;

use serde_json::{json, Value};

use crate::setting;

#[derive(Clone)]
struct InterfaceAddress {
    name: String,
    address: IpAddr,
    netmask: IpAddr,
}

fn interfaces() -> Result<Vec<InterfaceAddress>, String> {
    if_addrs::get_if_addrs()
        .map(|items| {
            items
                .into_iter()
                .map(|item| {
                    let (address, netmask) = match item.addr {
                        if_addrs::IfAddr::V4(addr) => {
                            (IpAddr::V4(addr.ip), IpAddr::V4(addr.netmask))
                        }
                        if_addrs::IfAddr::V6(addr) => {
                            (IpAddr::V6(addr.ip), IpAddr::V6(addr.netmask))
                        }
                    };
                    InterfaceAddress {
                        name: item.name,
                        address,
                        netmask,
                    }
                })
                .collect()
        })
        .map_err(|error| format!("网卡枚举失败: {error}"))
}

fn normalize_family(family: &str) -> Result<String, String> {
    let family = if family.is_empty() {
        "ipv4".to_string()
    } else {
        family.to_lowercase()
    };
    if family == "ipv4" || family == "ipv6" {
        Ok(family)
    } else {
        Err("family must be ipv4 or ipv6".to_string())
    }
}

fn usable(address: &IpAddr, family: &str) -> bool {
    if address.is_loopback() || address.is_unspecified() || address.is_multicast() {
        return false;
    }
    match address {
        IpAddr::V4(_) => family == "ipv4",
        // 无 scope id 的 link-local 不能生成可跨设备访问的分享 URL。
        IpAddr::V6(ip) => {
            family == "ipv6"
                && !ip.is_unicast_link_local()
                && ip
                    .to_ipv4_mapped()
                    .is_none_or(|mapped| !mapped.is_loopback())
        }
    }
}

fn names_for(items: &[InterfaceAddress], family: &str) -> Vec<String> {
    let mut names = Vec::new();
    for item in items.iter().filter(|item| usable(&item.address, family)) {
        if !names.contains(&item.name) {
            names.push(item.name.clone());
        }
    }
    let filtered: Vec<_> = names
        .iter()
        .filter(|name| {
            let name = name.to_lowercase();
            !["loopback", "vmware", "internal", "lo", "vethernet"]
                .iter()
                .any(|part| name.contains(part))
        })
        .cloned()
        .collect();
    if filtered.is_empty() {
        names
    } else {
        filtered
    }
}

fn address_by_name(items: &[InterfaceAddress], name: &str, family: &str) -> Option<String> {
    items
        .iter()
        .find(|item| item.name == name && usable(&item.address, family))
        .map(|item| item.address.to_string())
}

fn loopback(family: &str) -> &'static str {
    if family == "ipv6" {
        "::1"
    } else {
        "127.0.0.1"
    }
}

fn address_at(items: &[InterfaceAddress], idx: usize, family: &str) -> String {
    let names = names_for(items, family);
    if names.is_empty() {
        return loopback(family).to_string();
    }
    address_by_name(items, &names[idx % names.len()], family)
        .unwrap_or_else(|| loopback(family).to_string())
}

fn url_address(ip: &str, family: &str) -> String {
    if family == "ipv6" {
        format!("[{ip}]")
    } else {
        ip.to_string()
    }
}

pub fn get_net_interface_names(family: &str) -> Result<Vec<String>, String> {
    Ok(names_for(&interfaces()?, &normalize_family(family)?))
}

pub fn get_ip_addresses(name: &str, family: &str) -> Result<Vec<Value>, String> {
    let family = normalize_family(family)?;
    Ok(interfaces()?
        .iter()
        .filter(|item| item.name == name && usable(&item.address, &family))
        .map(|item| {
            let prefix = match item.netmask {
                IpAddr::V4(mask) => u32::from(mask).count_ones(),
                IpAddr::V6(mask) => u128::from(mask).count_ones(),
            };
            json!({
                "address": item.address.to_string(), "netmask": item.netmask.to_string(),
                "family": if family == "ipv6" { "IPv6" } else { "IPv4" },
                "mac": null, "internal": false, "cidr": format!("{}/{prefix}", item.address),
            })
        })
        .collect())
}

pub fn get_ip_address(idx: usize, family: &str) -> Result<String, String> {
    Ok(address_at(&interfaces()?, idx, &normalize_family(family)?))
}

pub fn get_ip_address_by_name(name: &str, family: &str) -> Result<Option<String>, String> {
    Ok(address_by_name(
        &interfaces()?,
        name,
        &normalize_family(family)?,
    ))
}

pub fn set_ip_family(family: &str) -> Result<(), String> {
    setting::set_ip_family(&json!(normalize_family(family)?))
}

pub fn get_ip_family() -> Result<String, String> {
    normalize_family(setting::get_ip_family()?.as_str().unwrap_or("ipv4"))
}

pub fn set_net_interface(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("netInterfaceName is required".to_string());
    }
    setting::set_net_interface_name(&json!(name))
}

pub fn get_net_interface() -> Result<String, String> {
    let family = get_ip_family()?;
    let names = get_net_interface_names(&family)?;
    let stored = setting::get_net_interface_name()?
        .as_str()
        .unwrap_or_default()
        .to_string();
    let name = if names.contains(&stored) {
        stored
    } else {
        names.into_iter().next().unwrap_or_default()
    };
    if !name.is_empty() {
        set_net_interface(&name)?;
    }
    Ok(name)
}

pub fn get_ip() -> Result<String, String> {
    let family = get_ip_family()?;
    let name = get_net_interface()?;
    let address =
        get_ip_address_by_name(&name, &family)?.unwrap_or_else(|| loopback(&family).to_string());
    Ok(url_address(&address, &family))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::with_test_home;
    use serde_json::json;

    fn address(name: &str, ip: &str) -> InterfaceAddress {
        InterfaceAddress {
            name: name.to_string(),
            address: ip.parse().unwrap(),
            netmask: if ip.contains(':') {
                "ffff:ffff:ffff:ffff::".parse().unwrap()
            } else {
                "255.255.255.0".parse().unwrap()
            },
        }
    }

    #[test]
    fn filters_loopback_interfaces_and_falls_back_when_name_filter_empties() {
        let interfaces = vec![
            address("lo0", "127.0.0.1"),
            address("vmware0", "192.168.5.1"),
            address("en0", "192.168.1.20"),
        ];
        assert_eq!(names_for(&interfaces, "ipv4"), vec!["en0"]);
        assert_eq!(names_for(&interfaces[..2], "ipv4"), vec!["vmware0"]);
        assert!(names_for(&interfaces[..1], "ipv4").is_empty());
    }

    #[test]
    fn interface_index_wraps_and_no_network_has_valid_loopback_url() {
        let interfaces = vec![address("en0", "192.168.1.20"), address("en1", "10.0.0.2")];
        assert_eq!(address_at(&interfaces, 3, "ipv4"), "10.0.0.2");
        assert_eq!(address_at(&[], 3, "ipv4"), "127.0.0.1");
        assert_eq!(address_at(&[], 0, "ipv6"), "::1");
        assert_eq!(url_address("::1", "ipv6"), "[::1]");
    }

    #[test]
    fn ipv6_selects_a_usable_address_instead_of_unscoped_link_local() {
        let interfaces = vec![
            address("en0", "fe80::1234"),
            address("en0", "fd12:3456::2"),
            address("lo0", "::1"),
        ];
        assert_eq!(address_at(&interfaces, 0, "ipv6"), "fd12:3456::2");
        assert_eq!(url_address("fd12:3456::2", "ipv6"), "[fd12:3456::2]");
        assert_eq!(address_at(&interfaces[..1], 0, "ipv6"), "::1");
    }

    #[test]
    fn family_normalization_and_selection_persist() {
        with_test_home(|_| {
            set_ip_family("IPv6").unwrap();
            assert_eq!(get_ip_family().unwrap(), "ipv6");
            set_net_interface("en-test").unwrap();
            assert_eq!(
                crate::setting::get_net_interface_name().unwrap(),
                json!("en-test")
            );
            assert_eq!(
                set_ip_family("ipv5").unwrap_err(),
                "family must be ipv4 or ipv6"
            );
            assert_eq!(
                set_net_interface("").unwrap_err(),
                "netInterfaceName is required"
            );
        });
    }

    #[test]
    fn dispatch_network_methods_normalize_payloads_and_report_validation_errors() {
        with_test_home(|_| {
            let result = crate::dispatch("setIpFamily", json!({"ipFamily":"IPv6"}), "");
            assert_eq!(result["ok"], json!(true));
            assert_eq!(
                crate::dispatch("getIpFamily", json!(null), "")["data"],
                json!("ipv6")
            );
            assert_eq!(
                crate::dispatch("setIpFamily", json!("ipv5"), "")["error"],
                json!("family must be ipv4 or ipv6")
            );
            let missing = crate::dispatch(
                "getIpAddresses",
                json!({"netInterfaceName":"missing","family":"ipv4"}),
                "",
            );
            assert_eq!(missing["data"], json!([]));
        });
    }
}
