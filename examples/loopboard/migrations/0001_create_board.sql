CREATE TABLE posts (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    author     TEXT NOT NULL,
    handle     TEXT NOT NULL,
    text       TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE ledger (
    user_id TEXT PRIMARY KEY,
    handle  TEXT NOT NULL,
    balance BIGINT NOT NULL
);

CREATE TABLE tips (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id    BIGINT NOT NULL,
    tipper     TEXT NOT NULL,
    recipient  TEXT NOT NULL,
    amount     BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);
