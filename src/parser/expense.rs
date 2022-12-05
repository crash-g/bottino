//! Parse an expense.
//!
//! Since expenses have a more or less complex syntax, we use nom.

use std::{cmp::Ordering, iter::repeat, num::ParseIntError};

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{char, multispace0, multispace1},
    combinator::{eof, map, map_res, opt, recognize, rest, verify},
    error::{context, ErrorKind},
    multi::many0,
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};

use crate::{
    types::{Amount, ParsedExpense, ParsedParticipant},
    validator::is_valid_name,
};

/// Parse an expense submitted by the user.
///
/// For the expense syntax, you can refer to the bot instructions
/// (INSTRUCTIONS.md). Some basic checks are performed by the parser, while
/// other checks are executed later.
pub fn parse_expense(s: &str) -> IResult<&str, ParsedExpense> {
    let (s, creditors) = parse_participants(s, true)?;
    let (s, amount) = parse_amount(s)?;
    let (s, mut debtors) = parse_participants(s, false)?;
    let (s, message) = parse_message(s)?;

    if s.trim().is_empty() {
        let mut participants = creditors;
        participants.append(&mut debtors);
        let message = message.map(|m| m.to_string());

        Ok((s, ParsedExpense::new(participants, amount, message)))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(s, ErrorKind::Eof)))
    }
}

fn parse_participants(s: &str, are_creditors: bool) -> IResult<&str, Vec<ParsedParticipant>> {
    let do_parse_participant_name = |s| parse_participant_name(s, are_creditors);

    context(
        "cannot parse participants",
        many0(preceded(
            multispace0,
            alt((
                // Participant with custom amount.
                map(
                    terminated(
                        tuple((do_parse_participant_name, parse_participant_amount)),
                        alt((multispace1, eof)),
                    ),
                    |(mut p, a)| {
                        p.amount = Some(a);
                        p
                    },
                ),
                // Participant without custom amount.
                terminated(do_parse_participant_name, alt((multispace1, eof))),
            )),
        )),
    )(s)
}

/// Participant name must be alphanumeric and cannot start with a number.
/// If there is a '@' prepended, it is stripped away.
/// If instead '#' is prepended, the participant is considered to be a group ('#' is still stripped away).
fn parse_participant_name(s: &str, is_creditor: bool) -> IResult<&str, ParsedParticipant> {
    let do_parse = |s: &str| -> ParsedParticipant {
        let is_group = s.starts_with('#');

        let name = if s.starts_with('#') || s.starts_with('@') {
            s[1..].to_lowercase()
        } else {
            s.to_lowercase()
        };

        if is_creditor && is_group {
            ParsedParticipant::new_creditor_group(&name, None)
        } else if is_creditor {
            ParsedParticipant::new_creditor(&name, None)
        } else if is_group {
            ParsedParticipant::new_debtor_group(&name, None)
        } else {
            ParsedParticipant::new_debtor(&name, None)
        }
    };

    fn is_valid(name: &str) -> bool {
        if name.starts_with('@') || name.starts_with('#') {
            is_valid_name(&name[1..])
        } else {
            is_valid_name(name)
        }
    }

    context(
        "cannot parse participant name",
        map(
            recognize(
                // Match until a whitespace or '/', '-' is found, then use is_valid
                // to make sure that a name was matched (and not a number, which
                // would be the amount).
                verify(is_not(" \t\r\n/-"), is_valid),
            ),
            do_parse,
        ),
    )(s)
}

fn parse_participant_amount(s: &str) -> IResult<&str, Amount> {
    // The difference with parse_amount is that there is no need to expect a whitespace
    // at the end because the caller already expects (and consumes) it.
    context(
        "cannot parse participant amount",
        // Match until the next whitespace, then try to parse.
        preceded(char('/'), map_res(is_not(" \t\r\n"), do_parse_amount)),
    )(s)
}

fn parse_amount(s: &str) -> IResult<&str, Amount> {
    context(
        "cannot parse amount",
        // Match until the next whitespace, then try to parse.
        delimited(
            multispace0,
            map_res(is_not(" \t\r\n"), do_parse_amount),
            alt((multispace1, eof)),
        ),
    )(s)
}

fn do_parse_amount(x: &str) -> Result<Amount, ParseIntError> {
    let components: Vec<_> = x.split(&[',', '.']).collect();
    if components.len() == 2 {
        let integer_part = components[0].to_string();
        let fractional_part = components[1].to_string();

        let fractional_part_len = fractional_part.len();
        let fractional_part = match fractional_part_len.cmp(&2) {
            Ordering::Less => fractional_part + &make_string_of_char('0', 2 - fractional_part_len),
            Ordering::Greater => fractional_part[0..2].to_string(),
            Ordering::Equal => fractional_part,
        };
        (integer_part + &fractional_part).parse::<i64>()
    } else {
        let integer_part = components[0].to_string();
        let fractional_part = make_string_of_char('0', 2);
        (integer_part + &fractional_part).parse::<i64>()
    }
}

fn make_string_of_char(c: char, length: usize) -> String {
    repeat(c).take(length).collect::<String>()
}

