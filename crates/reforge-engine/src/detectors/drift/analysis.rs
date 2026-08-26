include!("analysis/messages_and_boundaries.rs");
include!("analysis/source_syntax.rs");
include!("analysis/naming.rs");
include!("analysis/constants.rs");

use crate::detectors::concepts::{is_useful_concept_word, normalize_word, split_identifier_words};
