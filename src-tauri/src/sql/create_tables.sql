CREATE TABLE IF NOT EXISTS identities (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    enable          INTEGER NOT NULL DEFAULT 1,
    enable_update   INTEGER NOT NULL DEFAULT 1,
    birthday        TEXT,
    default_remember INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS accounts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    identity_id     INTEGER NOT NULL,
    account_name    TEXT NOT NULL,
    account_id      TEXT NOT NULL UNIQUE,
    password        TEXT NOT NULL,
    enable          INTEGER NOT NULL DEFAULT 1,
    enable_update   INTEGER NOT NULL DEFAULT 1,
    expire_date     TEXT NOT NULL DEFAULT '2099-12-31',
    last_update_time TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    FOREIGN KEY (identity_id) REFERENCES identities(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_accounts_identity_id ON accounts(identity_id);
CREATE INDEX IF NOT EXISTS idx_accounts_account_id ON accounts(account_id);

CREATE TABLE IF NOT EXISTS bill_merged (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    identity_id             INTEGER NOT NULL,
    date_str                TEXT NOT NULL,
    time_str                TEXT NOT NULL,
    time_str_formatted      TEXT,
    date_time_formatted     TEXT,
    end_date_time_formatted TEXT,
    timestamp               INTEGER,
    end_timestamp           INTEGER,
    item_type               TEXT,
    number                  TEXT,
    number_list             TEXT,
    target_user             TEXT,
    money_str               TEXT,
    money                   REAL,
    method                  TEXT,
    status_str              TEXT,
    is_combined             INTEGER NOT NULL DEFAULT 0,
    source_account_id       TEXT,
    is_manual               INTEGER NOT NULL DEFAULT 0,
    position                TEXT,
    room                    TEXT,
    notes                   TEXT,
    synced_at               TEXT
);

CREATE INDEX IF NOT EXISTS idx_bill_merged_identity_id ON bill_merged(identity_id);
CREATE INDEX IF NOT EXISTS idx_bill_merged_timestamp ON bill_merged(timestamp);
CREATE INDEX IF NOT EXISTS idx_bill_merged_number_list ON bill_merged(number_list);
CREATE INDEX IF NOT EXISTS idx_bill_merged_source_account ON bill_merged(source_account_id);

CREATE TABLE IF NOT EXISTS bill_original (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    date_str                TEXT NOT NULL,
    time_str                TEXT NOT NULL,
    time_str_formatted      TEXT,
    date_time_formatted     TEXT,
    end_date_time_formatted TEXT,
    timestamp               INTEGER,
    end_timestamp           INTEGER,
    item_type               TEXT,
    number                  TEXT,
    number_list             TEXT,
    target_user             TEXT,
    money_str               TEXT,
    money                   REAL,
    method                  TEXT,
    status_str              TEXT,
    is_combined             INTEGER NOT NULL DEFAULT 0,
    account_id              TEXT NOT NULL,
    synced_at               TEXT
);

CREATE INDEX IF NOT EXISTS idx_bill_original_timestamp ON bill_original(timestamp);
CREATE INDEX IF NOT EXISTS idx_bill_original_number_list ON bill_original(number_list);
CREATE INDEX IF NOT EXISTS idx_bill_original_account_id ON bill_original(account_id);

CREATE TABLE IF NOT EXISTS operation_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    identity_id     INTEGER NOT NULL,
    operation_type  TEXT NOT NULL,
    record_numbers  TEXT,
    operation_time  TEXT NOT NULL,
    description     TEXT,
    account_id      TEXT
);

CREATE TABLE IF NOT EXISTS session_info (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id  TEXT NOT NULL UNIQUE,
    cookies     TEXT NOT NULL,
    login_time  TEXT,
    expire_time TEXT,
    is_valid    INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_session_info_account_id ON session_info(account_id);
