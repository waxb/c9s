import SwiftUI

/// Displays a relative time string like "5m ago", "2h ago", "3d ago".
struct RelativeTimeLabel: View {
    let date: Date?

    var body: some View {
        if let date {
            Text(relativeString(from: date))
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private func relativeString(from date: Date) -> String {
        let seconds = Int(Date.now.timeIntervalSince(date))
        if seconds < 0 { return "just now" }
        if seconds < 60 { return "\(seconds)s ago" }
        if seconds < 3600 { return "\(seconds / 60)m ago" }
        if seconds < 86400 { return "\(seconds / 3600)h ago" }
        if seconds < 604800 { return "\(seconds / 86400)d ago" }
        return "\(seconds / 604800)w ago"
    }
}
