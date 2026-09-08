use std::collections::HashSet;

const ADJECTIVES: &[&str] = &[
    "agile", "amber", "ancient", "bold", "brave", "bright", "brisk", "calm", "candid", "clever",
    "cosmic", "crisp", "daring", "deft", "eager", "early", "easy", "fair", "fancy", "fierce",
    "fleet", "fond", "frank", "fresh", "gentle", "glad", "golden", "grand", "happy", "hardy",
    "honest", "humble", "jolly", "keen", "kind", "lively", "lucid", "lucky", "merry", "mighty",
    "modest", "nimble", "noble", "patient", "plucky", "proud", "quick", "quiet", "rapid", "sharp",
    "silent", "sleek", "smart", "snug", "steady", "swift", "tidy", "vivid", "warm", "witty",
];

const NOUNS: &[&str] = &[
    "badger", "beacon", "bison", "canyon", "cedar", "comet", "condor", "coral", "crane", "delta",
    "dune", "eagle", "ember", "falcon", "fern", "finch", "fjord", "forge", "gecko", "glacier",
    "harbor", "hawk", "heron", "ibis", "jaguar", "kestrel", "lagoon", "lantern", "lark", "lemur",
    "lynx", "maple", "marlin", "meadow", "mesa", "moose", "nebula", "newt", "orca", "osprey",
    "otter", "panda", "pebble", "pike", "quail", "raven", "reef", "ridge", "river", "robin",
    "sable", "salmon", "sparrow", "summit", "tern", "thicket", "tundra", "walrus", "willow",
    "wren",
];

const MAX_ATTEMPTS: usize = 64;

fn entropy() -> u128 {
    uuid::Uuid::new_v4().as_u128()
}

fn pick(seed: u128) -> (&'static str, &'static str) {
    let low = (seed & u64::MAX as u128) as u64;
    let high = (seed >> 64) as u64;
    let a = (low % ADJECTIVES.len() as u64) as usize;
    let n = (high % NOUNS.len() as u64) as usize;
    (ADJECTIVES[a], NOUNS[n])
}

pub fn generate(exclude: &HashSet<String>) -> String {
    for _ in 0..MAX_ATTEMPTS {
        let (a, n) = pick(entropy());
        let name = format!("{}-{}", a, n);
        if !exclude.contains(&name) {
            return name;
        }
    }
    let (a, n) = pick(entropy());
    let suffix = (entropy() % 1000) as u32;
    format!("{}-{}-{}", a, n, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_word_list_ok(list: &[&str]) -> bool {
        let unique: HashSet<&str> = list.iter().copied().collect();
        list.len() >= 40
            && unique.len() == list.len()
            && list
                .iter()
                .all(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase()))
    }

    #[test]
    fn test_word_lists_are_lowercase_unique_and_large_enough() {
        assert!(is_word_list_ok(ADJECTIVES));
        assert!(is_word_list_ok(NOUNS));
    }

    #[test]
    fn test_generate_shape() {
        let name = generate(&HashSet::new());
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert!(ADJECTIVES.contains(&parts[0]));
        assert!(NOUNS.contains(&parts[1]));
    }

    #[test]
    fn test_generate_avoids_excluded_names() {
        let mut exclude = HashSet::new();
        for a in ADJECTIVES {
            for n in NOUNS {
                exclude.insert(format!("{}-{}", a, n));
            }
        }
        let name = generate(&exclude);
        assert!(!exclude.contains(&name));
        assert_eq!(name.split('-').count(), 3);
    }

    #[test]
    fn test_pick_is_deterministic_for_seed() {
        assert_eq!(pick(0), (ADJECTIVES[0], NOUNS[0]));
        assert_eq!(pick(1), (ADJECTIVES[1], NOUNS[0]));
        assert_eq!(pick(1u128 << 64), (ADJECTIVES[0], NOUNS[1]));
    }
}
