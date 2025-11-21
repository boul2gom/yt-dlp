use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};

use super::{DownloadEvent, EventFilter, RetryStrategy};

/// HTTP method for webhook delivery
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WebhookMethod {
    /// HTTP POST
    #[default]
    Post,
    /// HTTP PUT
    Put,
    /// HTTP PATCH
    Patch,
}

/// Webhook configuration
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Webhook URL
    url: String,
    /// HTTP method
    method: WebhookMethod,
    /// Custom headers
    headers: HashMap<String, String>,
    /// Event filter
    filter: EventFilter,
    /// Retry strategy
    retry_strategy: RetryStrategy,
    /// Request timeout
    timeout: Duration,
    /// Whether to include full event data or just summary
    include_full_data: bool,
}

impl WebhookConfig {
    /// Creates a new webhook configuration
    ///
    /// # Arguments
    ///
    /// * `url` - The webhook URL
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: WebhookMethod::default(),
            headers: HashMap::new(),
            filter: EventFilter::all(),
            retry_strategy: RetryStrategy::default(),
            timeout: Duration::from_secs(10),
            include_full_data: true,
        }
    }

    /// Creates a webhook from environment variables
    ///
    /// Reads the following environment variables:
    /// - `YTDLP_WEBHOOK_URL` - Webhook URL (required)
    /// - `YTDLP_WEBHOOK_METHOD` - HTTP method (optional, default: POST)
    /// - `YTDLP_WEBHOOK_TIMEOUT` - Timeout in seconds (optional, default: 10)
    ///
    /// # Returns
    ///
    /// Some(WebhookConfig) if YTDLP_WEBHOOK_URL is set, None otherwise
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("YTDLP_WEBHOOK_URL").ok()?;

        let mut config = Self::new(url);

        if let Ok(method) = std::env::var("YTDLP_WEBHOOK_METHOD") {
            config.method = match method.to_uppercase().as_str() {
                "POST" => WebhookMethod::Post,
                "PUT" => WebhookMethod::Put,
                "PATCH" => WebhookMethod::Patch,
                _ => WebhookMethod::Post,
            };
        }

        if let Ok(timeout_str) = std::env::var("YTDLP_WEBHOOK_TIMEOUT")
            && let Ok(timeout_secs) = timeout_str.parse::<u64>()
        {
            config.timeout = Duration::from_secs(timeout_secs);
        }

        Some(config)
    }

    /// Sets the HTTP method
    pub fn with_method(mut self, method: WebhookMethod) -> Self {
        self.method = method;
        self
    }

    /// Adds a custom header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Sets multiple headers at once
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    /// Sets the event filter
    pub fn with_filter(mut self, filter: EventFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Sets the retry strategy
    pub fn with_retry_strategy(mut self, strategy: RetryStrategy) -> Self {
        self.retry_strategy = strategy;
        self
    }

    /// Sets the request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets whether to include full event data
    pub fn with_full_data(mut self, include: bool) -> Self {
        self.include_full_data = include;
        self
    }

    /// Returns the URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the filter
    pub fn filter(&self) -> &EventFilter {
        &self.filter
    }
}

/// Webhook payload that will be sent
#[derive(Debug, Clone, Serialize)]
struct WebhookPayload {
    /// Event type
    event_type: String,
    /// Download ID if applicable
    download_id: Option<u64>,
    /// Timestamp when the event occurred
    timestamp: String,
    /// Full event data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/// Webhook delivery system
pub struct WebhookDelivery {
    /// HTTP client for sending webhooks
    client: Client,
    /// Registered webhooks
    webhooks: Arc<RwLock<Vec<WebhookConfig>>>,
    /// Channel for queuing webhook deliveries
    tx: mpsc::UnboundedSender<(WebhookConfig, DownloadEvent)>,
}

impl std::fmt::Debug for WebhookDelivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookDelivery")
            .field("webhooks_count", &"<async>")
            .finish()
    }
}

impl WebhookDelivery {
    /// Creates a new webhook delivery system
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        let (tx, mut rx) = mpsc::unbounded_channel::<(WebhookConfig, DownloadEvent)>();

