mod text;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Dialect {
    Sqlite,
    Postgres,
}

impl Dialect {
    pub fn placeholders(&self, sql: &str) -> String {
        match self {
            Dialect::Sqlite => sql.to_string(),
            Dialect::Postgres => {
                let mut out = String::with_capacity(sql.len() + 8);
                let mut n = 0;
                text::scan(sql, |ch, in_code| {
                    if in_code && ch == '?' {
                        n += 1;
                        out.push('$');
                        out.push_str(&n.to_string());
                    } else {
                        out.push(ch);
                    }
                });
                out
            }
        }
    }

    pub fn migration_sql(&self, sql: &str) -> String {
        let substitutions: &[(&str, &str)] = match self {
            Dialect::Sqlite => &[
                ("BIGSERIAL", "INTEGER"),
                ("SERIAL", "INTEGER"),
                ("BYTEA", "BLOB"),
                ("TIMESTAMP", "TEXT"),
            ],
            Dialect::Postgres => &[("BLOB", "BYTEA"), ("TIMESTAMP", "TEXT")],
        };
        text::replace_words(sql, substitutions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_numbers_placeholders_in_order() {
        assert_eq!(
            Dialect::Postgres.placeholders("SELECT * FROM t WHERE a = ? AND b = ?"),
            "SELECT * FROM t WHERE a = $1 AND b = $2"
        );
    }

    #[test]
    fn sqlite_placeholders_pass_through() {
        let sql = "SELECT * FROM t WHERE a = ?";
        assert_eq!(Dialect::Sqlite.placeholders(sql), sql);
    }

    #[test]
    fn placeholders_skip_literals_identifiers_and_comments() {
        assert_eq!(
            Dialect::Postgres.placeholders(
                "SELECT '?', \"a?b\", 'it''s?' FROM t -- not this ?\nWHERE x = ? /* nor ? */ AND y = ?"
            ),
            "SELECT '?', \"a?b\", 'it''s?' FROM t -- not this ?\nWHERE x = $1 /* nor ? */ AND y = $2"
        );
    }

    #[test]
    fn sqlite_migrations_translate_serial_bytea_and_timestamp() {
        assert_eq!(
            Dialect::Sqlite.migration_sql(
                "CREATE TABLE t (id BIGSERIAL PRIMARY KEY, data BYTEA, at TIMESTAMP)"
            ),
            "CREATE TABLE t (id INTEGER PRIMARY KEY, data BLOB, at TEXT)"
        );
    }

    #[test]
    fn postgres_migrations_translate_blob_and_timestamp() {
        assert_eq!(
            Dialect::Postgres
                .migration_sql("create table t (id bigserial primary key, data blob, at timestamp)"),
            "create table t (id bigserial primary key, data BYTEA, at TEXT)"
        );
    }

    #[test]
    fn word_substitution_respects_boundaries_and_quotes() {
        assert_eq!(
            Dialect::Sqlite.migration_sql("CREATE TABLE serials (note TEXT DEFAULT 'BIGSERIAL')"),
            "CREATE TABLE serials (note TEXT DEFAULT 'BIGSERIAL')"
        );
    }
}
