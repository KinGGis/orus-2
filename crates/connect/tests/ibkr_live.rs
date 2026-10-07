//! Live smoke test against IBKR's MCP server.
//!
//! Ignored by default: it needs a real refresh token and network access. Run it
//! with a token obtained from the OAuth consent flow:
//!
//! ```text
//! IBKR_CLIENT_ID=... IBKR_REFRESH_TOKEN=... \
//!     cargo test -p wealthfolio-connect --test ibkr_live -- --ignored --nocapture
//! ```
//!
//! The refresh token rotates on use, so the test prints the replacement.

use std::sync::{Arc, Mutex};

use wealthfolio_connect::broker::BrokerApiClient;
use wealthfolio_connect::ibkr::{http_client, IbkrMcpClient, IbkrTokenManager, IbkrTokenStore};
use wealthfolio_core::errors::Result;

struct EnvTokenStore {
    token: Mutex<Option<String>>,
}

impl IbkrTokenStore for EnvTokenStore {
    fn load_refresh_token(&self) -> Result<Option<String>> {
        Ok(self.token.lock().unwrap().clone())
    }

    fn save_refresh_token(&self, token: &str) -> Result<()> {
        println!("ROTATED_REFRESH_TOKEN={token}");
        *self.token.lock().unwrap() = Some(token.to_string());
        Ok(())
    }

    fn clear_refresh_token(&self) -> Result<()> {
        *self.token.lock().unwrap() = None;
        Ok(())
    }
}

#[tokio::test]
#[ignore = "requires live IBKR credentials and network access"]
async fn fetches_real_holdings_and_trades() {
    let client_id = std::env::var("IBKR_CLIENT_ID").expect("IBKR_CLIENT_ID");
    let refresh_token = std::env::var("IBKR_REFRESH_TOKEN").expect("IBKR_REFRESH_TOKEN");

    let store = Arc::new(EnvTokenStore {
        token: Mutex::new(Some(refresh_token)),
    });
    let tokens = Arc::new(IbkrTokenManager::new(http_client(), client_id, store));
    let client = IbkrMcpClient::new(http_client(), tokens);

    let accounts = client.list_accounts(None).await.expect("list_accounts");
    assert_eq!(accounts.len(), 1);
    let account = &accounts[0];
    let account_id = account.id.clone().expect("account id");
    println!(
        "account currency={:?} nav={:?}",
        account.currency,
        account
            .balance
            .as_ref()
            .and_then(|b| b.total.as_ref())
            .and_then(|t| t.amount)
    );

    let holdings = client
        .get_account_holdings(&account_id)
        .await
        .expect("holdings");
    let positions = holdings.positions.unwrap_or_default();
    let balances = holdings.balances.unwrap_or_default();
    println!(
        "positions={} cash_balances={}",
        positions.len(),
        balances.len()
    );
    assert!(!positions.is_empty(), "IBKR reported no open positions");
    assert!(
        balances
            .iter()
            .all(|b| b.currency.as_ref().and_then(|c| c.code.as_deref()) != Some("BASE")),
        "the BASE aggregate row must be filtered out"
    );

    let page = client
        .get_account_activities(&account_id, None, None, Some(0), Some(50))
        .await
        .expect("activities");
    let total = page.pagination.as_ref().and_then(|p| p.total).unwrap_or(0);
    println!("activities total={total} first_page={}", page.data.len());
    assert!(total > 0, "IBKR reported no trades");
    assert_eq!(page.data.len(), 50.min(total as usize));

    // The five trade windows overlap, so a leaked duplicate would double-count.
    let all = client
        .get_account_activities(&account_id, None, None, Some(0), Some(100_000))
        .await
        .expect("all activities");
    let unique: std::collections::HashSet<_> = all
        .data
        .iter()
        .filter_map(|a| a.source_record_id.clone())
        .collect();
    assert_eq!(
        unique.len(),
        all.data.len(),
        "deduplication by trade_id failed"
    );

    // Allocations must reconcile with the NAV the broker reports, which is the
    // whole point of sourcing them from IBKR instead of our taxonomy join.
    let allocations = client.allocations().await.expect("allocations");
    println!(
        "allocations total_value={} asset_classes={} sectors={} regions={} instruments={} countries={}",
        allocations.total_value,
        allocations.asset_classes.categories.len(),
        allocations.sectors.categories.len(),
        allocations.regions.categories.len(),
        allocations.security_types.categories.len(),
        allocations
            .custom_groups
            .first()
            .map(|g| g.categories.len())
            .unwrap_or(0),
    );

    let nav = account
        .balance
        .as_ref()
        .and_then(|b| b.total.as_ref())
        .and_then(|t| t.amount)
        .expect("account nav");
    let nav = rust_decimal::Decimal::from_f64_retain(nav).expect("nav as decimal");

    let drift = (allocations.total_value - nav).abs();
    assert!(
        drift <= rust_decimal::Decimal::new(5, 0),
        "allocation total {} drifted from NAV {nav} by {drift}",
        allocations.total_value
    );

    for taxonomy in [
        &allocations.asset_classes,
        &allocations.sectors,
        &allocations.regions,
        &allocations.security_types,
    ] {
        assert!(
            !taxonomy.categories.is_empty(),
            "{} came back empty",
            taxonomy.taxonomy_id
        );
        assert!(
            taxonomy.categories.iter().all(|c| c.value > rust_decimal::Decimal::ZERO),
            "{} contains a non-positive slice",
            taxonomy.taxonomy_id
        );
        let sum: rust_decimal::Decimal = taxonomy.categories.iter().map(|c| c.percentage).sum();
        assert!(
            (sum - rust_decimal::Decimal::new(100, 0)).abs() <= rust_decimal::Decimal::new(5, 1),
            "{} percentages summed to {sum}",
            taxonomy.taxonomy_id
        );
    }
}

