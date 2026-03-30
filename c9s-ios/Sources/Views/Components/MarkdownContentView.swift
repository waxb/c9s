import SwiftUI

/// Renders markdown text using iOS 18's AttributedString markdown support.
struct MarkdownContentView: View {
    let content: String

    var body: some View {
        if let attributed = try? AttributedString(markdown: content, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            Text(attributed)
                .font(.body)
                .textSelection(.enabled)
        } else {
            // Fallback: plain text
            Text(content)
                .font(.body)
                .textSelection(.enabled)
        }
    }
}
