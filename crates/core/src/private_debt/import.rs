//! Import of a lending platform's exports (e.g. Maclear): the list of loans
//! with their terms, and the account statement.
//!
//! Each loan becomes a BOND asset carrying its terms ([`BondSpec`]), identified
//! by the instrument symbol `{PLATFORM}-{project id}` so that a later import
//! updates it instead of duplicating it. Each statement line becomes an
//! activity keyed by an idempotency key built from its content: importing the
//! same statement again, or a newer one overlapping it, only adds the lines
//! not booked yet. The real activities then confirm the estimated flows of the
//! loans' schedules at the next portfolio update (see [`super::debt_sync`]).

use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::activities::{
    ActivityBulkMutationRequest, ActivityServiceTrait, NewActivity, SymbolInput, ACTIVITY_TYPE_BUY,
    ACTIVITY_TYPE_DEPOSIT, ACTIVITY_TYPE_INTEREST, ACTIVITY_TYPE_SELL, ACTIVITY_TYPE_WITHDRAWAL,
};
use crate::assets::{
    add_months, Asset, AssetKind, AssetServiceTrait, BondSpec, InstrumentType, NewAsset, QuoteMode,
    UpdateAssetProfile,
};
use crate::errors::{Error, Result, ValidationError};
use crate::taxonomies::{NewAssetTaxonomyAssignment, TaxonomyServiceTrait};

pub const IMPORT_SOURCE_SYSTEM: &str = "PRIVATE_DEBT_IMPORT";
const DEFAULT_VALUATION_METHOD: &str = "PAR";
const QUANTITY_DP: u32 = 8;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateDebtImportRequest {
    pub account_id: String,
    /// Platform code, e.g. "MACLEAR".
    pub platform: String,
    #[serde(default)]
    pub loans: Vec<ImportedLoan>,
    #[serde(default)]
    pub transactions: Vec<ImportedTransaction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedLoan {
    pub project_id: String,
    pub borrower: String,
    pub purchase_date: NaiveDate,
    pub next_payment_date: Option<NaiveDate>,
    /// Annual interest rate, in percent (14.6 = 14.6 %).
    pub rate: Decimal,
    pub loyalty_bonus: Option<Decimal>,
    pub project_type: Option<String>,
    pub total_months: u32,
    pub remaining_months: u32,
    pub country_code: Option<String>,
    pub status: Option<String>,
    /// Principal held.
    pub nominal: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedTransaction {
    pub timestamp: NaiveDateTime,
    /// Statement line type, e.g. "Interest payment".
    pub kind: String,
    pub project_id: Option<String>,
    /// Signed cash amount (negative = paid out of the account).
    pub amount: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateDebtImportSummary {
    pub loans_created: usize,
    pub loans_updated: usize,
    pub loans_unchanged: usize,
    pub activities_created: usize,
    /// Statement lines already booked by a previous import.
    pub activities_skipped: usize,
    /// Statement lines that are not cash movements (reservations, discount info).
    pub lines_ignored: usize,
    pub warnings: Vec<String>,
}

/// What a statement line is, by its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Buy,
    Interest,
    Principal,
    Deposit,
    Withdrawal,
    /// Not a cash movement: a reservation precedes the investment it becomes,
    /// and the secondary market "profit" is the discount already in the price.
    Ignored,
}

fn line_kind(kind: &str) -> Option<LineKind> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "investment" | "secondary market buy" => Some(LineKind::Buy),
        "interest payment" => Some(LineKind::Interest),
        "principal repayment" => Some(LineKind::Principal),
        "deposit" => Some(LineKind::Deposit),
        "withdrawal" => Some(LineKind::Withdrawal),
        "reserved" | "secondary market profit" => Some(LineKind::Ignored),
        _ => None,
    }
}

/// Maturity of a loan: the next payment followed by the remaining monthly
/// ones or, once nothing remains, the purchase date plus the loan period.
fn maturity_date(loan: &ImportedLoan) -> Option<NaiveDate> {
    match loan.next_payment_date {
        Some(next) if loan.remaining_months > 0 => add_months(next, loan.remaining_months - 1),
        _ => add_months(loan.purchase_date, loan.total_months),
    }
}

