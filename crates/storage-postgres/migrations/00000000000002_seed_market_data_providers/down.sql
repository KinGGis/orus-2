DELETE FROM wf_market_data_providers
WHERE id IN (
    'YAHOO',
    'MARKETDATA_APP',
    'ALPHA_VANTAGE',
    'FINNHUB',
    'US_TREASURY_CALC',
    'BOERSE_FRANKFURT',
    'OPENFIGI',
    'METAL_PRICE_API'
);
