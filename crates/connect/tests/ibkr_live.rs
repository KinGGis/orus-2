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
