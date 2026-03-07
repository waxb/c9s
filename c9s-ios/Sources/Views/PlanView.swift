import SwiftUI

/// Scrollable markdown display of the AI-generated plan.
struct PlanView: View {
    let plan: String?

    var body: some View {
        ScrollView {
            if let plan, !plan.isEmpty {
                MarkdownContentView(content: plan)
                    .padding()
            } else {
                ContentUnavailableView {
                    Label("No Plan Yet", systemImage: "doc.text")
                } description: {
                    Text("The plan will appear here once the AI generates it.")
                }
            }
        }
    }
}
