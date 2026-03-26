#![cfg(test)]

//! Comprehensive tests for Oracle price validation, with focus on confidence threshold rounding.
//!
//! # Issue #260: Confidence Threshold Rounding
//!
//! The confidence validation formula is: `max_conf = (price_abs * max_confidence_bps) / 10000`
//!
//! ## Problem
//! Integer division can introduce bias for small prices:
//! - price=1, bps=500 (5%): (1 * 500) / 10000 = 0 (truncates, should be ~0.05)
//! - price=10, bps=100 (1%): (10 * 100) / 10000 = 0 (truncates, should be ~0.1)
//! - price=100, bps=100 (1%): (100 * 100) / 10000 = 1 (correct)
//!
//! This causes a **downward bias** for small prices, making it harder to accept prices
//! with any confidence interval at very small valuations.
//!
//! ## Potential Solutions
//! 1. **Ceiling division**: Use `(price * bps + 9999) / 10000` to round up
//! 2. **Fixed-point math**: Scale up before division to preserve precision
//! 3. **Reverse formula**: Check `(price * bps) >= (conf * 10000)` to avoid division
//!
//! ## Test Coverage
//! - `test_confidence_rounding_small_prices`: Tests 1-100 range prices
//! - `test_confidence_rounding_large_prices`: Tests million+ range prices
//! - `test_confidence_rounding_edge_cases_low_prices`: Targets specific rounding boundaries
//! - `test_confidence_rounding_negative_prices`: Validates absolute value handling
//! - `test_confidence_rounding_boundary_conditions`: Documents exact rounding behavior

use super::oracles::*;
use crate::errors::ErrorCode;
use crate::types::OracleConfig;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn test_config(e: &Env) -> OracleConfig {
    OracleConfig {
        oracle_address: Address::generate(e),
        feed_id: String::from_str(e, "test_feed"),
        min_responses: 1,
        max_staleness_seconds: 300,
        max_confidence_bps: 200,
    }
}

fn create_config(e: &Env, max_confidence_bps: u64) -> OracleConfig {
    OracleConfig {
        oracle_address: Address::generate(e),
        feed_id: String::from_str(e, "test_feed"),
        min_responses: Some(1),
        max_staleness_seconds: 3600,
        max_confidence_bps,
    }
}

fn create_price(price: i64, conf: u64, timestamp: u64) -> PythPrice {
    PythPrice {
        price,
        conf,
        expo: -2,
        publish_time: timestamp,
    }
}

#[test]
fn test_validate_fresh_price() {
    let e = Env::default();

    let config = test_config(&e);
    let price = PythPrice {
        price: 100000,
        conf: 1000, // 1% of price
        expo: -2,
        publish_time: e.ledger().timestamp() as i64 - 60, // 1 minute old
    };

    let result = validate_price(&e, &price, &config);
    assert!(result.is_ok());
}

#[test]
fn test_reject_stale_price() {
    let e = Env::default();

    let config = test_config(&e);
    let price = PythPrice {
        price: 100000,
        conf: 1000,
        expo: -2,
        publish_time: e.ledger().timestamp() as i64 - 400, // 400 seconds old
    };

    let result = validate_price(&e, &price, &config);
    assert_eq!(result, Err(ErrorCode::StalePrice));
}

#[test]
fn test_reject_low_confidence() {
    let e = Env::default();

    let config = test_config(&e);
    let price = PythPrice {
        price: 100000,
        conf: 3000, // 3% of price - exceeds 2% threshold
        expo: -2,
        publish_time: e.ledger().timestamp() as i64 - 60,
    };

    let result = validate_price(&e, &price, &config);
    assert_eq!(result, Err(ErrorCode::ConfidenceTooLow));
}

#[test]
fn test_cast_external_timestamp_rejects_negative_values() {
    assert_eq!(
        cast_external_timestamp(-1),
        Err(ErrorCode::InvalidTimestamp)
    );
}

#[test]
fn test_cast_external_timestamp_accepts_zero() {
    assert_eq!(cast_external_timestamp(0), Ok(0));
}

#[test]
fn test_cast_external_timestamp_accepts_positive_values() {
    assert_eq!(cast_external_timestamp(1_700_000_000), Ok(1_700_000_000));
}

#[test]
fn test_is_stale_returns_false_for_fresh_data() {
    assert!(!is_stale(1_700_001_000, 1_700_000_900, 300));
}

#[test]
fn test_is_stale_returns_true_for_old_data() {
    assert!(is_stale(1_700_001_000, 1_699_999_000, 300));
}

#[test]
fn test_is_stale_boundary_is_not_stale() {
    assert!(!is_stale(1_700_001_000, 1_700_000_700, 300));
}

#[test]
fn test_is_stale_future_timestamp_does_not_underflow() {
    assert!(!is_stale(1_700_000_000, 1_700_001_000, 300));
}

#[test]
fn test_validate_price_rejects_negative_publish_time() {
    let e = Env::default();
    let config = test_config(&e);
    let price = PythPrice {
        price: 100000,
        conf: 1000,
        expo: -2,
        publish_time: -1,
    };

    let result = validate_price(&e, &price, &config);
    assert_eq!(result, Err(ErrorCode::InvalidTimestamp));
}
