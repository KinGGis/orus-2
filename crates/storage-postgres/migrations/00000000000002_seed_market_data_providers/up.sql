-- Seed default market data providers (parity with the SQLite backend).
-- The Supabase schema migration creates wf_market_data_providers but does not
-- seed any rows, which leaves the Market Data settings screen empty when the
-- backend runs against Postgres. This migration inserts the canonical default
-- providers. It is idempotent via ON CONFLICT DO NOTHING.
INSERT INTO wf_market_data_providers
    (id, name, description, url, priority, enabled, logo_filename, last_synced_at, last_sync_status, last_sync_error)
VALUES
    ('YAHOO', 'Yahoo Finance', 'Yahoo Finance is a leading financial data provider for many markets. It provides historical and real-time stock data.', 'https://finance.yahoo.com/', 1, TRUE, 'yahoo-finance.png', NULL, NULL, NULL),
    ('MARKETDATA_APP', 'MarketData.app', 'MarketData.app provides real-time and historical data for U.S. stocks, options, ETFs, mutual funds, and more.', 'https://www.marketdata.app/', 2, FALSE, 'marketdata-app.png', NULL, NULL, NULL),
    ('ALPHA_VANTAGE', 'Alpha Vantage', 'Alpha Vantage provides free APIs for real-time and historical data on stocks, forex, and cryptocurrencies.', 'https://www.alphavantage.co/', 3, FALSE, 'alpha-vantage.png', NULL, NULL, NULL),
    ('FINNHUB', 'Finnhub', 'Finnhub provides real-time stock, forex, and cryptocurrency data with global coverage. Free tier includes 60 API calls/minute.', 'https://finnhub.io/', 4, FALSE, 'finnhub.png', NULL, NULL, NULL),
    ('US_TREASURY_CALC', 'US Treasury (Calculated)', 'Calculates US Treasury bond prices from yield curve data published by the US Treasury Department.', 'https://home.treasury.gov/', 10, TRUE, 'treasury.png', NULL, NULL, NULL),
    ('BOERSE_FRANKFURT', 'Börse Frankfurt', 'Börse Frankfurt provides bond and security pricing data for European markets.', 'https://www.boerse-frankfurt.de/', 11, TRUE, 'boerse.png', NULL, NULL, NULL),
    ('OPENFIGI', 'OpenFIGI', 'OpenFIGI provides a mapping service for financial instrument identifiers (FIGI, ISIN, CUSIP, ticker).', 'https://www.openfigi.com/', 12, TRUE, 'openfigi.png', NULL, NULL, NULL),
    ('METAL_PRICE_API', 'Metal Price API', 'Provides real-time and historical spot prices for precious metals (gold, silver, platinum, palladium).', 'https://metalpriceapi.com/', 13, FALSE, 'metal-price-api.png', NULL, NULL, NULL)
ON CONFLICT (id) DO NOTHING;
