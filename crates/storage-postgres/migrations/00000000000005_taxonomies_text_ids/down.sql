-- The text identifiers are what the application code expects; there is no
-- meaningful uuid form to restore. Drop the seeded system taxonomies only.
DELETE FROM wf_taxonomies WHERE is_system = TRUE;