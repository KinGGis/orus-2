-- ===========================================================================
-- 02 — VÉRIFICATION APRÈS DÉPLOIEMENT (lecture seule)
--
-- À exécuter une fois le serveur redémarré et la migration appliquée,
-- AVANT le re-sync market data et le recalcul.
-- ===========================================================================

-- 1. La colonne est-elle bien générée ?  attendu : is_generated = ALWAYS
SELECT column_name, is_generated, generation_expression
FROM information_schema.columns
WHERE table_name = 'wf_assets' AND column_name = 'instrument_key';

-- 2. L'index unique existe-t-il ?  attendu : 1 ligne
SELECT indexname, indexdef
FROM pg_indexes
WHERE tablename = 'wf_assets' AND indexname = 'uq_wf_assets_instrument_key';

-- 3. Compteurs.  cles_nulles doit valoir 0.
--    assets_total doit égaler cles_distinctes mesuré par 00_baseline.sql,
--    et non un chiffre figé : la table grossit à chaque sync.
SELECT
    (SELECT COUNT(*) FROM wf_assets) AS assets_total,
    (SELECT COUNT(*) FROM wf_assets
     WHERE instrument_type IS NOT NULL AND instrument_key IS NULL) AS cles_nulles,
    (SELECT COUNT(*) FROM wf_assets_dedup_map)      AS lignes_fusionnees,
    (SELECT COUNT(*) FROM wf_activities_dedup_map)  AS activites_repointees,
    (SELECT COUNT(*) FROM wf_activities WHERE asset_id IS NULL) AS activites_orphelines;

-- 4. Aucune activité ne doit pointer vers un actif inexistant.  attendu : 0
SELECT COUNT(*) AS activites_pointant_dans_le_vide
FROM wf_activities a
WHERE a.asset_id IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM wf_assets s WHERE s.id = a.asset_id);

-- 5. Les noms sont-ils les bons ?
--    attendu : Dassault Systèmes SE / Sanofi / LVMH — et non Big Tree Cloud,
--    Banco Santander, Moelis & Co.
SELECT instrument_symbol, instrument_exchange_mic, instrument_key, name
FROM wf_assets
WHERE instrument_symbol IN ('DSY', 'SAN', 'MC')
ORDER BY instrument_symbol;

-- 6. Doubles cotations préservées.
--    attendu : DEUX lignes par symbole, dans deux devises différentes.
--    Une seule ligne = elles ont été fusionnées à tort : STOP, restaurer.
SELECT instrument_symbol, instrument_key, quote_ccy, name
FROM wf_assets
WHERE instrument_symbol IN ('STLA', 'NWG', 'SAP', 'ASML')
ORDER BY instrument_symbol, instrument_key;

-- 7. Plus aucun doublon possible.  attendu : 0 ligne
SELECT instrument_key, COUNT(*)
FROM wf_assets
WHERE instrument_key IS NOT NULL
GROUP BY instrument_key
HAVING COUNT(*) > 1;
