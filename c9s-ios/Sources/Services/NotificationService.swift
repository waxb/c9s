import Foundation
import UserNotifications
import UIKit

/// Service for managing push notification registration, permissions, and handling.
/// Handles APNs registration, device token management, and notification categorization.
@MainActor
final class NotificationService: NSObject, ObservableObject {

    static let shared = NotificationService()

    /// Whether push notifications are authorized.
    @Published var isAuthorized = false

    /// Current device token (hex string).
    @Published var deviceToken: String?

    /// Notification categories for implementation events.
    enum Category: String {
        case implementationCompleted = "IMPL_COMPLETED"
        case implementationFailed = "IMPL_FAILED"
        case waitingForInput = "IMPL_WAITING_INPUT"
        case prReady = "IMPL_PR_READY"
    }

    /// Actions available on notifications.
    enum Action: String {
        case viewDetail = "VIEW_DETAIL"
        case sendMessage = "SEND_MESSAGE"
        case openPR = "OPEN_PR"
        case restart = "RESTART"
    }

    private override init() {
        super.init()
    }

    // MARK: - Registration

    /// Request notification permission and register for remote notifications.
    func requestAuthorization() async -> Bool {
        let center = UNUserNotificationCenter.current()

        do {
            let granted = try await center.requestAuthorization(options: [.alert, .badge, .sound])
            isAuthorized = granted

            if granted {
                registerCategories()
                UIApplication.shared.registerForRemoteNotifications()
            }

            return granted
        } catch {
            return false
        }
    }

    /// Check current authorization status.
    func checkAuthorizationStatus() async {
        let center = UNUserNotificationCenter.current()
        let settings = await center.notificationSettings()
        isAuthorized = settings.authorizationStatus == .authorized
    }

    /// Called when APNs returns a device token.
    func didRegisterForRemoteNotifications(deviceToken token: Data) {
        let tokenString = token.map { String(format: "%02x", $0) }.joined()
        self.deviceToken = tokenString
    }

    /// Called when APNs registration fails.
    func didFailToRegisterForRemoteNotifications(error: Error) {
        // Log but don't surface to user — notifications are optional
    }

    // MARK: - Categories

    /// Register notification categories with interactive actions.
    private func registerCategories() {
        let center = UNUserNotificationCenter.current()

        // Completed: View Detail
        let completedCategory = UNNotificationCategory(
            identifier: Category.implementationCompleted.rawValue,
            actions: [
                UNNotificationAction(
                    identifier: Action.viewDetail.rawValue,
                    title: "View Details",
                    options: .foreground
                ),
            ],
            intentIdentifiers: []
        )

        // Failed: View Detail, Restart
        let failedCategory = UNNotificationCategory(
            identifier: Category.implementationFailed.rawValue,
            actions: [
                UNNotificationAction(
                    identifier: Action.viewDetail.rawValue,
                    title: "View Details",
                    options: .foreground
                ),
                UNNotificationAction(
                    identifier: Action.restart.rawValue,
                    title: "Restart",
                    options: .foreground
                ),
            ],
            intentIdentifiers: []
        )

        // Waiting for Input: View Detail, Send Message
        let waitingCategory = UNNotificationCategory(
            identifier: Category.waitingForInput.rawValue,
            actions: [
                UNNotificationAction(
                    identifier: Action.viewDetail.rawValue,
                    title: "View Details",
                    options: .foreground
                ),
                UNNotificationAction(
                    identifier: Action.sendMessage.rawValue,
                    title: "Respond",
                    options: .foreground
                ),
            ],
            intentIdentifiers: []
        )

        // PR Ready: View Detail, Open PR
        let prReadyCategory = UNNotificationCategory(
            identifier: Category.prReady.rawValue,
            actions: [
                UNNotificationAction(
                    identifier: Action.viewDetail.rawValue,
                    title: "View Details",
                    options: .foreground
                ),
                UNNotificationAction(
                    identifier: Action.openPR.rawValue,
                    title: "Open PR",
                    options: .foreground
                ),
            ],
            intentIdentifiers: []
        )

        center.setNotificationCategories([
            completedCategory,
            failedCategory,
            waitingCategory,
            prReadyCategory,
        ])
    }

    // MARK: - Handling

    /// Parse notification payload and return the implementation ID for deep linking.
    func parseNotificationPayload(_ userInfo: [AnyHashable: Any]) -> NotificationPayload? {
        guard let implementationId = userInfo["implementationId"] as? String else {
            return nil
        }
        return NotificationPayload(
            implementationId: implementationId,
            category: userInfo["category"] as? String,
            title: userInfo["title"] as? String,
            message: userInfo["message"] as? String
        )
    }

    /// Handle a notification action (from interactive notification buttons).
    func handleAction(_ actionIdentifier: String, payload: NotificationPayload) -> NotificationAction {
        switch actionIdentifier {
        case Action.viewDetail.rawValue:
            return .navigateToDetail(payload.implementationId)
        case Action.sendMessage.rawValue:
            return .navigateToDetail(payload.implementationId)
        case Action.openPR.rawValue:
            return .navigateToDetail(payload.implementationId)
        case Action.restart.rawValue:
            return .restart(payload.implementationId)
        default:
            return .navigateToDetail(payload.implementationId)
        }
    }
}

// MARK: - Types

struct NotificationPayload {
    let implementationId: String
    let category: String?
    let title: String?
    let message: String?
}

enum NotificationAction {
    case navigateToDetail(String)
    case restart(String)
}

// MARK: - UNUserNotificationCenterDelegate

extension NotificationService: UNUserNotificationCenterDelegate {

    /// Handle notification when app is in foreground.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .badge, .sound]
    }

    /// Handle notification tap or action.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse
    ) async {
        let userInfo = response.notification.request.content.userInfo
        await MainActor.run {
            guard let payload = parseNotificationPayload(userInfo) else { return }
            let action = handleAction(response.actionIdentifier, payload: payload)
            DeepLinkRouter.shared.handle(action)
        }
    }
}
