import UIKit

/// Haptic feedback helpers for user interactions.
@MainActor
enum HapticFeedback {
    /// Light impact for button taps and selections.
    static func light() {
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
    }

    /// Medium impact for significant actions.
    static func medium() {
        UIImpactFeedbackGenerator(style: .medium).impactOccurred()
    }

    /// Success notification for completed actions.
    static func success() {
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }

    /// Error notification for failed actions.
    static func error() {
        UINotificationFeedbackGenerator().notificationOccurred(.error)
    }

    /// Warning notification for important state changes.
    static func warning() {
        UINotificationFeedbackGenerator().notificationOccurred(.warning)
    }

    /// Selection changed feedback.
    static func selection() {
        UISelectionFeedbackGenerator().selectionChanged()
    }
}
