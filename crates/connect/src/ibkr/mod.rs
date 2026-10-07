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

pub use allocation::{fetch_allocations, map_allocations, AllocationResponse};
pub use client::{IbkrMcpClient, IBKR_ACCOUNT_ID, IBKR_CONNECTION_ID, IBKR_SOURCE_SYSTEM};
pub use mcp::McpClient;
pub use oauth::{
    build_authorize_url, generate_pkce, generate_state, http_client, register_client,
    IbkrTokenManager, IbkrTokenStore, PkcePair, IBKR_MCP_ENDPOINT, IBKR_SCOPE,
};