/// GICS sector of a platform project type, when it is unambiguous.
fn gics_sector(project_type: &str) -> Option<&'static str> {
    match project_type.trim().to_ascii_lowercase().as_str() {
        "loan originator" => Some("40"),
        "services"
        | "manufactory/production"
        | "packaging and distribution"
        | "construction/services" => Some("20"),
        "wholesale and services" => Some("25"),
        "agriculture" => Some("30"),
        _ => None,
    }
}

fn platform_code(platform: &str) -> String {
    platform.trim().to_ascii_uppercase()
}

fn loan_symbol(platform: &str, project_id: &str) -> String {
    format!(
        "{}-{}",
        platform_code(platform),
        project_id.trim().to_ascii_uppercase()
    )
}

/// Terms of the loan, on top of those already stored: the imported terms
/// replace theirs, while a valuation method or other terms set by the user
/// are kept.
fn loan_spec(loan: &ImportedLoan, maturity: NaiveDate, existing: Option<BondSpec>) -> BondSpec {
    let mut spec = existing.unwrap_or_default();
    spec.issue_date = Some(loan.purchase_date);
    spec.maturity_date = Some(maturity);
    spec.coupon_rate = Some(loan.rate / Decimal::ONE_HUNDRED);
    spec.coupon_frequency = Some("MONTHLY".to_string());
    spec.face_value = Some(Decimal::ONE);
    spec.borrower = Some(loan.borrower.trim().to_string());
    spec.rate_type.get_or_insert_with(|| "FIXED".to_string());
    spec.amortization_type
        .get_or_insert_with(|| "BULLET".to_string());
    spec.day_count_convention
        .get_or_insert_with(|| "30/360".to_string());
    spec.valuation_method
        .get_or_insert_with(|| DEFAULT_VALUATION_METHOD.to_string());
    spec
}

fn loan_metadata(
    platform: &str,
    loan: &ImportedLoan,
    spec: &BondSpec,
    existing: Option<&Value>,
) -> Value {
    let mut metadata = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(Map::new);
    metadata.insert("bond".to_string(), json!(spec));
    metadata.insert(
        "privateDebt".to_string(),
        json!({
            "platform": platform_code(platform),
            "projectId": loan.project_id.trim(),
            "projectType": loan.project_type.as_deref().map(str::trim),
            "countryCode": loan.country_code.as_deref().map(str::trim),
            "status": loan.status.as_deref().map(str::trim),
            "loyaltyBonus": loan.loyalty_bonus,
        }),
    );
    Value::Object(metadata)
}

fn loan_name(loan: &ImportedLoan) -> String {
    format!("{} ({})", loan.borrower.trim(), loan.project_id.trim())
}

/// Idempotency key of a statement line. Identical lines get a sequence
/// suffix, so that they are all booked once.
fn line_key(platform: &str, line: &ImportedTransaction, activity_type: &str) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        platform_code(platform),
        line.timestamp.format("%Y-%m-%dT%H:%M:%S"),
        activity_type,
        line.project_id.as_deref().unwrap_or("").trim(),
        line.amount.normalize()
    )
}

