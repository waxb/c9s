import SwiftUI

/// Formatted USD cost display.
struct CostLabel: View {
    let cost: Double?

    var body: some View {
        if let cost {
            Text(formatted(cost))
                .font(.caption)
                .foregroundStyle(.secondary)
                .monospacedDigit()
        }
    }

    private func formatted(_ value: Double) -> String {
        if value < 0.01 { return "<$0.01" }
        if value < 1.0 { return String(format: "$%.2f", value) }
        return String(format: "$%.2f", value)
    }
}

/// Formatted token count display.
struct TokenCountLabel: View {
    let count: Int?

    var body: some View {
        if let count {
            Text(formatted(count))
                .font(.caption)
                .foregroundStyle(.secondary)
                .monospacedDigit()
        }
    }

    private func formatted(_ value: Int) -> String {
        if value >= 1_000_000 { return String(format: "%.1fM", Double(value) / 1_000_000) }
        if value >= 1_000 { return String(format: "%.1fK", Double(value) / 1_000) }
        return "\(value)"
    }
}
