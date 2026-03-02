use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::Serialize;
use tokio::sync::{RwLock, mpsc};

use crate::events::{DownloadEvent, EventFilter, RetryStrategy};
use crate::utils::retry::RetryPolicy;

/// HTTP method for webhook delivery
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default)]
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
    /// Retry policy
    retry_policy: RetryPolicy,
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
    ///
    /// # Returns
    ///
    /// A new WebhookConfig with default settings
    pub fn new(url: impl Into<String>) -> Self {
        let url_string = url.into();

        tracing::debug!(
            url = %url_string,
            "⚙️ Creating new WebhookConfig"
        );

        Self {
            url: url_string,
            method: WebhookMethod::default(),
            headers: HashMap::new(),
            filter: EventFilter::all(),
            retry_policy: RetryPolicy::default(),
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
        tracing::debug!("⚙️ Loading WebhookConfig from environment");

        let url = std::env::var("YTDLP_WEBHOOK_URL").ok()?;

        tracing::debug!(
            url = %url,
            "⚙️ Found YTDLP_WEBHOOK_URL in environment"
        );

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

        tracing::debug!(
            url = %config.url,
            method = ?config.method,
            timeout_secs = config.timeout.as_secs(),
            "✅ WebhookConfig created from environment"
        );

        Some(config)
    }

    /// Sets the HTTP method
    ///
    /// # Arguments
    ///
    /// * `method` - The HTTP method to use
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_method(mut self, method: WebhookMethod) -> Self {
        self.method = method;
        self
    }

    /// Adds a custom header.
    ///
    /// # Arguments
    ///
    /// * `key` - Header name
    /// * `value` - Header value
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Sets multiple headers at once.
    ///
    /// # Arguments
    ///
    /// * `headers` - Map of header names to values
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    /// Sets the event filter.
    ///
    /// # Arguments
    ///
    /// * `filter` - The event filter to apply
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn with_filter(mut self, filter: EventFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Sets the retry strategy.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The retry strategy to use for failed deliveries
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn with_retry_strategy(mut self, strategy: RetryStrategy) -> Self {
        self.retry_policy = RetryPolicy::builder()
            .max_attempts(strategy.max_attempts as u32)
            .initial_delay(strategy.initial_delay)
            .max_delay(strategy.max_delay)
            .backoff_factor(strategy.backoff_multiplier)
            .build();
        self
    }

    /// Sets the request timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The request timeout duration
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets whether to include full event data.
    ///
    /// # Arguments
    ///
    /// * `include` - Whether to include full event data or just a summary
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn with_full_data(mut self, include: bool) -> Self {
        self.include_full_data = include;
        self
    }

    /// Returns the URL.
    ///
    /// # Returns
    ///
    /// The webhook URL as a string slice.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the filter.
    ///
    /// # Returns
    ///
    /// A reference to the configured event filter.
    pub fn filter(&self) -> &EventFilter {
        &self.filter
    }
}

/// Webhook payload that will be sent
impl std::fmt::Display for WebhookMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Post => f.write_str("Post"),
            Self::Put => f.write_str("Put"),
            Self::Patch => f.write_str("Patch"),
        }
    }
}

impl std::fmt::Display for WebhookConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WebhookConfig(url={}, method={}, timeout={}s)",
            self.url,
            self.method,
            self.timeout.as_secs()
        )
    }
}

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
    client: Arc<Client>,
    /// Registered webhooks
    webhooks: Arc<RwLock<Vec<WebhookConfig>>>,
    /// Channel for queuing webhook deliveries
    tx: mpsc::Sender<(WebhookConfig, DownloadEvent)>,
}