/// Activities to create for the statement lines not booked yet.
///
/// `loan_assets` maps each project id to the asset of the loan, and `nominals`
/// to the principal held, from which the price paid for it is derived.
fn plan_activities(
    request: &PrivateDebtImportRequest,
    loan_assets: &HashMap<String, String>,
    nominals: &HashMap<String, Decimal>,
    booked_keys: &HashSet<String>,
    summary: &mut PrivateDebtImportSummary,
) -> Vec<NewActivity> {
    let project = |line: &ImportedTransaction| {
        line.project_id
            .as_deref()
            .map(|p| p.trim().to_ascii_uppercase())
            .filter(|p| !p.is_empty())
    };

    // Cash paid for each loan: a price below par is a secondary market discount.
    let mut paid: HashMap<String, Decimal> = HashMap::new();
    for line in &request.transactions {
        if line_kind(&line.kind) == Some(LineKind::Buy) {
            if let Some(project_id) = project(line) {
                *paid.entry(project_id).or_default() += line.amount.abs();
            }
        }
    }

    let mut occurrences: HashMap<String, usize> = HashMap::new();
    let mut activities = Vec::new();
    for line in &request.transactions {
        let Some(kind) = line_kind(&line.kind) else {
            summary.warnings.push(format!(
                "Unknown statement line type \"{}\" ({}), not imported",
                line.kind.trim(),
                line.timestamp
            ));
            continue;
        };
        let activity_type = match kind {
            LineKind::Ignored => {
                summary.lines_ignored += 1;
                continue;
            }
            LineKind::Buy => ACTIVITY_TYPE_BUY,
            LineKind::Interest => ACTIVITY_TYPE_INTEREST,
            LineKind::Principal => ACTIVITY_TYPE_SELL,
            LineKind::Deposit => ACTIVITY_TYPE_DEPOSIT,
            LineKind::Withdrawal => ACTIVITY_TYPE_WITHDRAWAL,
        };
        let amount = line.amount.abs();

        let (asset_id, quantity, unit_price) = match kind {
            LineKind::Deposit | LineKind::Withdrawal => (None, None, None),
            _ => {
                let Some(project_id) = project(line) else {
                    summary.warnings.push(format!(
                        "{} line of {} has no project, not imported",
                        line.kind.trim(),
                        line.timestamp
                    ));
                    continue;
                };
                let Some(asset_id) = loan_assets.get(&project_id) else {
                    summary.warnings.push(format!(
                        "{} line of {} refers to unknown loan {}, not imported",
                        line.kind.trim(),
                        line.timestamp,
                        project_id
                    ));
                    continue;
                };
                match kind {
                    LineKind::Buy => {
                        let nominal = nominals.get(&project_id).copied();
                        let total_paid = paid.get(&project_id).copied().unwrap_or(amount);
                        let (quantity, price) = match nominal {
                            Some(nominal) if nominal > Decimal::ZERO && !total_paid.is_zero() => (
                                round(amount * nominal / total_paid),
                                round(total_paid / nominal),
                            ),
                            _ => (amount, Decimal::ONE),
                        };
                        (Some(asset_id.clone()), Some(quantity), Some(price))
                    }
                    LineKind::Principal => {
                        (Some(asset_id.clone()), Some(amount), Some(Decimal::ONE))
                    }
                    _ => (Some(asset_id.clone()), None, None),
                }
            }
        };

        let base_key = line_key(&request.platform, line, activity_type);
        let n = occurrences.entry(base_key.clone()).or_insert(0);
        *n += 1;
        let key = if *n == 1 {
            base_key
        } else {
            format!("{}:{}", base_key, n)
        };
        if booked_keys.contains(&key) {
            summary.activities_skipped += 1;
            continue;
        }

        activities.push(NewActivity {
            id: None,
            account_id: request.account_id.clone(),
            symbol: asset_id.map(|id| SymbolInput {
                id: Some(id),
                ..Default::default()
            }),
            activity_type: activity_type.to_string(),
            subtype: None,
            activity_date: format!("{}Z", line.timestamp.format("%Y-%m-%dT%H:%M:%S")),
            quantity,
            unit_price,
            currency: line.currency.trim().to_ascii_uppercase(),
            fee: None,
            amount: Some(amount),
            status: None,
            notes: Some(
                format!(
                    "{} {}",
                    line.kind.trim(),
                    line.project_id.as_deref().unwrap_or("").trim()
                )
                .trim()
                .to_string(),
            ),
            fx_rate: None,
            metadata: None,
            needs_review: None,
            source_system: Some(IMPORT_SOURCE_SYSTEM.to_string()),
            source_record_id: Some(key.clone()),
            source_group_id: Some(platform_code(&request.platform)),
            idempotency_key: Some(key),
        });
    }
    activities
}

fn round(value: Decimal) -> Decimal {
    value
        .round_dp_with_strategy(QUANTITY_DP, RoundingStrategy::MidpointAwayFromZero)
        .normalize()
}