/// Runs the broker-agnostic mapping layer over live IBKR payloads.
///
/// Every trade must survive `map_broker_activity`, and every position must
/// carry the broker's own price. A regression in either place silently empties
/// the activity list or values holdings at zero.
#[tokio::test]
#[ignore = "requires live IBKR credentials and network access"]
async fn maps_every_live_trade_and_prices_every_position() {
    use wealthfolio_connect::broker::mapping::map_broker_activity;

    let client_id = std::env::var("IBKR_CLIENT_ID").expect("IBKR_CLIENT_ID");
    let refresh_token = std::env::var("IBKR_REFRESH_TOKEN").expect("IBKR_REFRESH_TOKEN");

    let store = Arc::new(EnvTokenStore {
        token: Mutex::new(Some(refresh_token)),
    });
    let tokens = Arc::new(IbkrTokenManager::new(http_client(), client_id, store));
    let client = IbkrMcpClient::new(http_client(), tokens);

    let accounts = client.list_accounts(None).await.expect("list_accounts");
    let account_id = accounts[0].id.clone().expect("account id");

    let all = client
        .get_account_activities(&account_id, None, None, Some(0), Some(100_000))
        .await
        .expect("all activities");

    let mut mapped = 0usize;
    let mut dropped: Vec<String> = Vec::new();
    let mut needs_review = 0usize;
    for activity in &all.data {
        match map_broker_activity(activity, "test-account", Some("GBP"), Some("GBP")) {
            Some(new_activity) => {
                mapped += 1;
                if new_activity.needs_review.unwrap_or(false) {
                    needs_review += 1;
                }
            }
            None => dropped.push(format!("{:?}", activity.id)),
        }
    }
    println!(
        "activities fetched={} mapped={} dropped={} needs_review={}",
        all.data.len(),
        mapped,
        dropped.len(),
        needs_review
    );
    assert!(
        dropped.is_empty(),
        "map_broker_activity dropped {} activities: {:?}",
        dropped.len(),
        &dropped[..dropped.len().min(10)]
    );
    assert_eq!(
        needs_review, 0,
        "live trades should map cleanly without review"
    );

    let holdings = client
        .get_account_holdings(&account_id)
        .await
        .expect("holdings");
    let positions = holdings.positions.unwrap_or_default();

    let unpriced: Vec<String> = positions
        .iter()
        .filter(|p| p.price.unwrap_or(0.0) <= 0.0)
        .map(|p| {
            p.symbol
                .as_ref()
                .and_then(|s| s.symbol.as_ref())
                .and_then(|s| s.symbol.clone())
                .unwrap_or_else(|| "<unnamed>".to_string())
        })
        .collect();
    println!(
        "positions={} priced={}",
        positions.len(),
        positions.len() - unpriced.len()
    );
    assert!(
        unpriced.is_empty(),
        "IBKR positions missing a broker price: {unpriced:?}"
    );

    // Non-USD listings are the ones the market-data provider cannot resolve
    // from a bare ticker, so they are exactly the ones that need these prices.
    let foreign = positions
        .iter()
        .filter(|p| {
            p.currency
                .as_ref()
                .and_then(|c| c.code.as_deref())
                .is_some_and(|c| c != "USD")
        })
        .count();
    println!("non-USD positions carrying a price={foreign}");
    assert!(foreign > 0, "expected non-USD listings in this account");
}

