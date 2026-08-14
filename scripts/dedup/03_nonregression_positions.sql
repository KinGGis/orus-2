-- ===========================================================================
-- 03 — NON-RÉGRESSION DES POSITIONS (lecture seule)
--
-- À exécuter APRÈS le re-sync market data et POST /api/v1/portfolio/recalculate.
--
-- Compare, pour chaque actif, la quantité du dernier snapshot recalculé au net
-- des activités. Les deux doivent coïncider.
--
-- Écarts attendus, connus, hors périmètre de cette PR :
--   * positions courtes écrasées à 0 par `reduce_lots_fifo` — le net d'activités
--     est négatif, le snapshot affiche 0 (PLTR, MRK, BRBY).
--   * splits et réinvestissements de dividendes, si présents, modifient la
--     quantité sans que la somme brute des activités le reflète.
-- Tout autre écart signifie que le recalcul ne repart pas des activités : c'est
-- le symptôme des positions fantômes, à investiguer séparément.
-- ===========================================================================

WITH dernier_snapshot AS (
    SELECT MAX(snapshot_date) AS d FROM wf_holdings_snapshots
),
positions_snapshot AS (
    SELECT
        (p.value ->> 'assetId')::uuid          AS asset_id,
        SUM((p.value ->> 'quantity')::numeric) AS qte_snapshot
    FROM wf_holdings_snapshots s
    CROSS JOIN LATERAL jsonb_each(s.positions) AS p
    WHERE s.snapshot_date = (SELECT d FROM dernier_snapshot)
    GROUP BY 1
),
net_activites AS (
    SELECT
        asset_id,
        SUM(quantity)                     AS qte_activites,
        string_agg(DISTINCT activity_type, ',' ORDER BY activity_type) AS types
    FROM wf_activities
    WHERE asset_id IS NOT NULL
      AND quantity IS NOT NULL
    GROUP BY asset_id
)
SELECT
    a.instrument_symbol,
    a.instrument_key,
    a.name,
    COALESCE(n.qte_activites, 0) AS qte_activites,
    COALESCE(p.qte_snapshot, 0)  AS qte_snapshot,
    COALESCE(p.qte_snapshot, 0) - COALESCE(n.qte_activites, 0) AS ecart,
    n.types,
    CASE
        WHEN COALESCE(n.qte_activites, 0) < 0 AND COALESCE(p.qte_snapshot, 0) = 0
            THEN 'ATTENDU — position courte ecrasee a 0'
        ELSE 'A INVESTIGUER'
    END AS diagnostic
FROM net_activites n
FULL OUTER JOIN positions_snapshot p ON p.asset_id = n.asset_id
JOIN wf_assets a ON a.id = COALESCE(n.asset_id, p.asset_id)
WHERE COALESCE(p.qte_snapshot, 0) <> COALESCE(n.qte_activites, 0)
ORDER BY diagnostic, ABS(COALESCE(p.qte_snapshot, 0) - COALESCE(n.qte_activites, 0)) DESC;