/// Imports the loans and statement lines of `request` into its account.
pub async fn import_private_debt(
    assets: &(dyn AssetServiceTrait + Send + Sync),
    activities: &(dyn ActivityServiceTrait + Send + Sync),
    taxonomies: &(dyn TaxonomyServiceTrait + Send + Sync),
    request: PrivateDebtImportRequest,
) -> Result<PrivateDebtImportSummary> {
    if request.account_id.trim().is_empty() {
        return Err(invalid("an account is required".to_string()));
    }
    if platform_code(&request.platform).is_empty() {
        return Err(invalid("a platform is required".to_string()));
    }
    let mut summary = PrivateDebtImportSummary::default();

    let mut by_symbol: HashMap<String, Asset> = assets
        .get_assets()?
        .into_iter()
        .filter(|asset| asset.instrument_type == Some(InstrumentType::Bond))
        .filter_map(|asset| Some((asset.instrument_symbol.clone()?, asset)))
        .collect();

    let mut loan_assets: HashMap<String, String> = HashMap::new();
    let mut nominals: HashMap<String, Decimal> = HashMap::new();
    for loan in &request.loans {
        let project_id = loan.project_id.trim().to_ascii_uppercase();
        let Some(maturity) = maturity_date(loan) else {
            summary.warnings.push(format!(
                "Loan {}: invalid maturity, not imported",
                project_id
            ));
            continue;
        };
        let symbol = loan_symbol(&request.platform, &project_id);
        let asset = match by_symbol.remove(&symbol) {
            Some(existing) => {
                let spec = loan_spec(loan, maturity, existing.bond_spec());
                let metadata =
                    loan_metadata(&request.platform, loan, &spec, existing.metadata.as_ref());
                let name = loan_name(loan);
                if existing.metadata.as_ref() == Some(&metadata)
                    && existing.name.as_deref() == Some(name.as_str())
                {
                    summary.loans_unchanged += 1;
                    existing
                } else {
                    summary.loans_updated += 1;
                    assets
                        .update_asset_profile(
                            &existing.id,
                            UpdateAssetProfile {
                                name: Some(name),
                                display_code: existing.display_code.clone(),
                                notes: existing.notes.clone().unwrap_or_default(),
                                kind: Some(existing.kind.clone()),
                                quote_mode: Some(QuoteMode::Manual),
                                quote_ccy: Some(existing.quote_ccy.clone()),
                                instrument_type: Some(InstrumentType::Bond),
                                instrument_symbol: Some(symbol.clone()),
                                instrument_exchange_mic: None,
                                provider_config: existing.provider_config.clone(),
                                metadata: Some(metadata),
                            },
                        )
                        .await?
                }
            }
            None => {
                let spec = loan_spec(loan, maturity, None);
                summary.loans_created += 1;
                assets
                    .create_asset(NewAsset {
                        id: None,
                        kind: AssetKind::Investment,
                        name: Some(loan_name(loan)),
                        display_code: Some(symbol.clone()),
                        is_active: true,
                        quote_mode: QuoteMode::Manual,
                        quote_ccy: loan.currency.trim().to_ascii_uppercase(),
                        instrument_type: Some(InstrumentType::Bond),
                        instrument_symbol: Some(symbol.clone()),
                        instrument_exchange_mic: None,
                        provider_config: None,
                        notes: None,
                        metadata: Some(loan_metadata(&request.platform, loan, &spec, None)),
                    })
                    .await?
            }
        };
        classify(taxonomies, &asset.id, loan, &mut summary).await;
        loan_assets.insert(project_id.clone(), asset.id.clone());
        nominals.insert(project_id, loan.nominal);
    }

    // Loans imported before and absent from this loan list.
    let prefix = format!("{}-", platform_code(&request.platform));
    for (symbol, asset) in by_symbol {
        if let Some(project_id) = symbol.strip_prefix(&prefix) {
            loan_assets
                .entry(project_id.to_string())
                .or_insert(asset.id);
        }
    }

    let booked_keys: HashSet<String> = activities
        .get_activities_by_account_id(&request.account_id)?
        .into_iter()
        .filter_map(|a| a.idempotency_key)
        .collect();
    let creates = plan_activities(
        &request,
        &loan_assets,
        &nominals,
        &booked_keys,
        &mut summary,
    );
    if !creates.is_empty() {
        let result = activities
            .bulk_mutate_activities(ActivityBulkMutationRequest {
                creates,
                updates: Vec::new(),
                delete_ids: Vec::new(),
            })
            .await?;
        if !result.errors.is_empty() {
            let messages: Vec<String> = result.errors.into_iter().map(|e| e.message).collect();
            return Err(invalid(format!(
                "statement lines rejected: {}",
                messages.join("; ")
            )));
        }
        summary.activities_created = result.created.len();
    }
    Ok(summary)
}

