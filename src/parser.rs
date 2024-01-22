use std::{cmp::Ordering, iter::repeat, num::ParseIntError};

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric0, char, multispace0, not_line_ending},
    combinator::{map, map_res, opt, recognize},
    multi::many0,
    sequence::{preceded, tuple},
    AsChar, IResult, InputTakeAtPosition,
};

use crate::types::{Amount, Expense, Participant};

pub fn parse_expense(s: &str) -> IResult<&str, Expense> {
    let (s, creditors) = parse_participants(s, true)?;
    let (s, amount) = parse_amount(s)?;
    let (s, mut debtors) = parse_participants(s, false)?;
    let (s, message) = parse_message(s)?;

    let mut participants = creditors;
    participants.append(&mut debtors);
    let message = message.map(|m| m.to_string());

    Ok((s, Expense::new(participants, amount, message)))
}

fn parse_participants(s: &str, are_creditors: bool) -> IResult<&str, Vec<Participant>> {
    let do_parse_participant = |x: (&str, Option<Amount>)| {
        if are_creditors {
            Participant::new_creditor(x.0, x.1)
        } else {
            Participant::new_debtor(x.0, x.1)
        }
    };

    many0(preceded(
        multispace0,
        map(
            tuple((parse_participant_name, opt(parse_participant_amount))),
            do_parse_participant,
        ),
    ))(s)
}

/// Participant name must start with a letter. Optionally, there can be
/// be a '@' prepended, which will be stripped away while parsing.
fn parse_participant_name(s: &str) -> IResult<&str, &str> {
    fn do_parse(s: &str) -> &str {
        if s.starts_with('@') {
            &s[1..s.len()]
        } else {
            s
        }
    }

    map(
        recognize(alt((
            preceded(alpha1, alphanumeric0),
            preceded(char('@'), preceded(alpha1, alphanumeric0)),
        ))),
        do_parse,
    )(s)
}

fn parse_participant_amount(s: &str) -> IResult<&str, Amount> {
    preceded(char('/'), parse_amount)(s)
}

fn float1(s: &str) -> IResult<&str, &str> {
    s.split_at_position1_complete(
        |item| !item.is_dec_digit() && item != ',' && item != '.' && item != '-' && item != '+',
        nom::error::ErrorKind::Float,
    )
}

fn parse_amount(s: &str) -> IResult<&str, Amount> {
    fn do_parse(x: &str) -> Result<Amount, ParseIntError> {
        let components: Vec<_> = x.split(&[',', '.']).collect();
        if components.len() == 2 {
            let integer_part = components[0].to_string();
            let fractional_part = components[1].to_string();

            let fractional_part_len = fractional_part.len();
            let fractional_part = match fractional_part_len.cmp(&2) {
                Ordering::Less => {
                    fractional_part + &make_string_of_char('0', 2 - fractional_part_len)
                }
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

    preceded(multispace0, map_res(float1, do_parse))(s)
}

fn make_string_of_char(c: char, length: usize) -> String {
    repeat(c).take(length).collect::<String>()
}

fn parse_message(s: &str) -> IResult<&str, Option<&str>> {
    opt(preceded(
        multispace0,
        preceded(tag("- "), map(not_line_ending, |s| s)),
    ))(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ParticipantMode;

    #[test]
    fn test_parse_participant_name() {
        assert_eq!(parse_participant_name("abc"), Ok(("", "abc")));
        assert_eq!(parse_participant_name("@abc"), Ok(("", "abc")));
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
        let (rest, parsed) = parse_participants("name1/2 - aa", false)?;
        assert_eq!(parsed[0].name, "name1");
        assert_eq!(parsed[0].mode, ParticipantMode::Debtor);
        assert_eq!(parsed[0].amount, Some(200));
        assert_eq!(rest, " - aa");

        let (rest, parsed) = parse_participants(" name1  ", true)?;
        assert_eq!(parsed[0].name, "name1");
        assert_eq!(parsed[0].mode, ParticipantMode::Creditor);
        assert_eq!(parsed[0].amount, None);
        assert_eq!(rest, "  ");
        Ok(())
    }

    #[test]
    fn test_parse_message() {
        assert_eq!(parse_message("- abc  "), Ok(("", Some("abc  "))));
        assert_eq!(parse_message("- abc  def"), Ok(("", Some("abc  def"))));
    }

    #[test]
    fn test_parse() -> anyhow::Result<()> {
        let (rest, expense) = parse_expense(
            " @creditor1 creditor2/-21.1 34.3   debtor1 debtor2/3  @debtor3/1  - yoh",
        )?;

        assert_eq!(expense.participants[0].name, "creditor1");
        assert_eq!(expense.participants[0].mode, ParticipantMode::Creditor);
        assert_eq!(expense.participants[0].amount, None);

        assert_eq!(expense.participants[1].name, "creditor2");
        assert_eq!(expense.participants[1].mode, ParticipantMode::Creditor);
        assert_eq!(expense.participants[1].amount, Some(-2110));

        assert_eq!(expense.participants[2].name, "debtor1");
        assert_eq!(expense.participants[2].mode, ParticipantMode::Debtor);
        assert_eq!(expense.participants[2].amount, None);

        assert_eq!(expense.participants[3].name, "debtor2");
        assert_eq!(expense.participants[3].mode, ParticipantMode::Debtor);
        assert_eq!(expense.participants[3].amount, Some(300));

        assert_eq!(expense.participants[4].name, "debtor3");
        assert_eq!(expense.participants[4].mode, ParticipantMode::Debtor);
        assert_eq!(expense.participants[4].amount, Some(100));

        assert_eq!(expense.amount, 3430);
        assert_eq!(expense.message, Some("yoh".to_string()));
        assert_eq!(rest, "");

        let (rest, expense) =
            parse_expense(" creditor1 creditor2/-21.1 34.3   debtor1 debtor2/3  debtor3/1")?;
        assert_eq!(expense.message, None);
        assert_eq!(rest, "");

        Ok(())
    }
}