fn parse_message(s: &str) -> IResult<&str, Option<&str>> {
    context(
        "cannot parse message",
        opt(preceded(multispace0, preceded(tag("- "), rest))),
    )(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_participant_name() {
        let participant = parse_participant_name("aBC", true);
        assert!(participant.is_ok());
        let participant = participant.expect("test").1;
        assert_eq!(participant.name, "abc".to_string());
        assert!(participant.is_creditor());
        assert!(participant.amount.is_none());
        assert!(!participant.is_group());

        let participant = parse_participant_name("@Abë", false);
        assert!(participant.is_ok());
        let participant = participant.expect("test").1;
        assert_eq!(participant.name, "abë".to_string());
        assert!(participant.is_debtor());
        assert!(participant.amount.is_none());
        assert!(!participant.is_group());

        let participant = parse_participant_name("#AóBC", false);
        assert!(participant.is_ok());
        let participant = participant.expect("test").1;
        assert_eq!(participant.name, "aóbc".to_string());
        assert!(participant.is_debtor());
        assert!(participant.amount.is_none());
        assert!(participant.is_group());
    }

    #[test]
    fn test_parse_amount() {
        assert_eq!(parse_amount("3.45"), Ok(("", 345)));
        assert_eq!(parse_amount("3,45"), Ok(("", 345)));
        assert_eq!(parse_amount("3"), Ok(("", 300)));
        assert_eq!(parse_amount("+3"), Ok(("", 300)));
        assert_eq!(parse_amount("-3.45"), Ok(("", -345)));
        assert_eq!(parse_amount("-3,45"), Ok(("", -345)));
        assert_eq!(parse_amount("-3"), Ok(("", -300)));
    }

    #[test]
    fn test_parse_participants() -> anyhow::Result<()> {
        let (rest, parsed) = parse_participants("Name1/2 - aa", false)?;
        assert_eq!(parsed[0].name, "name1");
        assert!(parsed[0].is_debtor());
        assert_eq!(parsed[0].amount, Some(200));
        assert_eq!(rest, "- aa");

        let (rest, parsed) = parse_participants(" name1  ", true)?;
        assert_eq!(parsed[0].name, "name1");
        assert!(parsed[0].is_creditor());
        assert_eq!(parsed[0].amount, None);
        assert_eq!(rest, "");
        Ok(())
    }

    #[test]
    fn test_parse_message() {
        assert_eq!(parse_message("- abc  "), Ok(("", Some("abc  "))));
        assert_eq!(parse_message("- abc  def"), Ok(("", Some("abc  def"))));
    }

    #[test]
    fn test_parse() -> anyhow::Result<()> {
        // Parse an expense with too many spaces, UTF-8 characters, custom amounts, groups and a message.
        let (rest, expense) = parse_expense(
            " @creditor1 creditòr2/-21.1 34.3   Debtor1 debtor2/3  @debtor3/1 #ǵroup  - yoh",
        )?;

        assert_eq!(expense.participants.len(), 6);

        assert_eq!(expense.participants[0].name, "creditor1");
        assert!(expense.participants[0].is_creditor());
        assert_eq!(expense.participants[0].amount, None);

        assert_eq!(expense.participants[1].name, "creditòr2");
        assert!(expense.participants[1].is_creditor());
        assert_eq!(expense.participants[1].amount, Some(-2110));

        assert_eq!(expense.participants[2].name, "debtor1");
        assert!(expense.participants[2].is_debtor());
        assert_eq!(expense.participants[2].amount, None);

        assert_eq!(expense.participants[3].name, "debtor2");
        assert!(expense.participants[3].is_debtor());
        assert_eq!(expense.participants[3].amount, Some(300));

        assert_eq!(expense.participants[4].name, "debtor3");
        assert!(expense.participants[4].is_debtor());
        assert_eq!(expense.participants[4].amount, Some(100));

        assert_eq!(expense.participants[5].name, "ǵroup");
        assert!(expense.participants[5].is_debtor());
        assert_eq!(expense.participants[5].amount, None);
        assert!(expense.participants[5].is_group());

        assert_eq!(expense.amount, 3430);
        assert_eq!(expense.message, Some("yoh".to_string()));
        assert_eq!(rest, "");

        // Parse an expense without a message.
        let (rest, expense) =
            parse_expense(" creditor1 creditor2/-21.1 34.3   debtor1 debtor2/3  debtor3/1")?;
        assert_eq!(expense.message, None);
        assert_eq!(rest, "");

        // Parse an expense on multiple lines.
        let (rest, expense) =
            parse_expense("creditor1\ncreditor2/-21.1\n34.3\ndebtor1\ndebtor2/3\ndebtor3/1 - message\non\nmany\nlines")?;
        assert_eq!(expense.participants.len(), 5);
        assert_eq!(expense.amount, 3430);
        assert_eq!(
            expense
                .message
                .unwrap()
                .split('\n')
                .collect::<Vec<_>>()
                .len(),
            4
        );
        assert_eq!(rest, "");

        Ok(())
    }

    #[test]
    fn test_parse_fails() -> anyhow::Result<()> {
        // All parsers finished but there are characters left.
        let result = parse_expense("c1 34.3 d1 d2 123");
        assert!(result.is_err());
        let error = result.unwrap_err();
        if let nom::Err::Error(e) = error {
            assert_eq!(e.input, "123");
            assert_eq!(e.code, ErrorKind::Eof);
        } else {
            assert!(false);
        }

        // Parsing the amount fails.
        let result = parse_expense("c1 12d d1");
        assert!(result.is_err());
        let error = result.unwrap_err();
        if let nom::Err::Error(e) = error {
            assert_eq!(e.input, "12d d1");
            assert_eq!(e.code, ErrorKind::MapRes);
        } else {
            assert!(false);
        }

        // Parsing a participant amount fails.
        let result = parse_expense("c1 12 d1/3.aa");
        assert!(result.is_err());
        let error = result.unwrap_err();
        if let nom::Err::Error(e) = error {
            assert_eq!(e.input, "d1/3.aa");
            assert_eq!(e.code, ErrorKind::Eof);
        } else {
            assert!(false);
        }

        Ok(())
    }
}
