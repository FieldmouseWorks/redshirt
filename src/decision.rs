//! Typed judgments and caller-owned uncertainty policy. No execution authority.
use crate::contract::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Questions = BTreeMap<String, Question>;
pub type Answers = BTreeMap<String, Answer>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Question {
    Choice {
        instructions: String,
        criteria: BTreeMap<String, String>,
    },
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
    Noul {
        instructions: String,
    },
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 2048
}
pub fn probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

impl Question {
    pub fn validate(&self) -> Result<()> {
        let valid = match self {
            Self::Choice {
                instructions,
                criteria,
            } => {
                text(instructions)
                    && (1..=255).contains(&criteria.len())
                    && criteria.iter().all(|(id, description)| {
                        !id.is_empty() && id.chars().count() <= 80 && description.len() <= 2048
                    })
            }
            Self::Score {
                instructions,
                criteria,
            } => {
                text(instructions)
                    && (2..=10).contains(&criteria.len())
                    && criteria.iter().all(|s| text(s))
            }
            Self::Noul { instructions } => text(instructions),
        };
        require(valid, "invalid_question")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Score {
        score: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Noul {
        noul: f64,
    },
}

/// Explicit consumer tolerance retained from the Conary/Python provider.
/// Original values are never normalized or replaced.
pub const TOTAL_TOLERANCE: f64 = 0.01 + 1e-12;

fn distribution(values: &BTreeMap<String, f64>, confidence: f64) -> Result<f64> {
    require(
        probability(confidence) && values.values().all(|p| probability(*p)),
        "invalid_probabilities",
    )?;
    let total: f64 = values.values().sum();
    require((total - 1.0).abs() <= TOTAL_TOLERANCE, "probability_total")?;
    Ok(total)
}

impl Answer {
    /// Validate against the question, including all answer/option bindings.
    /// Returns a distribution total for the receipt (Noul has no distribution).
    pub fn validate_for(&self, question: &Question) -> Result<Option<f64>> {
        question.validate()?;
        match (self, question) {
            (
                Self::Choice {
                    choice,
                    probabilities,
                    confidence,
                },
                Question::Choice { criteria, .. },
            ) => {
                require(
                    probabilities.keys().eq(criteria.keys()) && criteria.contains_key(choice),
                    "invalid_choice",
                )?;
                let total = distribution(probabilities, *confidence)?;
                require(
                    probabilities.values().all(|p| *p <= probabilities[choice]),
                    "choice_not_maximum",
                )?;
                Ok(Some(total))
            }
            (
                Self::Score {
                    score,
                    legend,
                    probabilities,
                    confidence,
                },
                Question::Score { criteria, .. },
            ) => {
                let expected: BTreeMap<_, _> = criteria
                    .iter()
                    .enumerate()
                    .map(|(i, value)| (i.to_string(), value.clone()))
                    .collect();
                require(
                    *legend == expected && probabilities.keys().all(|k| expected.contains_key(k)),
                    "invalid_score_levels",
                )?;
                let total = distribution(probabilities, *confidence)?;
                let highest = (criteria.len() - 1) as f64;
                require(
                    score.is_finite() && (0.0..=highest).contains(score),
                    "invalid_score",
                )?;
                let weighted: f64 = criteria
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        i as f64 * probabilities.get(&i.to_string()).copied().unwrap_or(0.0)
                    })
                    .sum();
                // Allow the same accepted total deviation plus rounding of the
                // reported mean; this checks consistency without changing data.
                require(
                    (weighted - score).abs() <= highest * TOTAL_TOLERANCE + 1e-6,
                    "score_distribution_mismatch",
                )?;
                Ok(Some(total))
            }
            (Self::Noul { noul }, Question::Noul { .. }) => {
                require(probability(*noul), "invalid_probability")?;
                Ok(None)
            }
            _ => Err("answer_type_mismatch".into()),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfidencePolicy {
    /// No universal threshold: the invoking application must choose one.
    pub min_confidence: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Accept,
    Abstain,
}

#[derive(Clone, Debug, Serialize)]
pub struct PolicyDecision {
    pub action: String,
    pub confidence: f64,
    pub minimum: Option<f64>,
    pub disposition: Disposition,
}

impl ConfidencePolicy {
    pub fn validate(&self) -> Result<()> {
        require(
            self.min_confidence.is_none_or(probability),
            "invalid_confidence_policy",
        )
    }

    pub fn assess(&self, answer: &Answer) -> Result<PolicyDecision> {
        self.validate()?;
        let Answer::Choice {
            choice, confidence, ..
        } = answer
        else {
            return Err("action_not_choice".into());
        };
        require(probability(*confidence), "invalid_confidence")?;
        Ok(PolicyDecision {
            action: choice.clone(),
            confidence: *confidence,
            minimum: self.min_confidence,
            disposition: if self
                .min_confidence
                .is_some_and(|minimum| *confidence < minimum)
            {
                Disposition::Abstain
            } else {
                Disposition::Accept
            },
        })
    }
}
