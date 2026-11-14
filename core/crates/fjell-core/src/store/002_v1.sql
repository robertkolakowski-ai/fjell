-- Migrering 2 (V1-kontrakten, docs/v1/V1-PLAN.md): beskyttelse, serier, personer, ansiktsindeks, regler.
-- 001 er uendret. Invarianter I1-I16 og alle triggere på `asset` og `action_log` er urørt.
-- `migrate_to` slår av `foreign_keys` rundt denne filen (tabellomskriving, SQLite sin 12-trinnsprosedyre) og
-- kjører `foreign_key_check` etterpå; ingen rader går tapt og ingen kaskade utløses.

-- Serier lagres som `grp` med kind = 'series' (keeper_id, medlemmer med rank og score gjenbrukes).
CREATE TABLE grp_new (
    id         INTEGER PRIMARY KEY,
    kind       TEXT    NOT NULL CHECK (kind IN ('exact', 'visual', 'series')),
    keeper_id  INTEGER REFERENCES asset(id),
    created_at INTEGER NOT NULL
);
INSERT INTO grp_new(id, kind, keeper_id, created_at) SELECT id, kind, keeper_id, created_at FROM grp;
DROP TABLE grp;
ALTER TABLE grp_new RENAME TO grp;

-- Forslag får kind 'series' og 'rule'. Samme kolonner, samme unikhet, samme kortindeks.
CREATE TABLE suggestion_new (
    id          INTEGER PRIMARY KEY,
    asset_id    INTEGER NOT NULL REFERENCES asset(id),
    kind        TEXT    NOT NULL CHECK (kind IN ('exact_dup', 'visual_dup', 'blurry', 'screenshot', 'series', 'rule')),
    grp_id      INTEGER REFERENCES grp(id) ON DELETE SET NULL,
    reason_code TEXT    NOT NULL,
    confidence  REAL    NOT NULL,
    status      TEXT    NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
    UNIQUE (asset_id, kind)
);
INSERT INTO suggestion_new(id, asset_id, kind, grp_id, reason_code, confidence, status)
    SELECT id, asset_id, kind, grp_id, reason_code, confidence, status FROM suggestion;
DROP TABLE suggestion;
ALTER TABLE suggestion_new RENAME TO suggestion;
CREATE INDEX idx_suggestion_cards ON suggestion(status, kind, confidence DESC);

-- Beskyttelse per omfang. `target` er tekst: bilde-id, mappeprefiks, album-id eller person-id
-- (kodingen eies av `model::ProtectScope`). Fjerning av en rad er konfigurasjon og kan angres med ny rad.
CREATE TABLE protection (
    scope      TEXT    NOT NULL CHECK (scope IN ('asset', 'folder', 'album', 'person')),
    target     TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (scope, target)
);

-- Øyeanalyse per bilde (EyeAnalyzer). Ingen rad = ikke analysert (ikke «ingen lukkede øyne»).
CREATE TABLE eye_result (
    asset_id   INTEGER PRIMARY KEY REFERENCES asset(id),
    faces      INTEGER NOT NULL,
    closed     INTEGER NOT NULL,
    confidence REAL    NOT NULL,
    algo_ver   INTEGER NOT NULL DEFAULT 0
);

-- Personer (avledet data). `merged_into` gjør sammenslåing angrebar (null = selvstendig person). Personer
-- fjernes bare av «Slett ansiktsindeks» (eksplisitt, avledet data som kan gjenskapes).
CREATE TABLE person (
    id          INTEGER PRIMARY KEY,
    name        TEXT,
    created_at  INTEGER NOT NULL,
    merged_into INTEGER REFERENCES person(id),
    CHECK (merged_into IS NULL OR merged_into <> id)
);

-- Ansiktsembedding: BLOB (little-endian f32), KUN LOKAL. Aldri eksportert, aldri sendt ut av enheten.
-- Fylles bare når `setting.face_index_enabled = '1'`; «Slett ansiktsindeks» tømmer tabellen (avledet data).
CREATE TABLE face_embedding (
    id        INTEGER PRIMARY KEY,
    asset_id  INTEGER NOT NULL REFERENCES asset(id),
    person_id INTEGER REFERENCES person(id),
    box_x     INTEGER NOT NULL,                  -- promille av bildebredde
    box_y     INTEGER NOT NULL,
    box_w     INTEGER NOT NULL,
    box_h     INTEGER NOT NULL,
    quality   REAL    NOT NULL,
    embedding BLOB    NOT NULL,
    algo_ver  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_face_embedding_asset ON face_embedding(asset_id);
CREATE INDEX idx_face_embedding_person ON face_embedding(person_id);

-- Ansiktsindeks er av som standard (ingen rad = av; raden gjør tilstanden eksplisitt og lesbar).
INSERT OR IGNORE INTO setting(key, value) VALUES ('face_index_enabled', '0');

-- Regler. `spec` er tekst kodet av `rules`-modulen. Regler foreslår bare; `rule_hit` har ingen effekt på
-- bildetilstand. `deleted_at` gir angrebar sletting; selve raden fjernes aldri (trigger under).
CREATE TABLE rule (
    id         INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL,
    spec       TEXT    NOT NULL,
    enabled    INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE TABLE rule_hit (
    rule_id  INTEGER NOT NULL REFERENCES rule(id),
    asset_id INTEGER NOT NULL REFERENCES asset(id),
    hit_at   INTEGER NOT NULL,
    PRIMARY KEY (rule_id, asset_id)
);
CREATE INDEX idx_rule_hit_asset ON rule_hit(asset_id);

CREATE TRIGGER rule_no_delete
BEFORE DELETE ON rule
BEGIN
    SELECT RAISE(ABORT, 'rule slettes aldri hardt; bruk deleted_at');
END;
