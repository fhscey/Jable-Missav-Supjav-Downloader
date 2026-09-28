use reqwest::{Client, IntoUrl, Method, Proxy, RequestBuilder};
use std::collections::HashMap;
use std::ops::Deref;
use std::time::Duration;

use super::error::NetworkError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "url")]
pub enum ProxyMode {
    System,
    Direct,
    Custom(String),
}

pub const DEFAULT_UA: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct HttpClient {
    inner: Client,
    proxy_mode: ProxyMode,
}

impl Deref for HttpClient {
    type Target = Client;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl HttpClient {
    pub fn client(&self) -> &Client {
        &self.inner
    }

    pub fn inner(&self) -> &Client {
        &self.inner
    }

    pub fn new(proxy_mode: &ProxyMode) -> anyhow::Result<Self> {
        let client = Self::build_client(proxy_mode)?;
        Ok(Self {
            inner: client,
            proxy_mode: proxy_mode.clone(),
        })
    }

    /// 动态切换代理模式（重新构建底层连接池）
    pub fn set_proxy(&mut self, proxy_mode: &ProxyMode) -> anyhow::Result<()> {
        let inner = Self::build_client(proxy_mode)?;
        self.inner = inner;
        self.proxy_mode = proxy_mode.clone();
        Ok(())
    }

    pub fn proxy_mode(&self) -> &ProxyMode {
        &self.proxy_mode
    }

    fn build_client(proxy_mode: &ProxyMode) -> anyhow::Result<Client> {
        let mut builder = Client::builder()
            .user_agent(DEFAULT_UA)
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10);

        match proxy_mode {
            ProxyMode::Direct => {
                builder = builder.no_proxy();
            }
            ProxyMode::System => {
                match sysproxy::Sysproxy::get_system_proxy() {
                    Ok(sys) if sys.enable => {
                        let proxy_url = format!("http://{}:{}", sys.host, sys.port);
                        let proxy = Proxy::all(proxy_url)?;
                        builder = builder.proxy(proxy);
                    }
                    _ => {
                        builder = builder.no_proxy();
                    }
                }
            }
            ProxyMode::Custom(url) => {
                let proxy = Proxy::all(url)?;
                builder = builder.proxy(proxy);
            }
        }

        let client = builder.build()?;
        Ok(client)
    }

    #[inline]
    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.get(url)
    }

    #[inline]
    pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.post(url)
    }

    #[inline]
    pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.head(url)
    }

    #[inline]
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        self.inner.request(method, url)
    }

    pub async fn fetch_text_with_retry(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
        max_retries: usize,
    ) -> Result<String, NetworkError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let mut req = self.inner.get(url);
            if let Some(ref h) = headers {
                for (k, v) in h {
                    req = req.header(k, v);
                }
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp.text().await.map_err(NetworkError::RequestError)?);
                    }
                    if attempt >= max_retries {
                        return Err(NetworkError::HttpStatus(status.as_u16()));
                    }
                }
                Err(e) => {
                    if attempt >= max_retries {
                        return Err(NetworkError::RequestError(e));
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500 * (1 << (attempt - 1)))).await;
        }
    }
}
