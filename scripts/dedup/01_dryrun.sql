-- ===========================================================================
-- 01 — DRY RUN DE LA MIGRATION
--
-- Exécute l'INTÉGRALITÉ de la migration 00000000000004 puis lève une exception
-- pour tout annuler. Le rollback n'est pas une option qu'on pourrait oublier :
-- le bloc se termine TOUJOURS par une exception, donc il est structurellement
-- impossible que ce script écrive quoi que ce soit.
--
-- Le résultat s'affiche comme un message d'ERREUR dans l'éditeur Supabase.
-- C'est le fonctionnement attendu : le message contient les compteurs.
-- Un message commençant par "DRY RUN OK" est un succès.
--
-- À exécuter APRÈS avoir pris une sauvegarde, et après 00_baseline.sql.
-- ===========================================================================

DO $dryrun$
DECLARE
    v_assets_avant     BIGINT;
    v_assets_apres     BIGINT;
    v_fusionnes        BIGINT;
    v_activites_repointees BIGINT;
    v_quotes_avant     BIGINT;
    v_quotes_apres     BIGINT;
    v_taxo_supprimees  BIGINT;
    v_cles_nulles      BIGINT;
    v_controle_noms    TEXT;
    v_doubles_cotations TEXT;
BEGIN
    SET LOCAL statement_timeout = 0;

    SELECT COUNT(*) INTO v_assets_avant FROM wf_assets;
    SELECT COUNT(*) INTO v_quotes_avant FROM wf_quotes;

    -- --- 1. Filet de sécurité ------------------------------------------------
    CREATE TABLE IF NOT EXISTS wf_assets_dedup_backup AS
    SELECT * FROM wf_assets;

    CREATE TABLE IF NOT EXISTS wf_assets_dedup_map (
        duplicate_id   UUID PRIMARY KEY,
        canonical_id   UUID NOT NULL,
        instrument_key TEXT NOT NULL,
        merged_at      TIMESTAMPTZ NOT NULL DEFAULT now()
    );

    -- --- 2. Résolution des groupes -------------------------------------------
    INSERT INTO wf_assets_dedup_map (duplicate_id, canonical_id, instrument_key)
    WITH keyed AS (
        SELECT
            id,
            created_at,
            CASE
                WHEN instrument_type IS NULL OR instrument_symbol IS NULL THEN NULL
                WHEN instrument_type IN ('FX', 'CRYPTO')
                    THEN instrument_type || ':' || instrument_symbol || '/' || quote_ccy
                WHEN instrument_exchange_mic IS NOT NULL
                    THEN instrument_type || ':' || instrument_symbol || '@' || instrument_exchange_mic
                ELSE instrument_type || ':' || instrument_symbol
            END AS computed_key
        FROM wf_assets
    ),
    ranked AS (
        SELECT
            id AS duplicate_id,
            computed_key,
            first_value(id) OVER (
                PARTITION BY computed_key ORDER BY created_at DESC, id DESC
            ) AS canonical_id,
            row_number() OVER (
                PARTITION BY computed_key ORDER BY created_at DESC, id DESC
            ) AS row_num
        FROM keyed
        WHERE computed_key IS NOT NULL
    )
    SELECT duplicate_id, canonical_id, computed_key
    FROM ranked
    WHERE row_num > 1
    ON CONFLICT (duplicate_id) DO NOTHING;

    SELECT COUNT(*) INTO v_fusionnes FROM wf_assets_dedup_map;

    -- --- 3. Repointage des activités (réversible) ----------------------------
    CREATE TABLE IF NOT EXISTS wf_activities_dedup_map (
        activity_id  UUID PRIMARY KEY,
        old_asset_id UUID NOT NULL
    );

    INSERT INTO wf_activities_dedup_map (activity_id, old_asset_id)
    SELECT a.id, a.asset_id
    FROM wf_activities a
    JOIN wf_assets_dedup_map m ON m.duplicate_id = a.asset_id
    ON CONFLICT (activity_id) DO NOTHING;

    SELECT COUNT(*) INTO v_activites_repointees FROM wf_activities_dedup_map;

    UPDATE wf_activities a
    SET asset_id = m.canonical_id
    FROM wf_assets_dedup_map m
    WHERE a.asset_id = m.duplicate_id;

    -- --- 4. Assignations de taxonomie ----------------------------------------
    WITH losers AS (
        SELECT
            t2.id,
            row_number() OVER (
                PARTITION BY COALESCE(m.canonical_id, t2.asset_id), t2.taxonomy_id, t2.category_id
                ORDER BY (m.canonical_id IS NULL) DESC, t2.id
            ) AS row_num
        FROM wf_asset_taxonomy_assignments t2
        LEFT JOIN wf_assets_dedup_map m ON m.duplicate_id = t2.asset_id
    )
    DELETE FROM wf_asset_taxonomy_assignments t
    USING losers
    WHERE t.id = losers.id AND losers.row_num > 1;

    GET DIAGNOSTICS v_taxo_supprimees = ROW_COUNT;

    UPDATE wf_asset_taxonomy_assignments t
    SET asset_id = m.canonical_id
    FROM wf_assets_dedup_map m
    WHERE t.asset_id = m.duplicate_id;

    -- --- 5. Suppression des doublons (cascade sur les cotations) -------------
    DELETE FROM wf_assets a
    USING wf_assets_dedup_map m
    WHERE a.id = m.duplicate_id;

    SELECT COUNT(*) INTO v_assets_apres FROM wf_assets;
    SELECT COUNT(*) INTO v_quotes_apres FROM wf_quotes;

    -- --- 6. Colonne générée + index unique -----------------------------------
    DROP INDEX IF EXISTS idx_wf_assets_key;
    ALTER TABLE wf_assets DROP COLUMN IF EXISTS instrument_key;

    ALTER TABLE wf_assets
    ADD COLUMN instrument_key TEXT GENERATED ALWAYS AS (
        CASE
            WHEN instrument_type IS NULL OR instrument_symbol IS NULL THEN NULL
            WHEN instrument_type IN ('FX', 'CRYPTO')
                THEN instrument_type || ':' || instrument_symbol || '/' || quote_ccy
            WHEN instrument_exchange_mic IS NOT NULL
                THEN instrument_type || ':' || instrument_symbol || '@' || instrument_exchange_mic
            ELSE instrument_type || ':' || instrument_symbol
        END
    ) STORED;

    -- Échoue si un doublon a survécu : c'est la garde, pas un accident.
    CREATE UNIQUE INDEX uq_wf_assets_instrument_key
    ON wf_assets(instrument_key)
    WHERE instrument_key IS NOT NULL;

    -- --- 7. Purge des dérivés -------------------------------------------------
    DELETE FROM wf_holdings_snapshots;
    DELETE FROM wf_daily_account_valuation;

    -- --- Contrôles -----------------------------------------------------------
    -- EXECUTE car la colonne n'existait pas au démarrage du bloc.
    EXECUTE 'SELECT COUNT(*) FROM wf_assets
             WHERE instrument_type IS NOT NULL AND instrument_key IS NULL'
    INTO v_cles_nulles;

    SELECT string_agg(instrument_symbol || ' => ' || COALESCE(name, '(sans nom)'), ' | '
                      ORDER BY instrument_symbol)
    INTO v_controle_noms
    FROM wf_assets
    WHERE instrument_symbol IN ('DSY', 'SAN', 'MC');

    EXECUTE $q$
        SELECT string_agg(instrument_key || ' (' || quote_ccy || ')', ' | '
                          ORDER BY instrument_key)
        FROM wf_assets
        WHERE instrument_symbol IN ('STLA', 'NWG', 'SAP', 'ASML')
    $q$ INTO v_doubles_cotations;

    RAISE EXCEPTION E'DRY RUN OK — TOUT EST ANNULE, RIEN N''A ETE ECRIT\n'
        '  assets  : % -> %  (% fusionnes)\n'
        '  activites repointees : %\n'
        '  cotations : % -> %  (% supprimees par cascade, a re-synchroniser)\n'
        '  assignations taxonomie supprimees : %\n'
        '  cles nulles restantes (doit etre 0) : %\n'
        '  controle noms : %\n'
        '  doubles cotations preservees : %',
        v_assets_avant, v_assets_apres, v_fusionnes,
        v_activites_repointees,
        v_quotes_avant, v_quotes_apres, v_quotes_avant - v_quotes_apres,
        v_taxo_supprimees,
        v_cles_nulles,
        COALESCE(v_controle_noms, '(aucun)'),
        COALESCE(v_doubles_cotations, '(aucun)');
END
$dryrun$;
