use std::time::Duration;

use yt_dlp::utils::retry::RetryPolicy;

// ============================== RetryPolicy defaults ==============================

#[test]
fn check_retry_policy_default_values_match_expectations() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.max_attempts(), 3);
    assert_eq!(policy.initial_delay(), Duration::from_millis(500));
    assert_eq!(policy.max_delay(), Duration::from_secs(60));
    assert!((policy.backoff_factor() - 2.0).abs() < f64::EPSILON);
    assert!(policy.has_jitter());
}

#[test]
fn build_retry_policy_with_custom_values_succeeds() {
    let policy = RetryPolicy::builder()
        .max_attempts(5)
        .initial_delay(Duration::from_millis(100))
        .max_delay(Duration::from_secs(30))
        .backoff_factor(3.0)
        .jitter(false)
        .build();

    assert_eq!(policy.max_attempts(), 5);
    assert_eq!(policy.initial_delay(), Duration::from_millis(100));
    assert_eq!(policy.max_delay(), Duration::from_secs(30));
    assert!((policy.backoff_factor() - 3.0).abs() < f64::EPSILON);
    assert!(!policy.has_jitter());
}

#[test]
fn create_retry_policy_via_new_matches_default() {
    let from_new = RetryPolicy::new();
    let from_default = RetryPolicy::default();
    assert_eq!(from_new.max_attempts(), from_default.max_attempts());
    assert_eq!(from_new.initial_delay(), from_default.initial_delay());
    assert_eq!(from_new.max_delay(), from_default.max_delay());
    assert!((from_new.backoff_factor() - from_default.backoff_factor()).abs() < f64::EPSILON);
}

// ============================== RetryPolicy Display ==============================

#[test]
fn display_retry_policy_shows_key_fields() {
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .backoff_factor(2.0)
        .jitter(false)
        .build();
    let display = format!("{}", policy);
    assert!(display.contains("3"));
    assert!(display.contains("2"));
    assert!(display.contains("false"));
}

// ============================== RetryPolicy::execute (pure logic, no sleep) ==============================

#[tokio::test]
async fn retry_execute_succeeds_on_first_attempt() {
    let policy = RetryPolicy::builder().max_attempts(3).jitter(false).build();

    let mut call_count = 0usize;
    let result = policy
        .execute(|| {
            call_count += 1;
            async { Ok::<_, String>("ok") }
        })
        .await;

    assert_eq!(result.unwrap(), "ok");
    assert_eq!(call_count, 1);
}

#[tokio::test]
async fn retry_execute_all_fail_returns_last_error() {
    let policy = RetryPolicy::builder()
        .max_attempts(2)
        .initial_delay(Duration::from_millis(1))
        .jitter(false)
        .build();

    let mut call_count = 0usize;
    let result = policy
        .execute(|| {
            call_count += 1;
            async { Err::<String, _>("fail".to_string()) }
        })
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "fail");
    assert_eq!(call_count, 2);
}

#[tokio::test]
async fn retry_execute_with_condition_stops_on_non_retryable() {
    let policy = RetryPolicy::builder()
        .max_attempts(5)
        .initial_delay(Duration::from_millis(1))
        .jitter(false)
        .build();

    let mut call_count = 0usize;
    let result = policy
        .execute_with_condition(
            || {
                call_count += 1;
                async { Err::<String, _>("permanent error".to_string()) }
            },
            |_| false, // never retryable
        )
        .await;

    assert!(result.is_err());
    // Should only try once since the condition says not retryable
    assert_eq!(call_count, 1);
}

#[tokio::test]
async fn retry_execute_succeeds_after_initial_failure() {
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .initial_delay(Duration::from_millis(1))
        .jitter(false)
        .build();

    let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let call_count_clone = call_count.clone();

    let result = policy
        .execute(move || {
            let count = call_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            async move {
                if count < 2 {
                    Err::<String, _>("temporary error".to_string())
                } else {
                    Ok("success".to_string())
                }
            }
        })
        .await;

    assert_eq!(result.unwrap(), "success");
    assert_eq!(call_count.load(std::sync::atomic::Ordering::Relaxed), 3);
}
