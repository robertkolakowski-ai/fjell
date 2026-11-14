-- Migrering 3: indeks på forslagets gruppe. `suggestion.grp_id` har `ON DELETE SET NULL`, så hver `DELETE FROM grp`
-- skannet hele forslagstabellen uten denne (kvadratisk ved gruppeoppdateringer i store indekser, se
-- docs/mvp/YTELSE-STORT.md).
CREATE INDEX idx_suggestion_grp ON suggestion(grp_id);
