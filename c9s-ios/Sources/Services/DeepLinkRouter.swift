import Foundation
import Observation

/// Centralized router for deep links from notifications, URLs, and Spotlight.
/// Publishes navigation requests that the app root view observes.
@Observable
@MainActor
final class DeepLinkRouter {

    static let shared = DeepLinkRouter()

    /// The implementation ID to navigate to (set by notification or URL handler).
    var pendingImplementationId: String?

    /// Whether a navigation is pending.
    var hasPendingNavigation: Bool {
        pendingImplementationId != nil
    }

    private init() {}

    /// Handle a notification action by setting the pending navigation.
    func handle(_ action: NotificationAction) {
        switch action {
        case .navigateToDetail(let id):
            pendingImplementationId = id
        case .restart(let id):
            pendingImplementationId = id
            // Restart is triggered by the detail view when it detects the action
            Task {
                try? await TervezoService().restart(id: id)
            }
        }
    }

    /// Handle a deep link URL (e.g. c9s://implementation/impl-123).
    func handleURL(_ url: URL) -> Bool {
        guard url.scheme == "c9s" else { return false }

        let pathComponents = url.pathComponents.filter { $0 != "/" }

        // c9s://implementation/{id}
        if url.host == "implementation", let id = pathComponents.first {
            pendingImplementationId = id
            return true
        }

        return false
    }

    /// Consume the pending navigation (call after navigating).
    func consumePendingNavigation() {
        pendingImplementationId = nil
    }
}
