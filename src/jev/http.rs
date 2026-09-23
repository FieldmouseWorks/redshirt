use super::*;

/// Pinned native HTTPS transport. Constructing it never sends a request.
pub struct Https {
    client: reqwest::Client,
    authorization: reqwest::header::HeaderValue,
    request_bytes: usize,
}

impl Https {
    fn new(key: &str, request_bytes: usize) -> Result<Self> {
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
            request_bytes,
        })
    }

    fn request(&self, body: Vec<u8>) -> Result<reqwest::Request> {
        require(body.len() <= self.request_bytes, "provider_request_size")?;
        self.client
            .post(ENDPOINT)
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .build()
            .map_err(|_| Stop::from("provider_transport"))
    }
}

impl Jev<Https> {
    pub fn live(key: String, config: Config) -> Result<Self> {
        config.validate()?;
        let transport = Https::new(&key, config.request_bytes)?;
        let mut provider = Self::new(transport, config)?;
        provider.name = "jev-live";
        provider.secret = Some(key);
        Ok(provider)
    }
}

#[async_trait]
impl Transport for Https {
    async fn post(&mut self, body: Vec<u8>) -> Result<(u16, Vec<u8>)> {
        let request = self.request(body)?;
        let mut response = self
            .client
            .execute(request)
            .await
            .map_err(|_| Stop::from("provider_transport"))?;
        let status = response.status().as_u16();
        require(
            response
                .content_length()
                .is_none_or(|n| n <= MAX_BYTES as u64),
            "provider_response_size",
        )?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Stop::from("provider_transport"))?
        {
            require(
                bytes.len() + chunk.len() <= MAX_BYTES,
                "provider_response_size",
            )?;
            bytes.extend_from_slice(&chunk);
        }
        Ok((status, bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pinned_request_and_sensitive_credentials_without_network() {
        let provider = Jev::live("synthetic-only-key".into(), Config::default()).unwrap();
        let request = provider.transport.request(b"{}".to_vec()).unwrap();
        assert_eq!(request.url().as_str(), ENDPOINT);
        assert_eq!(request.method(), reqwest::Method::POST);
        assert!(request.headers()[reqwest::header::AUTHORIZATION].is_sensitive());
        assert!(!format!("{request:?}").contains("synthetic-only-key"));
        assert!(provider.transport.request(vec![0; MAX_BYTES + 1]).is_err());
        assert_eq!(provider.calls(), 0);
        for key in ["", " leading", "trailing ", "invalid\nheader"] {
            assert!(Jev::live(key.into(), Config::default()).is_err());
        }
    }

    #[tokio::test]
    async fn native_transport_honors_validated_host_request_limit() {
        let provider = Jev::live(
            "synthetic-only-key".into(),
            Config {
                request_bytes: 32768,
                ..Config::default()
            },
        )
        .unwrap();
        assert!(provider.transport.request(vec![0; 32768]).is_ok());
        assert!(provider.transport.request(vec![0; 32769]).is_err());
        assert_eq!(provider.calls(), 0);
        assert!(
            Jev::live(
                "synthetic-only-key".into(),
                Config {
                    request_bytes: MAX_REQUEST_BYTES + 1,
                    ..Config::default()
                }
            )
            .is_err()
        );
    }
}