/// Every held instrument must come back with a usable year of daily closes.
///
/// This is the series that values the portfolio on past days. If IBKR refuses
/// a contract, that position falls back to being worth zero in the history,
/// which is the exact defect this import exists to remove.
#[tokio::test]
#[ignore = "requires live IBKR credentials and network access"]
async fn supplies_a_price_history_for_every_position() {
    let client_id = std::env::var("IBKR_CLIENT_ID").expect("IBKR_CLIENT_ID");
    let refresh_token = std::env::var("IBKR_REFRESH_TOKEN").expect("IBKR_REFRESH_TOKEN");

    let store = Arc::new(EnvTokenStore {
        token: Mutex::new(Some(refresh_token)),
    });
    let tokens = Arc::new(IbkrTokenManager::new(http_client(), client_id, store));
    let client = IbkrMcpClient::new(http_client(), tokens);

    let accounts = client.list_accounts(None).await.expect("list_accounts");
    let account_id = accounts[0].id.clone().expect("account id");

    let holdings = client
        .get_account_holdings(&account_id)
        .await
        .expect("holdings");
    let positions = holdings.positions.unwrap_or_default();

    let mut without_history: Vec<String> = Vec::new();
    for position in &positions {
        let symbol = position
            .symbol
            .as_ref()
            .and_then(|s| s.symbol.as_ref())
            .and_then(|s| s.symbol.clone())
            .unwrap_or_else(|| "<unnamed>".to_string());

        let Some(contract_id) = position
            .symbol
            .as_ref()
            .and_then(|s| s.id.as_ref())
            .and_then(|id| id.parse::<i64>().ok())
        else {
            without_history.push(format!("{symbol} (no contract id)"));
            continue;
        };

        match client.price_history(contract_id, "STK", "FIVE_YEARS").await {
            Ok(bars) if bars.len() >= 100 => {
                let first = bars.first().unwrap();
                let last = bars.last().unwrap();
                println!(
                    "{symbol}: {} bars, {} .. {} (close {} -> {})",
                    bars.len(),
                    first.date,
                    last.date,
                    first.close,
                    last.close
                );
            }
            Ok(bars) => without_history.push(format!("{symbol} (only {} bars)", bars.len())),
            Err(err) => without_history.push(format!("{symbol} ({err})")),
        }
    }

    assert!(
        without_history.is_empty(),
        "positions without a usable price history: {without_history:?}"
    );
}
