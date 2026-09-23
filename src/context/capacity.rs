//! Offline Jev admission estimates. TypeSafe publishes no Jev tokenizer or
//! model-side framing, so these OpenAI BPE counts are surrogates, not guarantees.
use super::*;
use std::sync::OnceLock;
use tiktoken_rs::{CoreBPE, cl100k_base, o200k_base, r50k_base};

pub const PUBLISHED_TOTAL: usize = 64_000;
pub const PUBLISHED_SINGLE: usize = 32_000;
pub const HEADROOM_PERCENT: usize = 20;
pub const ADMIT_TOTAL: usize = PUBLISHED_TOTAL * (100 - HEADROOM_PERCENT) / 100;
pub const ADMIT_SINGLE: usize = PUBLISHED_SINGLE * (100 - HEADROOM_PERCENT) / 100;
pub const ESTIMATOR: &str = "tiktoken-rs_0.12.0_max_r50k_cl100k_o200k_ordinary_json_v1";

struct Encoders {
    r50k: CoreBPE,
    cl100k: CoreBPE,
    o200k: CoreBPE,
}
static ENCODERS: OnceLock<Result<Encoders>> = OnceLock::new();

fn encoders() -> Result<&'static Encoders> {
    ENCODERS
        .get_or_init(|| {
            Ok(Encoders {
                r50k: r50k_base().map_err(|_| Stop::from("context_estimator_init"))?,
                cl100k: cl100k_base().map_err(|_| Stop::from("context_estimator_init"))?,
                o200k: o200k_base().map_err(|_| Stop::from("context_estimator_init"))?,
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn estimate(value: &Value) -> Result<usize> {
    let bytes = encoded(value)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| Stop::from("context_estimator_encoding"))?;
    let encoders = encoders()?;
    Ok([
        encoders.r50k.encode_ordinary(text).len(),
        encoders.cl100k.encode_ordinary(text).len(),
        encoders.o200k.encode_ordinary(text).len(),
    ]
    .into_iter()
    .max()
    .expect("three surrogate tokenizers"))
}

pub fn policy() -> Value {
    json!({"context_policy":CONTEXT_POLICY,"model":jev::MODEL,
        "estimator":ESTIMATOR,"counting":"maximum_ordinary_bpe_count_of_canonical_json",
        "surrogate_tokenizers":["r50k_base","cl100k_base","o200k_base"],
        "published_total_tokens":PUBLISHED_TOTAL,"published_state_plus_longest_question_tokens":PUBLISHED_SINGLE,
        "headroom_percent":HEADROOM_PERCENT,"admit_total_tokens":ADMIT_TOTAL,
        "admit_state_plus_longest_question_tokens":ADMIT_SINGLE,
        "exact_jev_tokenization_known":false,"provider_framing_known":false,
        "published_k_interpretation":"decimal_conservative"})
}

/// Count the actual batch and each state-plus-one-question request. JSON IDs
/// remain in the estimate although TypeSafe says question IDs are omitted from
/// inference; that and the 20% headroom make this an intentionally cautious
/// admission rule, not an exact provider count.
pub fn admit(state: &Value, questions: &Questions) -> Result<Value> {
    require(!questions.is_empty(), "context_capacity_questions")?;
    let body = request(state.clone(), questions);
    let total = estimate(&body)?;
    require(total <= ADMIT_TOTAL, "context_capacity_total")?;
    let mut longest = 0;
    for (id, question) in questions {
        let one = BTreeMap::from([(id.clone(), question.clone())]);
        longest = longest.max(estimate(&request(state.clone(), &one))?);
    }
    require(longest <= ADMIT_SINGLE, "context_capacity_single")?;
    Ok(json!({"total_estimated_tokens":total,
        "state_plus_longest_question_estimated_tokens":longest,
        "request_bytes":encoded(&body)?.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::Question;

    fn noise(length: usize) -> String {
        let mut seed = 0x1234_5678u32;
        (0..length)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                    [(seed as usize) % 62] as char
            })
            .collect()
    }

    #[test]
    fn aggregate_admission_can_fail_while_each_question_fits() {
        // This direct estimator test uses a wider synthetic batch than the
        // validated eight-question campaign protocol can encode in 8 KiB.
        let state = json!({"text":"<|endoftext|> é \" \\"});
        let questions: Questions = (0..8)
            .map(|i| {
                (
                    format!("q{i}"),
                    Question::Noul {
                        instructions: noise(14_000 + i),
                    },
                )
            })
            .collect();
        let total = estimate(&request(state.clone(), &questions)).unwrap();
        assert!(total > ADMIT_TOTAL, "total={total}");
        for (id, question) in &questions {
            let one = BTreeMap::from([(id.clone(), question.clone())]);
            assert!(estimate(&request(state.clone(), &one)).unwrap() < ADMIT_SINGLE);
        }
        assert_eq!(
            admit(&state, &questions).unwrap_err().0,
            "context_capacity_total"
        );
        let shorter: Questions = questions.into_iter().take(4).collect();
        assert!(admit(&state, &shorter).is_ok());
    }
}
