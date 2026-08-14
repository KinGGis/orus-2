-- ===========================================================================
-- 00 — MESURE DE RÉFÉRENCE (lecture seule, aucun risque)
--
-- À exécuter dans l'éditeur SQL Supabase AVANT toute autre chose, et à garder.
-- Ces chiffres sont la référence contre laquelle on validera la migration.
-- Ils changent à chaque sync : ne pas réutiliser une mesure ancienne.
-- ===========================================================================

WITH keyed AS (
    SELECT
        id,
        CASE
            WHEN instrument_type IS NULL OR instrument_symbol IS NULL THEN NULL
            WHEN instrument_type IN ('FX', 'CRYPTO')
                THEN instrument_type || ':' || instrument_symbol || '/' || quote_ccy
            WHEN instrument_exchange_mic IS NOT NULL
                THEN instrument_type || ':' || instrument_symbol || '@' || instrument_exchange_mic
            ELSE instrument_type || ':' || instrument_symbol
        END AS computed_key
    FROM wf_assets
)
SELECT
    (SELECT COUNT(*) FROM wf_assets)                       AS assets_total,
    COUNT(*) FILTER (WHERE computed_key IS NOT NULL)       AS assets_avec_cle,
    COUNT(DISTINCT computed_key)                           AS cles_distinctes,
    COUNT(*) FILTER (WHERE computed_key IS NOT NULL)
        - COUNT(DISTINCT computed_key)                     AS lignes_a_fusionner,
    (SELECT COUNT(*) FROM wf_activities)                   AS activites,
    (SELECT COUNT(*) FROM wf_activities WHERE asset_id IS NULL) AS activites_sans_actif,
    (SELECT COUNT(*) FROM wf_quotes)                       AS cotations,
    (SELECT COUNT(*) FROM wf_holdings_snapshots)           AS snapshots,
    (SELECT COUNT(*) FROM wf_daily_account_valuation)      AS valorisations,
    (SELECT COUNT(*) FROM wf_asset_taxonomy_assignments)   AS assignations_taxonomie
FROM keyed;

-- ---------------------------------------------------------------------------
-- Contrôle des doubles cotations : ces paires DOIVENT rester distinctes
-- après migration. Deux clés par symbole, dans deux devises.
-- ---------------------------------------------------------------------------

SELECT
    instrument_symbol,
    instrument_exchange_mic,
    quote_ccy,
    COUNT(*) AS lignes,
    MAX(name) AS exemple_nom
FROM wf_assets
WHERE instrument_symbol IN ('STLA', 'NWG', 'SAP', 'ASML')
GROUP BY instrument_symbol, instrument_exchange_mic, quote_ccy
ORDER BY instrument_symbol, instrument_exchange_mic NULLS FIRST;