/// Classifies a loan in each taxonomy where it has no category yet, so that
/// a classification made by the user is never overridden.
async fn classify(
    taxonomies: &(dyn TaxonomyServiceTrait + Send + Sync),
    asset_id: &str,
    loan: &ImportedLoan,
    summary: &mut PrivateDebtImportSummary,
) {
    let assigned: HashSet<String> = match taxonomies.get_asset_assignments(asset_id) {
        Ok(assignments) => assignments.into_iter().map(|a| a.taxonomy_id).collect(),
        Err(err) => {
            summary.warnings.push(format!(
                "Loan {}: classification skipped ({})",
                loan.project_id.trim(),
                err
            ));
            return;
        }
    };
    let country = loan
        .country_code
        .as_deref()
        .map(str::trim)
        .filter(|c| c.len() == 2)
        .map(|c| format!("country_{}", c.to_ascii_uppercase()));
    let sector = loan.project_type.as_deref().and_then(gics_sector);
    let targets = [
        ("instrument_type", Some("PRIVATE_LOAN".to_string())),
        ("asset_classes", Some("ALT_PRIVATE_CREDIT".to_string())),
        ("regions", country),
        ("industries_gics", sector.map(str::to_string)),
    ];
    for (taxonomy_id, category_id) in targets {
        let Some(category_id) = category_id else {
            continue;
        };
        if assigned.contains(taxonomy_id) {
            continue;
        }
        let assignment = NewAssetTaxonomyAssignment {
            id: None,
            asset_id: asset_id.to_string(),
            taxonomy_id: taxonomy_id.to_string(),
            category_id: category_id.clone(),
            weight: 10000,
            source: "AUTO".to_string(),
        };
        if let Err(err) = taxonomies.assign_asset_to_category(assignment).await {
            summary.warnings.push(format!(
                "Loan {}: cannot classify as {} in {} ({})",
                loan.project_id.trim(),
                category_id,
                taxonomy_id,
                err
            ));
        }
    }
}

