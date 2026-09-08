#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPricing {
    pub input: f64,
    pub output: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub cache_read: f64,
}

const fn price(input: f64, output: f64, w5m: f64, w1h: f64, read: f64) -> ModelPricing {
    ModelPricing {
        input,
        output,
        cache_write_5m: w5m,
        cache_write_1h: w1h,
        cache_read: read,
    }
}

const FABLE_5_1: ModelPricing = price(10.0, 50.0, 12.5, 20.0, 0.25);
const FABLE_5: ModelPricing = price(10.0, 50.0, 12.5, 20.0, 1.0);
const OPUS_CURRENT: ModelPricing = price(5.0, 25.0, 6.25, 10.0, 0.5);
const OPUS_LEGACY: ModelPricing = price(15.0, 75.0, 18.75, 30.0, 1.5);
const SONNET_5: ModelPricing = price(2.0, 10.0, 2.5, 4.0, 0.2);
const SONNET_4: ModelPricing = price(3.0, 15.0, 3.75, 6.0, 0.3);
const HAIKU_4_5: ModelPricing = price(1.0, 5.0, 1.25, 2.0, 0.1);
const HAIKU_3_5: ModelPricing = price(0.8, 4.0, 1.0, 1.6, 0.08);

const WINDOW_1M: u64 = 1_000_000;
const WINDOW_200K: u64 = 200_000;

struct ModelSpec {
    needle: &'static str,
    pricing: ModelPricing,
    context_window: u64,
}

const fn spec(needle: &'static str, pricing: ModelPricing, context_window: u64) -> ModelSpec {
    ModelSpec {
        needle,
        pricing,
        context_window,
    }
}

const SPECS: &[ModelSpec] = &[
    spec("fable-5-1", FABLE_5_1, WINDOW_1M),
    spec("mythos-5-1", FABLE_5_1, WINDOW_1M),
    spec("fable", FABLE_5, WINDOW_1M),
    spec("mythos", FABLE_5, WINDOW_1M),
    spec("opus-5", OPUS_CURRENT, WINDOW_1M),
    spec("opus-4-8", OPUS_CURRENT, WINDOW_1M),
    spec("opus-4-7", OPUS_CURRENT, WINDOW_1M),
    spec("opus-4-6", OPUS_CURRENT, WINDOW_1M),
    spec("opus-4-5", OPUS_CURRENT, WINDOW_200K),
    spec("opus-4-1", OPUS_LEGACY, WINDOW_200K),
    spec("opus-4", OPUS_LEGACY, WINDOW_200K),
    spec("opus", OPUS_CURRENT, WINDOW_1M),
    spec("sonnet-5", SONNET_5, WINDOW_1M),
    spec("sonnet-4-6", SONNET_4, WINDOW_1M),
    spec("sonnet-4-5", SONNET_4, WINDOW_200K),
    spec("sonnet-4", SONNET_4, WINDOW_200K),
    spec("sonnet-3-7", SONNET_4, WINDOW_200K),
    spec("sonnet", SONNET_4, WINDOW_1M),
    spec("haiku-4-5", HAIKU_4_5, WINDOW_200K),
    spec("haiku-3-5", HAIKU_3_5, WINDOW_200K),
    spec("haiku", HAIKU_4_5, WINDOW_200K),
];

fn lookup(model: &str) -> Option<&'static ModelSpec> {
    SPECS.iter().find(|s| model.contains(s.needle))
}

pub fn pricing_for(model: &str) -> ModelPricing {
    lookup(model).map(|s| s.pricing).unwrap_or(SONNET_4)
}

pub fn context_window(model: &str) -> u64 {
    if model.contains("[1m]") {
        return WINDOW_1M;
    }
    lookup(model)
        .map(|s| s.context_window)
        .unwrap_or(WINDOW_200K)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specificity_ordering() {
        assert_eq!(pricing_for("claude-opus-4-1-20250805"), OPUS_LEGACY);
        assert_eq!(pricing_for("claude-opus-4-20250514"), OPUS_LEGACY);
        assert_eq!(pricing_for("claude-opus-4-6"), OPUS_CURRENT);
        assert_eq!(pricing_for("claude-opus-4-5-20251101"), OPUS_CURRENT);
        assert_eq!(pricing_for("claude-opus-5"), OPUS_CURRENT);
        assert_eq!(pricing_for("claude-fable-5-1"), FABLE_5_1);
        assert_eq!(pricing_for("claude-fable-5"), FABLE_5);
        assert_eq!(pricing_for("claude-mythos-5-1"), FABLE_5_1);
        assert_eq!(pricing_for("claude-sonnet-5"), SONNET_5);
        assert_eq!(pricing_for("claude-sonnet-4-6"), SONNET_4);
        assert_eq!(pricing_for("claude-haiku-4-5-20251001"), HAIKU_4_5);
        assert_eq!(pricing_for("claude-3-5-haiku-20241022"), HAIKU_4_5);
        assert_eq!(pricing_for("claude-haiku-3-5"), HAIKU_3_5);
    }

    #[test]
    fn test_unknown_model_uses_sonnet_rates() {
        assert_eq!(pricing_for(""), SONNET_4);
        assert_eq!(pricing_for("gpt-9"), SONNET_4);
    }

    #[test]
    fn test_cache_multipliers() {
        let p = pricing_for("claude-sonnet-4-6");
        assert!((p.cache_write_5m - p.input * 1.25).abs() < 1e-9);
        assert!((p.cache_write_1h - p.input * 2.0).abs() < 1e-9);
        assert!((p.cache_read - p.input * 0.1).abs() < 1e-9);
        let fable = pricing_for("claude-fable-5-1");
        assert!((fable.cache_read - 0.25).abs() < 1e-9);
    }

    #[test]
    fn test_context_window() {
        assert_eq!(context_window("claude-fable-5-1"), WINDOW_1M);
        assert_eq!(context_window("claude-opus-4-6"), WINDOW_1M);
        assert_eq!(context_window("claude-opus-4-5"), WINDOW_200K);
        assert_eq!(context_window("claude-sonnet-5"), WINDOW_1M);
        assert_eq!(context_window("claude-sonnet-4-5"), WINDOW_200K);
        assert_eq!(context_window("claude-sonnet-4-5[1m]"), WINDOW_1M);
        assert_eq!(context_window("claude-haiku-4-5"), WINDOW_200K);
        assert_eq!(context_window(""), WINDOW_200K);
    }
}
