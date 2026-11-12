-- Migrering 2: begrunnelsen for valgt keeper lagres sammen med gruppen (NULL = valgt manuelt), og indeks for kortspørringen.
ALTER TABLE grp ADD COLUMN keeper_reason TEXT;

-- Kortspørringen slår opp ventende forslag per bilde (høyere prioritet skjuler lavere). Uten denne skannet
-- SQLite alle ventende forslag per rad (målt 25-40 ms ved 10 000 bilder, < 1 ms med indeksen).
CREATE INDEX idx_suggestion_asset ON suggestion(asset_id, status);
