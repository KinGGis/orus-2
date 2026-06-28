use uuid::Uuid;

use wealthfolio_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID;
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::{Error, Result};

pub const TOTAL_PORTFOLIO_ACCOUNT_UUID: &str = "81edc1c4-ee7e-451c-be90-413cc912c3a4";
pub const TOTAL_PORTFOLIO_ACCOUNT_NAME: &str = "Total Portfolio";

pub fn total_portfolio_account_uuid() -> Uuid {
    Uuid::parse_str(TOTAL_PORTFOLIO_ACCOUNT_UUID).expect("valid TOTAL portfolio UUID")
}

pub fn is_total_portfolio_account_uuid(account_id: Uuid) -> bool {
    account_id == total_portfolio_account_uuid()
}

pub fn parse_account_id(value: &str) -> Result<Uuid> {
    if value.eq_ignore_ascii_case(PORTFOLIO_TOTAL_ACCOUNT_ID) {
        return Ok(total_portfolio_account_uuid());
    }

    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid account_id UUID: {err}"
        )))
    })
}

pub fn account_id_to_domain(account_id: Uuid) -> String {
    if is_total_portfolio_account_uuid(account_id) {
        PORTFOLIO_TOTAL_ACCOUNT_ID.to_string()
    } else {
        account_id.to_string()
    }
}