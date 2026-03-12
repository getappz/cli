//! Readability analysis using Flesch-Kincaid scoring
//!
//! Custom implementation - no external crate needed

/// Readability analysis results
pub struct ReadabilityResult {
    pub flesch_reading_ease: f64,
    pub flesch_kincaid_grade: f64,
    pub avg_words_per_sentence: f64,
    pub avg_syllables_per_word: f64,
    pub interpretation: String,
}

/// Analyze text readability using Flesch-Kincaid formula
pub fn analyze_readability(text: &str) -> ReadabilityResult {
    let sentences: Vec<&str> = text
        .split(|c: char| c == '.' || c == '!' || c == '?')
        .filter(|s| !s.trim().is_empty())
        .collect();
    
    let sentence_count = sentences.len().max(1) as f64;
    
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect();
    
    let word_count = words.len() as f64;
    
    let mut total_syllables = 0;
    for word in &words {
        total_syllables += count_syllables(word);
    }
    
    let avg_words_per_sentence = word_count / sentence_count;
    let avg_syllables_per_word = if word_count > 0.0 {
        total_syllables as f64 / word_count
    } else {
        0.0
    };
    
    // Flesch Reading Ease formula
    // 206.835 - (1.015 * ASL) - (84.6 * ASW)
    // ASL = average sentence length (words per sentence)
    // ASW = average syllables per word
    let flesch_reading_ease = 206.835 - (1.015 * avg_words_per_sentence) - (84.6 * avg_syllables_per_word);
    
    // Flesch-Kincaid Grade Level
    // (0.39 * ASL) + (11.8 * ASW) - 15.59
    let flesch_kincaid_grade = (0.39 * avg_words_per_sentence) + (11.8 * avg_syllables_per_word) - 15.59;
    
    let interpretation = match flesch_reading_ease {
        f if f >= 90.0 => "Very easy to read".to_string(),
        f if f >= 80.0 => "Easy to read".to_string(),
        f if f >= 70.0 => "Fairly easy to read".to_string(),
        f if f >= 60.0 => "Standard".to_string(),
        f if f >= 50.0 => "Fairly difficult".to_string(),
        f if f >= 30.0 => "Difficult".to_string(),
        _ => "Very difficult".to_string(),
    };
    
    ReadabilityResult {
        flesch_reading_ease: flesch_reading_ease.max(0.0).min(100.0),
        flesch_kincaid_grade: flesch_kincaid_grade.max(0.0),
        avg_words_per_sentence,
        avg_syllables_per_word,
        interpretation,
    }
}

/// Count syllables in a word (approximation)
fn count_syllables(word: &str) -> usize {
    let word = word.to_lowercase();
    let word = word.trim_matches(|c: char| !c.is_alphabetic());
    
    if word.is_empty() {
        return 1;
    }
    
    // Count vowel groups
    let mut syllables = 0;
    let mut prev_was_vowel = false;
    
    for ch in word.chars() {
        let is_vowel = matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
        
        if is_vowel && !prev_was_vowel {
            syllables += 1;
        }
        
        prev_was_vowel = is_vowel;
    }
    
    // Handle silent 'e' at the end
    if word.ends_with('e') && syllables > 1 {
        syllables -= 1;
    }
    
    // Minimum 1 syllable
    syllables.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syllable_count() {
        assert_eq!(count_syllables("test"), 1);
        assert_eq!(count_syllables("hello"), 2);
        assert_eq!(count_syllables("beautiful"), 3);
    }

    #[test]
    fn test_readability() {
        let text = "This is a simple test. It has multiple sentences. Each sentence is easy to read.";
        let result = analyze_readability(text);
        assert!(result.flesch_reading_ease > 60.0);
    }
}

