import SwiftUI

/// Horizontal step progress indicator showing circles for each implementation step.
struct StepProgressView: View {
    let steps: [TervezoStep]

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                ForEach(steps) { step in
                    VStack(spacing: 4) {
                        ZStack {
                            Circle()
                                .fill(fillColor(for: step.status))
                                .frame(width: 24, height: 24)

                            statusIcon(step.status)
                                .font(.system(size: 10, weight: .bold))
                                .foregroundStyle(.white)
                        }

                        Text(step.name)
                            .font(.system(size: 9))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .frame(maxWidth: 60)
                    }

                    if step.id != steps.last?.id {
                        Rectangle()
                            .fill(connectorColor(step.status))
                            .frame(width: 16, height: 2)
                            .offset(y: -8)
                    }
                }
            }
            .padding(.horizontal, 4)
        }
    }

    private func fillColor(for status: String) -> Color {
        switch status {
        case "completed": .green
        case "running", "in_progress": .blue
        case "failed": .red
        case "waiting_for_input": .orange
        default: .gray.opacity(0.3)
        }
    }

    @ViewBuilder
    private func statusIcon(_ status: String) -> some View {
        switch status {
        case "completed":
            Image(systemName: "checkmark")
        case "running", "in_progress":
            ProgressView()
                .scaleEffect(0.5)
                .tint(.white)
        case "failed":
            Image(systemName: "xmark")
        case "waiting_for_input":
            Image(systemName: "questionmark")
        default:
            Circle()
                .fill(.gray)
                .frame(width: 6, height: 6)
        }
    }

    private func connectorColor(_ status: String) -> Color {
        switch status {
        case "completed": .green
        default: .gray.opacity(0.3)
        }
    }
}
