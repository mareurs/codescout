//! One measure of the response as delivered.
//!
//! The inline limit is a limit on the COMPACT SERIALIZED JSON of the whole response, with the
//! cut record (`CUT_FIELDS_KEY`) stripped, because that is what the caller receives and what
//! `Tool::call_content` judges. A body measured in raw bytes, or a response measured before its
//! last keys were added, is a different thing, and every tool that did that returned a response
//! `call_content` then buffered a second time (see
//! `docs/adrs/2026-10-07-one-measure-of-the-delivered-response.md`).
//!
//! Code that decides "inline or buffered" builds the response it would return and asks one of
//! these three questions of it:
//!
//! - [`response_fits`]: is this response, as delivered, within the limit?
//! - [`response_room`]: how many serialized bytes are left beside this response's other keys?
//! - [`body_alone_overflows`]: is a raw body, by itself, already too large? It answers in ONE
//!   direction only: `true` proves the response is over; `false` proves nothing.

use serde_json::Value;

use super::types::{delivered_len, exceeds_inline_limit_len, INLINE_MAX_RESPONSE_LEN};

/// The response `value`, serialized as it will be delivered, is within the inline limit.
///
/// The measure `call_content` applies: compact JSON, less the cut record the backstop strips.
pub(crate) fn response_fits(value: &Value) -> bool {
    !exceeds_inline_limit_len(delivered_len(value))
}

/// Serialized bytes left for one more field beside `widest`.
///
/// `widest` is the response with that field present but empty and every optional key at the
/// widest value it can take. The real response carries a subset of those keys with values no
/// wider, so a field whose ESCAPED length fits this room keeps the response within the limit. A
/// response built to exactly this room lands ON the limit, which is what the limit is for.
///
/// The cost of the field's own key and quotes is already in `widest` when the field is present
/// as `""`; a caller that builds its skeleton WITHOUT the field must subtract that key itself.
pub(crate) fn response_room(widest: &Value) -> usize {
    INLINE_MAX_RESPONSE_LEN.saturating_sub(delivered_len(widest))
}

/// A raw body, ALONE, is already over the limit.
///
/// Escaping never makes text shorter and the response also carries quotes and other keys, so a
/// body whose raw length is over the limit makes any response that holds it over too. `true` is a
/// proof; `false` is not: 5,000 `"` is 5,000 raw bytes and 10,014 serialized. That is why this
/// returns only the overflow direction and is named for it: it can skip building a candidate that
/// cannot fit, and it cannot be used as a gate that says "this fits".
pub(crate) fn body_alone_overflows(raw: &str) -> bool {
    exceeds_inline_limit_len(raw.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::core::types::{record_cut, CUT_FIELDS_KEY};
    use serde_json::json;

    /// `{"c":""}` is 8 bytes, so a field of `n` single-byte characters makes a response of `n + 8`.
    const EMPTY_C: usize = 8;

    fn with_c(n: usize) -> Value {
        json!({ "c": "a".repeat(n) })
    }

    #[test]
    fn a_response_exactly_on_the_limit_fits_and_one_byte_more_does_not() {
        let edge = INLINE_MAX_RESPONSE_LEN - EMPTY_C;
        assert_eq!(with_c(edge).to_string().len(), INLINE_MAX_RESPONSE_LEN);
        assert!(response_fits(&with_c(edge)), "on the limit fits");
        assert!(!response_fits(&with_c(edge + 1)), "one byte over does not");
    }

    #[test]
    fn the_cut_record_is_not_counted_because_it_is_never_delivered() {
        let edge = INLINE_MAX_RESPONSE_LEN - EMPTY_C;
        let mut v = with_c(edge);
        record_cut(&mut v, "c");
        assert!(
            v.to_string().len() > INLINE_MAX_RESPONSE_LEN,
            "precondition: the record pushes the raw serialization over the limit"
        );
        assert!(
            v.get(CUT_FIELDS_KEY).is_some(),
            "precondition: the record is present"
        );
        assert!(response_fits(&v), "the delivered response is on the limit");
        assert_eq!(delivered_len(&v), INLINE_MAX_RESPONSE_LEN);
    }

    #[test]
    fn a_field_built_to_the_room_lands_on_the_limit() {
        let skeleton = json!({ "c": "", "next": "read_file(\"@file_1\", start_line=9)", "n": 12 });
        let room = response_room(&skeleton);
        let build = |n: usize| json!({ "c": "a".repeat(n), "next": "read_file(\"@file_1\", start_line=9)", "n": 12 });
        assert_eq!(build(room).to_string().len(), INLINE_MAX_RESPONSE_LEN);
        assert!(response_fits(&build(room)));
        assert!(!response_fits(&build(room + 1)), "one more byte is over");
    }

    #[test]
    fn the_room_is_zero_when_the_other_keys_alone_are_over_the_limit() {
        let huge = json!({ "c": "", "k": "x".repeat(INLINE_MAX_RESPONSE_LEN) });
        assert_eq!(response_room(&huge), 0);
    }

    #[test]
    fn the_room_does_not_count_the_cut_record() {
        let mut skeleton = json!({ "c": "" });
        let bare = response_room(&skeleton);
        record_cut(&mut skeleton, "c");
        assert_eq!(
            response_room(&skeleton),
            bare,
            "the record is stripped before delivery, so it takes no room"
        );
    }

    #[test]
    fn a_body_alone_over_the_limit_overflows_and_one_byte_fewer_does_not() {
        assert!(!body_alone_overflows(&"a".repeat(INLINE_MAX_RESPONSE_LEN)));
        assert!(body_alone_overflows(
            &"a".repeat(INLINE_MAX_RESPONSE_LEN + 1)
        ));
    }

    /// The contract that makes `body_alone_overflows` safe: `false` proves nothing. 5,000 `"` is
    /// under the limit as a raw body and over it as a serialized response. A gate that read `false`
    /// as "fits" is the defect this module exists to stop.
    #[test]
    fn false_proves_nothing_a_body_can_be_under_alone_and_over_in_the_response() {
        let quotes = "\"".repeat(5_000);
        assert!(!body_alone_overflows(&quotes));
        assert!(!response_fits(&json!({ "content": quotes })));
    }

    /// Soundness of the one direction it does answer: for every escape class, whenever the raw
    /// body alone overflows, the response holding it does not fit. Swept across the edge.
    #[test]
    fn a_body_that_overflows_alone_never_fits_in_any_response() {
        for ch in ["a", "\"", "\\", "\u{1}", "\u{1b}", "\u{20ac}", "\u{1F600}"] {
            let unit = ch.len();
            let edge = INLINE_MAX_RESPONSE_LEN / unit;
            for n in edge.saturating_sub(3)..=edge + 3 {
                let body = ch.repeat(n);
                if body_alone_overflows(&body) {
                    assert!(
                        !response_fits(&json!({ "content": body })),
                        "{ch:?} x{n}: the body alone overflows, so the response cannot fit"
                    );
                }
            }
        }
    }
}
