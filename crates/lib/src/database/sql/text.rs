pub(super) fn scan(sql: &str, mut emit: impl FnMut(char, bool)) {
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

pub(super) fn replace_words(sql: &str, substitutions: &[(&str, &str)]) -> String {
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