impl WebhookDelivery {
    /// Creates a new webhook delivery system
    ///
    /// # Returns
    ///
    /// A new WebhookDelivery instance with a background worker
    pub fn new() -> Self {
        tracing::debug!("⚙️ Creating new WebhookDelivery system");

        let client = crate::utils::http::build_http_client(crate::utils::http::HttpClientConfig {
            timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        })
        .unwrap_or_else(|_| Arc::new(Client::new()));

        let (tx, mut rx) = mpsc::channel::<(WebhookConfig, DownloadEvent)>(1024);

        let webhooks = Arc::new(RwLock::new(Vec::new()));

        let client_clone = client.clone();

        tracing::debug!("⚙️ Spawning webhook delivery worker task");

        // Spawn worker task to process webhook deliveries with concurrency limit
        const MAX_CONCURRENT_DELIVERIES: usize = 16;
        let delivery_semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DELIVERIES));

        tokio::spawn(async move {
            tracing::debug!("⚙️ Webhook delivery worker started");

            while let Some((config, event)) = rx.recv().await {
                let client = client_clone.clone();
                let permit = delivery_semaphore.clone().acquire_owned().await;
                let Ok(permit) = permit else {
                    tracing::warn!("Webhook semaphore closed, stopping delivery worker");
                    break;
                };
                tokio::spawn(async move {
                    let _permit = permit;
                    Self::deliver_webhook(client, config, event).await;
                });
            }

            tracing::debug!("⚙️ Webhook delivery worker stopped");
        });

        Self { client, webhooks, tx }
    }

    /// Registers a new webhook
    ///
    /// # Arguments
    ///
    /// * `config` - The webhook configuration
    pub async fn register(&self, config: WebhookConfig) {
        tracing::debug!(
            url = %config.url,
            method = ?config.method,
            "🔔 Registering new webhook"
        );

        let mut webhooks = self.webhooks.write().await;
        webhooks.push(config);

        tracing::debug!(total_webhooks = webhooks.len(), "✅ Webhook registered");
    }

    /// Processes an event and delivers it to matching webhooks
    ///
    /// # Arguments
    ///
    /// * `event` - The event to deliver
    pub async fn process_event(&self, event: &DownloadEvent) {
        tracing::debug!(
            event_type = event.event_type(),
            download_id = event.download_id(),
            "🔔 Processing event for webhook delivery"
        );

        let webhooks = self.webhooks.read().await;
        let mut matched_count = 0;

        for webhook in webhooks.iter() {
            if webhook.filter.matches(event) {
                matched_count += 1;
                if let Err(e) = self.tx.try_send((webhook.clone(), event.clone())) {
                    tracing::warn!(error = %e, "Webhook channel full, dropping event");
                }
            }
        }

        tracing::debug!(
            event_type = event.event_type(),
            total_webhooks = webhooks.len(),
            matched_webhooks = matched_count,
            "✅ Event processed for webhook delivery"
        );
    }

    /// Returns the number of registered webhooks
    ///
    /// # Returns
    ///
    /// The total number of registered webhooks
    pub async fn count(&self) -> usize {
        let webhooks = self.webhooks.read().await;
        webhooks.len()
    }

    /// Clears all registered webhooks
    pub async fn clear(&self) {
        tracing::debug!("⚙️ Clearing all webhooks");

        let mut webhooks = self.webhooks.write().await;
        let count = webhooks.len();
        webhooks.clear();

        tracing::debug!(webhooks_cleared = count, "✅ All webhooks cleared");
    }

    /// Delivers a webhook with retry logic
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client for sending requests
    /// * `config` - Webhook configuration
    /// * `event` - Event to deliver
    async fn deliver_webhook(client: Arc<Client>, config: WebhookConfig, event: DownloadEvent) {
        tracing::debug!(
            url = %config.url,
            event_type = event.event_type(),
            download_id = event.download_id(),
            "🔔 Starting webhook delivery"
        );

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

        let policy = config.retry_policy.clone();

        let result = policy
            .execute_with_condition(
                || {
                    let client = &client;
                    let config = &config;
                    let payload = &payload;
                    async move { Self::send_webhook(client, config, payload).await }
                },
                |e: &String| {
                    // Don't retry permanent 4xx client errors (except 429 Too Many Requests)
                    if e.starts_with("HTTP 4") && !e.starts_with("HTTP 429") {
                        return false;
                    }
                    true
                },
            )
            .await;

        match result {
            Ok(_) => {
                tracing::debug!(url = config.url, "✅ Webhook delivered successfully");
            }
            Err(e) => {
                tracing::error!(url = config.url, error = %e, "Webhook delivery failed after retries");
            }
        }
    }

    /// Sends a single webhook request
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client
    /// * `config` - Webhook configuration
    /// * `payload` - Webhook payload to send
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Err with error message on failure
    async fn send_webhook(client: &Client, config: &WebhookConfig, payload: &WebhookPayload) -> Result<(), String> {
        tracing::debug!(
            url = %config.url,
            method = ?config.method,
            event_type = %payload.event_type,
            "🔔 Sending webhook request"
        );

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
        let response = request.send().await.map_err(|e| format!("Request failed: {}", e))?;

        // Check status code
        if !response.status().is_success() {
            let status = response.status();

            tracing::warn!(
                url = %config.url,
                status_code = status.as_u16(),
                "🔔 Webhook request failed"
            );

            return Err(format!("HTTP {}", status));
        }

        tracing::debug!(
            url = %config.url,
            status_code = response.status().as_u16(),
            "✅ Webhook request succeeded"
        );

        Ok(())
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

impl std::fmt::Debug for WebhookDelivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookDelivery")
            .field("webhooks_count", &"<async>")
            .finish()
    }
}

impl std::fmt::Display for WebhookDelivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WebhookDelivery")
    }
}
