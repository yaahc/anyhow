use crate::chain::Chain;
use crate::error::ErrorImpl;
use core::fmt::{self, Debug, Write};

#[cfg(backtrace)]
use crate::backtrace::Backtrace;

pub struct ErrorInfo<'a> {
    error: &'a (dyn std::error::Error + 'static),
    #[cfg(backtrace)]
    backtrace: &'a Backtrace,
    span_backtrace: Option<&'a tracing_error::Context>,
}

trait ErrorFormatter {
    fn fmt_error<'a>(error: ErrorInfo<'a>, f: &mut fmt::Formatter) -> fmt::Result;
}

pub struct RootCauseFirst;
pub struct RootCauseLast;

impl ErrorFormatter for RootCauseFirst {
    fn fmt_error<'a>(
        ErrorInfo {
            error,
            #[cfg(backtrace)]
            backtrace,
            span_backtrace,
        }: ErrorInfo<'a>,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let errors = Chain::new(error).rev().enumerate();

        for (n, error) in errors {
            writeln!(f)?;
            write!(Indented::numbered(f, n), "{}", error.to_string().trim())?;
        }

        if let Some(span_context) = span_backtrace.as_ref() {
            let span_backtrace = span_context.span_backtrace();
            write!(f, "\n\nContext:\n")?;
            write!(f, "{}", span_backtrace)?;
        }

        #[cfg(backtrace)]
        {
            use std::backtrace::BacktraceStatus;
            if let BacktraceStatus::Captured = backtrace.status() {
                let mut backtrace = backtrace.to_string();
                if backtrace.starts_with("stack backtrace:") {
                    // Capitalize to match "Caused by:"
                    backtrace.replace_range(0..7, "Stack B");
                }
                backtrace.truncate(backtrace.trim_end().len());
                write!(f, "\n\n{}", backtrace)?;
            }
        }

        Ok(())
    }
}

impl ErrorFormatter for RootCauseLast {
    fn fmt_error<'a>(
        ErrorInfo {
            error,
            #[cfg(backtrace)]
            backtrace,
            span_backtrace,
        }: ErrorInfo<'a>,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let errors = Chain::new(error).enumerate();

        writeln!(f)?;

        for (n, error) in errors {
            write!(Indented::numbered(f, n), "{}", error)?;
            writeln!(f)?;
        }

        if let Some(span_context) = span_backtrace.as_ref() {
            let span_backtrace = span_context.span_backtrace();
            write!(f, "\n\nContext:\n")?;
            write!(f, "{}", span_backtrace)?;
        }

        #[cfg(backtrace)]
        {
            use std::backtrace::BacktraceStatus;

            if let BacktraceStatus::Captured = backtrace.status() {
                let mut backtrace = backtrace.to_string();
                if backtrace.starts_with("stack backtrace:") {
                    // Capitalize to match "Caused by:"
                    backtrace.replace_range(0..7, "Stack B");
                }
                backtrace.truncate(backtrace.trim_end().len());
                write!(f, "\n\n{}", backtrace)?;
            }
        }

        Ok(())
    }
}

impl ErrorImpl<()> {
    pub(crate) fn display(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.error())?;

        if f.alternate() {
            for cause in self.chain().skip(1) {
                write!(f, ": {}", cause)?;
            }
        }

        Ok(())
    }

    pub(crate) fn debug(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let error = self.error();

        if f.alternate() {
            return Debug::fmt(error, f);
        }

        RootCauseFirst::fmt_error(
            ErrorInfo {
                error,
                #[cfg(backtrace)]
                backtrace: self.backtrace(),
                span_backtrace: self.span_backtrace.as_ref(),
            },
            f,
        )
    }
}

struct Indented<'a, D> {
    inner: &'a mut D,
    ind: Option<usize>,
    started: bool,
}

impl<'a, D> Indented<'a, D> {
    fn numbered(inner: &'a mut D, ind: usize) -> Self {
        Self {
            inner,
            ind: Some(ind),
            started: false,
        }
    }
}

impl<T> fmt::Write for Indented<'_, T>
where
    T: fmt::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (ind, mut line) in s.split('\n').enumerate() {
            if !self.started {
                // trim first line to ensure it lines up with the number nicely
                line = line.trim();
                // Don't render the first line unless its actually got text on it
                if line.is_empty() {
                    continue;
                }

                self.started = true;
                match self.ind {
                    Some(ind) => self.inner.write_fmt(format_args!("{: >5}: ", ind))?,
                    None => self.inner.write_fmt(format_args!("    "))?,
                }
            } else if ind > 0 {
                self.inner.write_char('\n')?;
                if self.ind.is_some() {
                    self.inner.write_fmt(format_args!("       "))?;
                } else {
                    self.inner.write_fmt(format_args!("    "))?;
                }
            }

            self.inner.write_fmt(format_args!("{}", line))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_digit() {
        let input = "verify\nthis";
        let expected = "    2: verify\n       this";
        let mut output = String::new();

        Indented {
            inner: &mut output,
            ind: Some(2),
            started: false,
        }
        .write_str(input)
        .unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn two_digits() {
        let input = "verify\nthis";
        let expected = "   12: verify\n       this";
        let mut output = String::new();

        Indented {
            inner: &mut output,
            ind: Some(12),
            started: false,
        }
        .write_str(input)
        .unwrap();

        assert_eq!(expected, output);
    }

    #[test]
    fn no_digits() {
        let input = "verify\nthis";
        let expected = "    verify\n    this";
        let mut output = String::new();

        Indented {
            inner: &mut output,
            ind: None,
            started: false,
        }
        .write_str(input)
        .unwrap();

        assert_eq!(expected, output);
    }
}
