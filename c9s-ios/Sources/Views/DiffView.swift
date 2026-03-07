import SwiftUI

/// Syntax-highlighted unified diff display with green/red line coloring.
struct DiffView: View {
    let patch: String

    var body: some View {
        ScrollView(.horizontal, showsIndicators: true) {
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(patch.split(separator: "\n", omittingEmptySubsequences: false).enumerated()), id: \.offset) { _, line in
                    let lineStr = String(line)
                    Text(lineStr)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(lineColor(lineStr))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 1)
                        .background(lineBackground(lineStr))
                }
            }
        }
        .background(.fill.quinary)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private func lineColor(_ line: String) -> Color {
        if line.hasPrefix("+") && !line.hasPrefix("+++") { return .green }
        if line.hasPrefix("-") && !line.hasPrefix("---") { return .red }
        if line.hasPrefix("@@") { return .cyan }
        return .primary
    }

    private func lineBackground(_ line: String) -> Color {
        if line.hasPrefix("+") && !line.hasPrefix("+++") { return .green.opacity(0.1) }
        if line.hasPrefix("-") && !line.hasPrefix("---") { return .red.opacity(0.1) }
        if line.hasPrefix("@@") { return .cyan.opacity(0.05) }
        return .clear
    }
}
