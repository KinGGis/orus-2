//! Direct Interactive Brokers integration over their public MCP server.
//!
//! Unlike the aggregator path, this talks to IBKR itself: positions, balances
//! and trades come straight from the broker, which removes the reconciliation
//! drift that produced phantom positions and an inflated NAV.

pub mod allocation;
pub mod client;
pub mod mcp;
pub mod models;
pub mod oauth;
pub mod snapshot;

pub use allocation::{fetch_allocations, map_allocations, parse_allocations, AllocationResponse};
pub use client::{
    security_type_for, IbkrMcpClient, IbkrNavHistory, IbkrPriceBar, IBKR_ACCOUNT_ID,
    IBKR_CONNECTION_ID, IBKR_SOURCE_SYSTEM,
};
pub use mcp::McpClient;
pub use oauth::{
    build_authorize_url, generate_pkce, generate_state, http_client, register_client,
    IbkrTokenManager, IbkrTokenStore, PkcePair, IBKR_MCP_ENDPOINT, IBKR_SCOPE,
};
pub use snapshot::{account_summary_snapshot, allocation_snapshot, performance_snapshot};
