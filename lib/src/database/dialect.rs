/* the single place where sqlite and postgres SQL diverge: positional
placeholder style and the portable-DDL type spellings used in migrations */

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Dialect {
    Sqlite,
    Postgres,
}

impl Dialect {
    /// rewrites `?` placeholders to the dialect's positional style, skipping
    /// string literals, quoted identifiers and comments.
    pub fn placeholders(&self, sql: &str) -> String {
        match self {
            Dialect::Sqlite => sql.to_string(),
            Dialect::Postgres => {
                let mut out = String::with_capacity(sql.len() + 8);
                let mut n = 0;
                scan(sql, |ch, in_code| {
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

    /// rewrites portable migration DDL into this dialect. Migrations use one
    /// vocabulary — `BIGSERIAL` for auto-increment keys, `BLOB` for bytes,
    /// `TIMESTAMP` for dates — and each backend gets its native spelling.
    /// Dates are stored as ISO-8601 text for now, matching `Date(String)`.
    pub fn migration_sql(&self, sql: &str) -> String {
        let substitutions: &[(&str, &str)] = match self {
            // INTEGER PRIMARY KEY is sqlite's rowid alias, which auto-increments
            Dialect::Sqlite => &[
                ("BIGSERIAL", "INTEGER"),
                ("SERIAL", "INTEGER"),
                ("BYTEA", "BLOB"),
                ("TIMESTAMP", "TEXT"),
            ],
            Dialect::Postgres => &[("BLOB", "BYTEA"), ("TIMESTAMP", "TEXT")],
        };
        replace_words(sql, substitutions)
    }
}

/// calls `emit(ch, in_code)` for every char, with `in_code == false` inside
/// '...' / "..." (with doubled-quote escapes) and `--` / `/* */` comments.
fn scan(sql: &str, mut emit: impl FnMut(char, bool)) {
    #[derive(PartialEq)]
    enum State {
        Code,
        SingleQuote,
        DoubleQuote,
        LineComment,
        BlockComment,
    }
    let mut state = State::Code;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        match state {
            State::Code => {
                match ch {
                    '\'' => state = State::SingleQuote,
                    '"' => state = State::DoubleQuote,
                    '-' if chars.peek() == Some(&'-') => state = State::LineComment,
                    '/' if chars.peek() == Some(&'*') => state = State::BlockComment,
                    _ => {}
                }
                emit(ch, state == State::Code);
            }
            State::SingleQuote => {
                emit(ch, false);
                // a doubled quote is an escaped quote, not a terminator
                if ch == '\'' {
                    if chars.peek() == Some(&'\'') {
                        emit(chars.next().unwrap(), false);
                    } else {
                        state = State::Code;
                    }
                }
            }
            State::DoubleQuote => {
                emit(ch, false);
                if ch == '"' {
                    if chars.peek() == Some(&'"') {
                        emit(chars.next().unwrap(), false);
                    } else {
                        state = State::Code;
                    }
                }
            }
            State::LineComment => {
                emit(ch, false);
                if ch == '\n' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                emit(ch, false);
                if ch == '*' && chars.peek() == Some(&'/') {
                    emit(chars.next().unwrap(), false);
                    state = State::Code;
                }
            }
        }
    }
}

/// Case-insensitive whole-word substitution outside strings and comments.
fn replace_words(sql: &str, substitutions: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut word = String::new();

    let flush = |word: &mut String, out: &mut String| {
        if word.is_empty() {
            return;
        }
        let replacement = substitutions
            .iter()
            .find(|(from, _)| word.eq_ignore_ascii_case(from))
            .map(|(_, to)| *to);
        match replacement {
            Some(to) => out.push_str(to),
            None => out.push_str(word),
        }
        word.clear();
    };

    scan(sql, |ch, in_code| {
        if in_code && (ch.is_ascii_alphanumeric() || ch == '_') {
            word.push(ch);
        } else {
            flush(&mut word, &mut out);
            out.push(ch);
        }
    });
    flush(&mut word, &mut out);
    out
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
            Dialect::Postgres.migration_sql(
                "create table t (id bigserial primary key, data blob, at timestamp)"
            ),
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
