import SwiftUI

/// Scrollable timeline of implementation messages with auto-scroll to bottom.
struct TimelineView: View {
    let messages: [TervezoTimelineMessage]

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(messages) { message in
                        TimelineMessageView(message: message)
                            .id(message.id)
                        Divider()
                    }
                }
                .padding(.horizontal)
            }
            .onChange(of: messages.count) { _, _ in
                // Auto-scroll to bottom when new messages arrive
                if let last = messages.last {
                    withAnimation(.easeOut(duration: 0.3)) {
                        proxy.scrollTo(last.id, anchor: .bottom)
                    }
                }
            }
        }
    }
}
