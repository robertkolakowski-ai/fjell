-- Migrering 1: hele skjemaet fra docs/mvp/ARKITEKTUR.md del 3, inkl. triggere (beslutning 19).
-- Tider er Unix-sekunder. Ingen kolonne lagrer bildedata utover hasher (I16).

CREATE TABLE asset (
    id                  INTEGER PRIMARY KEY,
    source_id           TEXT    NOT NULL UNIQUE,
    kind                TEXT    NOT NULL CHECK (kind IN ('photo', 'screenshot_hint', 'video')),
    created_at          INTEGER NOT NULL,
    mod_token           TEXT    NOT NULL,
    width               INTEGER NOT NULL,
    height              INTEGER NOT NULL,
    bytes               INTEGER NOT NULL,
    mime                TEXT    NOT NULL,
    platform_screenshot_hint INTEGER NOT NULL DEFAULT 0,
    has_camera_exif     INTEGER,                         -- NULL = ukjent
    is_local            INTEGER NOT NULL DEFAULT 1,
    backup              INTEGER,                         -- NULL = ukjent, 0 = ikke sikkerhetskopiert, 1 = ja
    state               INTEGER NOT NULL DEFAULT 0 CHECK (state IN (0, 1, 2)),  -- 0 active, 1 trashed, 2 purged
    trashed_at          INTEGER,
    expires_at          INTEGER,
    expired_notified_at INTEGER,
    protect_flags       INTEGER NOT NULL DEFAULT 0       -- bitmaske: favoritt, delt album, redigert, ansikter
);
CREATE INDEX idx_asset_state ON asset(state);
CREATE INDEX idx_asset_bytes ON asset(bytes);

CREATE TABLE fingerprint (
    asset_id     INTEGER PRIMARY KEY REFERENCES asset(id),
    content_hash BLOB CHECK (content_hash IS NULL OR length(content_hash) = 32),
    dhash        INTEGER,
    phash        INTEGER,
    blur_score   REAL,
    shot_signals INTEGER,
    algo_ver     INTEGER NOT NULL DEFAULT 0,
    fail_count   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_fingerprint_content_hash ON fingerprint(content_hash);

CREATE TABLE grp (
    id         INTEGER PRIMARY KEY,
    kind       TEXT    NOT NULL CHECK (kind IN ('exact', 'visual')),
    keeper_id  INTEGER REFERENCES asset(id),
    created_at INTEGER NOT NULL
);

CREATE TABLE grp_member (
    grp_id   INTEGER NOT NULL REFERENCES grp(id) ON DELETE CASCADE,
    asset_id INTEGER NOT NULL REFERENCES asset(id),
    rank     INTEGER NOT NULL,
    score    REAL,
    PRIMARY KEY (grp_id, asset_id)
);
CREATE INDEX idx_grp_member_asset ON grp_member(asset_id);

CREATE TABLE suggestion (
    id          INTEGER PRIMARY KEY,
    asset_id    INTEGER NOT NULL REFERENCES asset(id),
    kind        TEXT    NOT NULL CHECK (kind IN ('exact_dup', 'visual_dup', 'blurry', 'screenshot')),
    grp_id      INTEGER REFERENCES grp(id) ON DELETE SET NULL,
    reason_code TEXT    NOT NULL,
    confidence  REAL    NOT NULL,
    status      TEXT    NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
    UNIQUE (asset_id, kind)
);
CREATE INDEX idx_suggestion_cards ON suggestion(status, kind, confidence DESC);

CREATE TABLE action_log (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id         INTEGER NOT NULL,
    ts               INTEGER NOT NULL,
    op               TEXT    NOT NULL CHECK (op IN ('trash', 'restore', 'keep', 'purge')),
    asset_id         INTEGER NOT NULL REFERENCES asset(id),
    prev_state       INTEGER NOT NULL,
    suggestion_id    INTEGER,
    inverse_of       INTEGER UNIQUE REFERENCES action_log(id),  -- unik: dobbel angre inverterer ikke dobbelt (I11)
    prev_sugg_status TEXT,
    cause            TEXT    NOT NULL CHECK (cause IN ('user', 'expiry', 'external', 'undo'))
);
CREATE INDEX idx_action_log_batch ON action_log(batch_id);

CREATE TABLE setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE album (
    id     INTEGER PRIMARY KEY,
    name   TEXT    NOT NULL,
    shared INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE asset_album (
    asset_id INTEGER NOT NULL REFERENCES asset(id),
    album_id INTEGER NOT NULL REFERENCES album(id),
    PRIMARY KEY (asset_id, album_id)
);
CREATE INDEX idx_asset_album_asset ON asset_album(asset_id);

-- I3: `purged` er terminal.
CREATE TRIGGER asset_purged_is_terminal
BEFORE UPDATE OF state ON asset
WHEN OLD.state = 2
BEGIN
    SELECT RAISE(ABORT, 'asset.state: purged er terminal');
END;

-- I2: overgangen 0 -> 2 er blokkert; purged nås bare via trashed (1 -> 2).
CREATE TRIGGER asset_no_direct_purge
BEFORE UPDATE OF state ON asset
WHEN OLD.state = 0 AND NEW.state = 2
BEGIN
    SELECT RAISE(ABORT, 'asset.state: active -> purged er blokkert');
END;

-- I14: `action_log` er append-only.
CREATE TRIGGER action_log_no_update
BEFORE UPDATE ON action_log
BEGIN
    SELECT RAISE(ABORT, 'action_log er append-only');
END;

CREATE TRIGGER action_log_no_delete
BEFORE DELETE ON action_log
BEGIN
    SELECT RAISE(ABORT, 'action_log er append-only');
END;