fn invalid(message: String) -> Error {
    Error::Validation(ValidationError::InvalidInput(format!(
        "Private debt import: {message}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn loan(
        project_id: &str,
        purchase: &str,
        next: Option<&str>,
        total: u32,
        remaining: u32,
        nominal: Decimal,
    ) -> ImportedLoan {
        ImportedLoan {
            project_id: project_id.to_string(),
            borrower: "Borrower".to_string(),
            purchase_date: date(purchase),
            next_payment_date: next.map(date),
            rate: dec!(14.5),
            loyalty_bonus: Some(Decimal::ZERO),
            project_type: Some("Services".to_string()),
            total_months: total,
            remaining_months: remaining,
            country_code: Some("GB".to_string()),
            status: Some("Funded".to_string()),
            nominal,
            currency: "EUR".to_string(),
        }
    }

    fn line(
        timestamp: &str,
        kind: &str,
        project: Option<&str>,
        amount: Decimal,
    ) -> ImportedTransaction {
        ImportedTransaction {
            timestamp: NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S").unwrap(),
            kind: kind.to_string(),
            project_id: project.map(str::to_string),
            amount,
            currency: "EUR".to_string(),
        }
    }

    fn request(transactions: Vec<ImportedTransaction>) -> PrivateDebtImportRequest {
        PrivateDebtImportRequest {
            account_id: "acc".to_string(),
            platform: "Maclear".to_string(),
            loans: Vec::new(),
            transactions,
        }
    }

    #[test]
    fn maturity_follows_the_remaining_payments_or_the_loan_period() {
        let funded = loan(
            "PJ2262",
            "2026-08-17",
            Some("2026-12-17"),
            14,
            11,
            dec!(150),
        );
        assert_eq!(maturity_date(&funded), Some(date("2027-10-17")));
        let secondary = loan("PJ1602", "2026-04-06", Some("2026-12-06"), 13, 6, dec!(30));
        assert_eq!(maturity_date(&secondary), Some(date("2027-05-06")));
        let repaid = loan("PJ509", "2025-06-03", None, 9, 0, dec!(114.25));
        assert_eq!(maturity_date(&repaid), Some(date("2026-03-03")));
    }

    #[test]
    fn imported_terms_keep_the_user_valuation_method() {
        let pj = loan(
            "PJ2262",
            "2026-08-17",
            Some("2026-12-17"),
            14,
            11,
            dec!(150),
        );
        let spec = loan_spec(&pj, date("2027-10-17"), None);
        assert_eq!(spec.coupon_rate, Some(dec!(0.145)));
        assert_eq!(spec.valuation_method.as_deref(), Some("PAR"));
        assert_eq!(spec.issue_date, Some(date("2026-08-17")));

        let user = BondSpec {
            valuation_method: Some("AMORTIZED_COST".to_string()),
            coupon_rate: Some(dec!(0.2)),
            ..Default::default()
        };
        let spec = loan_spec(&pj, date("2027-10-17"), Some(user));
        assert_eq!(spec.valuation_method.as_deref(), Some("AMORTIZED_COST"));
        assert_eq!(spec.coupon_rate, Some(dec!(0.145)));
    }

    #[test]
    fn statement_lines_become_cash_and_loan_activities() {
        let req = request(vec![
            line("2025-01-23 10:00:00", "Deposit", None, dec!(500)),
            line(
                "2026-04-06 15:11:39",
                "Secondary market buy",
                Some("PJ1602"),
                dec!(-29.40),
            ),
            line(
                "2026-04-06 15:11:39",
                "Secondary market profit",
                Some("PJ1602"),
                dec!(0.60),
            ),
            line(
                "2026-04-05 10:00:00",
                "Reserved",
                Some("PJ1602"),
                dec!(-29.40),
            ),
            line(
                "2026-05-06 04:12:16",
                "Interest payment",
                Some("PJ1602"),
                dec!(0.36),
            ),
            line(
                "2027-05-06 04:00:00",
                "Principal repayment",
                Some("PJ1602"),
                dec!(30),
            ),
            line(
                "2026-05-07 04:00:00",
                "Interest payment",
                Some("PJ9999"),
                dec!(1),
            ),
            line("2026-05-08 04:00:00", "Cashback", None, dec!(1)),
        ]);
        let assets = HashMap::from([("PJ1602".to_string(), "asset-1602".to_string())]);
        let nominals = HashMap::from([("PJ1602".to_string(), dec!(30))]);
        let mut summary = PrivateDebtImportSummary::default();
        let planned = plan_activities(&req, &assets, &nominals, &HashSet::new(), &mut summary);

        let types: Vec<&str> = planned.iter().map(|a| a.activity_type.as_str()).collect();
        assert_eq!(types, ["DEPOSIT", "BUY", "INTEREST", "SELL"]);
        assert!(planned[0].symbol.is_none());
        assert_eq!(planned[1].quantity, Some(dec!(30)));
        assert_eq!(planned[1].unit_price, Some(dec!(0.98)));
        assert_eq!(planned[1].amount, Some(dec!(29.40)));
        assert_eq!(planned[1].activity_date, "2026-04-06T15:11:39Z");
        assert_eq!(planned[3].quantity, Some(dec!(30)));
        assert_eq!(summary.lines_ignored, 2);
        assert_eq!(summary.warnings.len(), 2);

        // A second import of the same statement books nothing new.
        let booked: HashSet<String> = planned
            .iter()
            .filter_map(|a| a.idempotency_key.clone())
            .collect();
        let mut again = PrivateDebtImportSummary::default();
        assert!(plan_activities(&req, &assets, &nominals, &booked, &mut again).is_empty());
        assert_eq!(again.activities_skipped, 4);
    }

    #[test]
    fn identical_lines_are_each_booked_once() {
        let req = request(vec![
            line(
                "2026-05-06 04:00:00",
                "Interest payment",
                Some("PJ1"),
                dec!(1.83),
            ),
            line(
                "2026-05-06 04:00:00",
                "Interest payment",
                Some("PJ1"),
                dec!(1.830),
            ),
        ]);
        let assets = HashMap::from([("PJ1".to_string(), "a".to_string())]);
        let mut summary = PrivateDebtImportSummary::default();
        let planned = plan_activities(
            &req,
            &assets,
            &HashMap::new(),
            &HashSet::new(),
            &mut summary,
        );
        let keys: Vec<&str> = planned
            .iter()
            .filter_map(|a| a.idempotency_key.as_deref())
            .collect();
        assert_eq!(
            keys,
            [
                "MACLEAR:2026-05-06T04:00:00:INTEREST:PJ1:1.83",
                "MACLEAR:2026-05-06T04:00:00:INTEREST:PJ1:1.83:2"
            ]
        );
    }

    #[test]
    fn project_types_map_to_gics_sectors() {
        assert_eq!(gics_sector("Loan Originator"), Some("40"));
        assert_eq!(gics_sector("Packaging and Distribution "), Some("20"));
        assert_eq!(gics_sector("Wholesale and services"), Some("25"));
        assert_eq!(gics_sector("Agriculture"), Some("30"));
        assert_eq!(gics_sector("Real estate"), None);
    }
}
