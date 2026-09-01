const DEFAULT_DOWNLOAD_PROXY: &str = "http://127.0.0.1:61193";

/// All Mosaic-managed file downloads use the local download proxy by default.
/// The value is never persisted and contains no application or CPA credential.
pub fn download_agent_builder() -> ureq::AgentBuilder {
    let proxy_url = std::env::var("MOSAIC_DOWNLOAD_PROXY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_DOWNLOAD_PROXY.to_string());
    let proxy = ureq::Proxy::new(&proxy_url)
        .unwrap_or_else(|_| ureq::Proxy::new(DEFAULT_DOWNLOAD_PROXY).expect("valid proxy URL"));
    ureq::AgentBuilder::new().proxy(proxy)
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_download_proxy_is_loopback_port_61193() {
        assert_eq!(super::DEFAULT_DOWNLOAD_PROXY, "http://127.0.0.1:61193");
    }
}
