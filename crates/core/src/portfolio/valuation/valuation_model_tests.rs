//! Tests for `DailyAccountValuation::stable_id`.
//!
//! The Postgres backend stores `wf_daily_account_valuation.id` as a UUID while
//! the calculator builds `"{account_id}_{date}"`. `stable_id` is the bridge, so
//! these tests pin the properties the upsert path depends on.

use super::valuation_model::DailyAccountValuation;
use crate::portfolio::snapshot::AccountStateSnapshot;
use chrono::NaiveDate;
use uuid::Uuid;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

#[test]
fn stable_id_is_a_valid_uuid() {
    let id = DailyAccountValuation::stable_id("acc_1", date(2026, 8, 15));
    assert!(Uuid::parse_str(&id).is_ok(), "stable_id must parse as UUID");
}

#[test]
fn stable_id_is_version_5_shaped() {
    let id = DailyAccountValuation::stable_id("acc_1", date(2026, 8, 15));
    let uuid = Uuid::parse_str(&id).expect("valid UUID");
    assert_eq!(uuid.get_version_num(), 5);
    assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
}

#[test]
fn stable_id_is_deterministic() {
    // Upserts rely on this: recalculating a day must target the same row
    // instead of appending a duplicate.
    let first = DailyAccountValuation::stable_id("acc_1", date(2026, 8, 15));
    let second = DailyAccountValuation::stable_id("acc_1", date(2026, 8, 15));
    assert_eq!(first, second);
}

#[test]
fn stable_id_differs_per_date() {
    let day_one = DailyAccountValuation::stable_id("acc_1", date(2026, 8, 15));
    let day_two = DailyAccountValuation::stable_id("acc_1", date(2026, 8, 16));
    assert_ne!(day_one, day_two);
}

#[test]
fn stable_id_differs_per_account() {
    let account_one = DailyAccountValuation::stable_id("acc_1", date(2026, 8, 15));
    let account_two = DailyAccountValuation::stable_id("acc_2", date(2026, 8, 15));
    assert_ne!(account_one, account_two);
}

#[test]
fn stable_id_handles_the_total_portfolio_account() {
    let id = DailyAccountValuation::stable_id("TOTAL", date(2026, 8, 15));
    assert!(
        Uuid::parse_str(&id).is_ok(),
        "the synthetic TOTAL account must produce a usable id too"
    );
}

#[test]
fn stable_id_does_not_collide_with_snapshot_stable_id() {
    // Both tables key on account + date. A shared namespace would make a
    // valuation id equal to its snapshot id, which is confusing at best and a
    // real collision if the two ever share a table.
    let account_id = "acc_1";
    let day = date(2026, 8, 15);
    assert_ne!(
        DailyAccountValuation::stable_id(account_id, day),
        AccountStateSnapshot::stable_id(account_id, day)
    );
}