        let webhooks = Arc::new(RwLock::new(Vec::new()));

        let client_clone = client.clone();

        // Spawn worker task to process webhook deliveries
        tokio::spawn(async move {
            while let Some((config, event)) = rx.recv().await {
                let client = client_clone.clone();
                tokio::spawn(async move {
                    Self::deliver_webhook(client, config, event).await;
                });
            }
        });

        Self {
            client,
            webhooks,
            tx,
        }
    }

    /// Registers a new webhook
    ///
    /// # Arguments
    ///
    /// * `config` - The webhook configuration
    pub async fn register(&self, config: WebhookConfig) {
        let mut webhooks = self.webhooks.write().await;
        webhooks.push(config);
    }

    /// Processes an event and delivers it to matching webhooks
    ///
    /// # Arguments
    ///
    /// * `event` - The event to deliver
    pub async fn process_event(&self, event: &DownloadEvent) {
        let webhooks = self.webhooks.read().await;

        for webhook in webhooks.iter() {
            if webhook.filter.matches(event) {
                let _ = self.tx.send((webhook.clone(), event.clone()));
            }
        }
    }

    /// Delivers a webhook with retry logic
    async fn deliver_webhook(client: Client, config: WebhookConfig, event: DownloadEvent) {
        let payload = WebhookPayload {
            event_type: event.event_type().to_string(),
            download_id: event.download_id(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            data: if config.include_full_data {
                serde_json::to_value(&event).ok()
            } else {
                None
            },
        };

        let mut attempt = 0;

        loop {
            let result = Self::send_webhook(&client, &config, &payload).await;

            match result {
                Ok(_) => {
                    #[cfg(feature = "tracing")]
                    tracing::debug!(
                        "Webhook delivered successfully to {} (attempt {})",
                        config.url,
                        attempt + 1
                    );
                    break;
                }
                Err(e) => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!(
                        "Webhook delivery failed to {} (attempt {}): {}",
                        config.url,
                        attempt + 1,
                        e
                    );

                    if !config.retry_strategy.should_retry(attempt) {
                        #[cfg(feature = "tracing")]
                        tracing::error!(
                            "Webhook delivery to {} failed after {} attempts",
                            config.url,
                            attempt + 1
                        );
                        break;
                    }

                    let delay = config.retry_strategy.delay_for_attempt(attempt);
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }

    /// Sends a single webhook request
    async fn send_webhook(
        client: &Client,
        config: &WebhookConfig,
        payload: &WebhookPayload,
    ) -> Result<(), String> {
        let mut request = match config.method {
            WebhookMethod::Post => client.post(&config.url),
            WebhookMethod::Put => client.put(&config.url),
            WebhookMethod::Patch => client.patch(&config.url),
        };

        // Add custom headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Add JSON content type
        request = request.header("Content-Type", "application/json");

        // Add payload
        request = request.json(payload);

        // Set timeout
        request = request.timeout(config.timeout);

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        // Check status code
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }

        Ok(())
    }

    /// Returns the number of registered webhooks
    pub async fn count(&self) -> usize {
        let webhooks = self.webhooks.read().await;
        webhooks.len()
    }

    /// Clears all registered webhooks
    pub async fn clear(&self) {
        let mut webhooks = self.webhooks.write().await;
        webhooks.clear();
    }
}

impl Default for WebhookDelivery {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WebhookDelivery {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            webhooks: self.webhooks.clone(),
            tx: self.tx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_config() {
        let config = WebhookConfig::new("https://example.com/webhook")
            .with_method(WebhookMethod::Post)
            .with_header("Authorization", "Bearer token")
            .with_filter(EventFilter::only_completed())
            .with_timeout(Duration::from_secs(5));

        assert_eq!(config.url(), "https://example.com/webhook");
        assert_eq!(config.method, WebhookMethod::Post);
        assert_eq!(
            config.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
        assert_eq!(config.timeout, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_webhook_delivery() {
        let delivery = WebhookDelivery::new();

        let config = WebhookConfig::new("https://example.com/webhook")
            .with_filter(EventFilter::only_completed());

        delivery.register(config).await;

        assert_eq!(delivery.count().await, 1);
    }
}
