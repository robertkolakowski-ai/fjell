-- Migrering 4: hvilken brukerregel (se `rules`) som valgte keeper, lagret som typet tekst
-- (`nøkkel[:param]`). NULL = automatikken (K5) eller manuelt valg. Selve regellisten ligger i
-- `setting` med nøkkelen `keeper_rules`; en indeks uten regler er uendret og gir uendret oppførsel.
ALTER TABLE grp ADD COLUMN keeper_rule TEXT;
