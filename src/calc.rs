//! A small calculator for the launcher: + - * / % ^ and brackets.

pub fn answer(text: &str) -> Option<String> {
    let t = text.trim();
    // Only treat it as a sum if it has a digit and an operator or bracket.
    if !t.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if !t.chars().any(|c| "+-*/%^(".contains(c)) {
        return None;
    }
    if !t.chars().all(|c| c.is_ascii_digit() || " .+-*/%^()x\u{d7}\u{f7}".contains(c)) {
        return None;
    }
    let src: Vec<char> = t
        .chars()
        .map(|c| match c {
            'x' | '\u{d7}' => '*',
            '\u{f7}' => '/',
            c => c,
        })
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut p = Parser { s: &src, i: 0 };
    let v = p.expr()?;
    if p.i != src.len() || !v.is_finite() {
        return None;
    }
    Some(format_number(v))
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{:.10}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

struct Parser<'a> {
    s: &'a [char],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(c @ ('+' | '-')) = self.peek() {
            self.i += 1;
            let r = self.term()?;
            v = if c == '+' { v + r } else { v - r };
        }
        Some(v)
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        while let Some(c @ ('*' | '/' | '%')) = self.peek() {
            self.i += 1;
            let r = self.power()?;
            v = match c {
                '*' => v * r,
                '/' => v / r,
                _ => v % r,
            };
        }
        Some(v)
    }
    fn power(&mut self) -> Option<f64> {
        let base = self.unary()?;
        if self.peek() == Some('^') {
            self.i += 1;
            let e = self.power()?; // right associative
            return Some(base.powf(e));
        }
        Some(base)
    }
    fn unary(&mut self) -> Option<f64> {
        match self.peek()? {
            '-' => {
                self.i += 1;
                Some(-self.unary()?)
            }
            '+' => {
                self.i += 1;
                self.unary()
            }
            _ => self.atom(),
        }
    }
    fn atom(&mut self) -> Option<f64> {
        if self.peek()? == '(' {
            self.i += 1;
            let v = self.expr()?;
            if self.peek()? != ')' {
                return None;
            }
            self.i += 1;
            return Some(v);
        }
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect::<String>().parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::answer;
    #[test]
    fn sums() {
        assert_eq!(answer("18*24/3").as_deref(), Some("144"));
        assert_eq!(answer("2^3^2").as_deref(), Some("512"));
        assert_eq!(answer("(1+2)*3").as_deref(), Some("9"));
        assert_eq!(answer("10 % 4").as_deref(), Some("2"));
        assert_eq!(answer("1/3").as_deref(), Some("0.3333333333"));
        assert_eq!(answer("firefox"), None);
        assert_eq!(answer("42"), None);
    }
}
