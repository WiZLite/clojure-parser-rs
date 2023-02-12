mod alt;
mod tuple;

pub use tuple::tuple;
pub use alt::alt;
pub use token_combinator_macros::ParseToken;

// T stands for Token
// K stands for Kind
// O stands for Output
// W stands for Wrapper

#[derive(Debug, PartialEq, Eq)]
pub enum TokenParseErrorKind<T> {
    Expects { expects: &'static str, found: T },
    NotEnoughToken,
    Context(&'static str),
}

#[derive(Debug, PartialEq, Eq)]
pub struct TokenParseError<T> {
    pub errors: Vec<TokenParseErrorKind<T>>,
    pub tokens_consumed: usize,
}

impl<T> TokenParseError<T> {
    pub fn with_tokens_consumed(self, tokens_consumed: usize) -> Self {
        TokenParseError {
            errors: self.errors,
            tokens_consumed
        }
    }
}

pub type TokenParseResult<'a, T, O, W = T> = Result<(&'a [W], O), TokenParseError<T>>;

pub trait TokenParser<'a, T: Copy, O, W: Into<T>> {
    fn parse(&mut self, tokens: &'a [W]) -> Result<(&'a [W], O), TokenParseError<T>>;
}

impl<'a, T, O, W, F> TokenParser<'a, T, O, W> for F
where
    T: Copy,
    W: 'a + Copy + Into<T>,
    F: FnMut(&'a [W]) -> Result<(&'a [W], O), TokenParseError<T>>,
{
    fn parse(&mut self, tokens: &'a [W]) -> Result<(&'a [W], O), TokenParseError<T>> {
        self(tokens)
    }
}

pub fn many1<'a, T, O, W>(
    mut parser: impl TokenParser<'a, T, O, W>,
) -> impl FnMut(&'a [W]) -> TokenParseResult<'a, T, Vec<O>, W>
where
    T: Copy,
    W: 'a + Copy + Into<T>,
{
    move |tokens: &'a [W]| {
        let mut vec = Vec::new();
        let mut rest = tokens;
        let mut succeeded_at_least_once = false;
        while rest.len() > 0 {
            match parser.parse(rest) {
                Ok((rest_tokens, item)) => {
                    rest = rest_tokens;
                    succeeded_at_least_once = true;
                    vec.push(item);
                    continue;
                }
                Err(err) => {
                    if succeeded_at_least_once {
                        break;
                    } else {
                        return Err(err);
                    }
                }
            }
        }
        Ok((rest, vec))
    }
}

pub fn many0<'a, T, O, W>(
    mut parser: impl TokenParser<'a, T, O, W>,
) -> impl FnMut(&'a [W]) -> TokenParseResult<'a, T, Vec<O>, W>
where
    T: Copy,
    W: 'a + Copy + Into<T>,
{
    move |tokens: &'a [W]| {
        let mut vec = Vec::new();
        let mut rest = tokens;
        while rest.len() > 0 {
            match parser.parse(rest) {
                Ok((rest_tokens, item)) => {
                    rest = rest_tokens;
                    vec.push(item);
                    continue;
                }
                _ => break,
            }
        }
        Ok((rest, vec))
    }
}

pub fn opt<'a, T, O, W>(
    mut parser: impl TokenParser<'a, T, O, W>,
) -> impl FnMut(&'a [W]) -> TokenParseResult<'a, T, Option<O>, W>
where
    T: Copy,
    W: 'a + Copy + Into<T>,
{
    move |tokens: &'a [W]| {
        match parser.parse(tokens) {
            Ok((rest, output)) => Ok((rest, Some(output))),
            Err(_) => Ok((tokens, None))
        }
    }
}

pub fn delimited<'a, T, O1, O2, O3, W: 'a>(
    mut l: impl FnMut(&'a [W]) -> TokenParseResult<'a, T, O1, W>,
    mut main: impl FnMut(&'a [W]) -> TokenParseResult<'a, T, O2, W>,
    mut r: impl FnMut(&'a [W]) -> TokenParseResult<'a, T, O3, W>,
) -> impl FnMut(&'a [W]) -> TokenParseResult<'a, T, O2, W> {
    move |tokens: &'a [W]| {
        let (rest, _) = l(tokens)?;
        let (rest, result) = main(rest)?;
        let (rest, _) = r(rest)?;

        Ok((rest, result))
    }
}

pub fn separated_list0<'a, T, O, OSep, W: 'a>(
    mut separator_parser: impl FnMut(&'a [W]) -> TokenParseResult<'a, T, OSep, W>,
    mut item_parser: impl FnMut(&'a [W]) -> TokenParseResult<'a, T, O, W>,
) -> impl FnMut(&'a [W]) -> TokenParseResult<'a, T, Vec<O>, W> {
    move |tokens: &'a [W]| {
        let mut items = Vec::new();
        let mut rest = tokens;
        while !tokens.is_empty() {
            match item_parser(rest) {
                Ok((rest_tokens, item)) => {
                    rest = rest_tokens;
                    items.push(item);
                }
                Err(_) => return Ok((rest, items))
            }
            if rest.is_empty() {
                return Ok((rest, items))
            }
            match separator_parser(rest) {
                Ok((rest_tokens, _)) => {
                    rest = rest_tokens;
                }
                Err(_) => return Ok((rest, items))
            }
        }
        Ok((&[], Vec::new()))
    }
}

pub fn separated_list1<'a, T, O, OSep, W: 'a>(
    mut separator_parser: impl FnMut(&'a [W]) -> TokenParseResult<'a, T, OSep, W>,
    mut item_parser: impl FnMut(&'a [W]) -> TokenParseResult<'a, T, O, W>,
) -> impl FnMut(&'a [W]) -> TokenParseResult<'a, T, Vec<O>, W> {
    move |tokens: &'a [W]| {
        let num_tokens = tokens.len();
        let mut items = Vec::new();
        let mut rest = tokens;
        while !tokens.is_empty() {
            match item_parser(rest) {
                Ok((rest_tokens, item)) => {
                    rest = rest_tokens;
                    items.push(item);
                }
                Err(err) => {
                    if items.len() > 0 {
                        return Ok((rest, items))
                    } else {
                        return Err(err.with_tokens_consumed(num_tokens - rest.len()));
                    }
                }
            }
            if rest.is_empty() {
                return Ok((rest, items))
            }
            match separator_parser(rest) {
                Ok((rest_tokens, _)) => {
                    rest = rest_tokens;
                }
                Err(_) => return Ok((rest, items))
            }
        }
        // If tokens is empty, returns error.
        return Err(TokenParseError { errors: vec![TokenParseErrorKind::NotEnoughToken], tokens_consumed: 0 });
    }
}

pub fn map<'a, T, OParser, O, W: 'a>(
    mut parser: impl FnMut(&'a [W]) -> TokenParseResult<'a, T, OParser, W>,
    mut mapper: impl FnMut(OParser) -> O
) -> impl FnMut(&'a [W]) -> TokenParseResult<'a, T, O, W> {
    move| tokens: &'a [W]| {
        let (rest, result) = parser(tokens)?;
        Ok((rest, mapper(result)))
    }
}