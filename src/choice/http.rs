use super::*;

/// Pinned official HTTPS endpoints. Constructing this transport sends nothing.
pub struct Https {
    client: reqwest::Client,
    authorization: reqwest::header::HeaderValue,
    endpoint: &'static str,
    request_bytes: usize,
}

impl Https {
    fn new(key: &str, config: &Config) -> Result<Self> {
        require(
            !key.is_empty() && key.len() <= 4096 && key.trim() == key,
            "missing_or_invalid_key",
        )?;
        let mut authorization = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| Stop::from("missing_or_invalid_key"))?;
        authorization.set_sensitive(true);
        let client = reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .timeout(DEADLINE)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .map_err(|_| Stop::from("provider_transport_config"))?;
        Ok(Self {
            client,
            authorization,
            endpoint: config.profile.endpoint(),
            request_bytes: config.request_bytes,
        })
    }
    fn request(&self, body: Vec<u8>) -> Result<reqwest::Request> {
        require(body.len() <= self.request_bytes, "provider_request_size")?;
        self.client
            .post(self.endpoint)
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .build()
            .map_err(|_| Stop::from("provider_transport"))
    }
}

impl Choice<Https> {
    pub fn live(key: String, config: Config) -> Result<Self> {
        config.validate()?;
        let transport = Https::new(&key, &config)?;
        let mut provider = Self::new(transport, config)?;
        provider.secret = Some(key);
        Ok(provider)
    }
}

#[async_trait]
impl Transport for Https {
    async fn post(&mut self, body: Vec<u8>) -> Result<Reply> {
        let request = self.request(body)?;
        let mut response = self
            .client
            .execute(request)
            .await
            .map_err(|_| Stop::from("provider_transport"))?;
        let status = response.status().as_u16();
        let mut rate_limits = BTreeMap::new();
        for (name, value) in response.headers() {
            let name = name.as_str();
            if (name.starts_with("x-ratelimit-") || name.starts_with("ratelimit-"))
                && rate_limits.len() < 16
                && let Ok(value) = value.to_str()
                && value.len() <= 100
                && value.is_ascii()
            {
                rate_limits.insert(name.to_owned(), value.to_owned());
            }
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Stop::from("provider_transport"))?
        {
            let remaining = MAX_BYTES + 1 - bytes.len();
            bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
            if bytes.len() > MAX_BYTES {
                break;
            }
        }
        let overflow = bytes.len() > MAX_BYTES;
        Ok(Reply {
            status,
            body: bytes,
            overflow,
            rate_limits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_requests_and_sensitive_keys() {
        for profile in [Profile::OpenaiLuna, Profile::DeepseekFlash] {
            let config = Config {
                version: 1,
                profile,
                request_limit: 6,
                request_bytes: MAX_BYTES,
            };
            let provider = Choice::live("synthetic-only-key".into(), config).unwrap();
            let request = provider.transport.request(b"{}".to_vec()).unwrap();
            assert_eq!(request.url().as_str(), profile.endpoint());
            assert_eq!(request.method(), reqwest::Method::POST);
            assert!(request.headers()[reqwest::header::AUTHORIZATION].is_sensitive());
            assert!(!format!("{request:?}").contains("synthetic-only-key"));
            assert!(provider.transport.request(vec![0; MAX_BYTES + 1]).is_err());
            assert_eq!(provider.calls(), 0);
        }
    }
}
