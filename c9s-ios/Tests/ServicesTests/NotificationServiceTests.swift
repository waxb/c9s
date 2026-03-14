import Testing
import Foundation
@testable import C9sLib

/// Tests for NotificationService's pure logic methods.
/// Covers notification payload parsing, action routing, and device token encoding.
///
/// Critical path: push notification taps route through parseNotificationPayload → handleAction.
/// A regression here means users tap a notification and nothing happens (silent failure).
@Suite("NotificationService Tests")
@MainActor
struct NotificationServiceTests {

    private let service = NotificationService.shared

    // MARK: - parseNotificationPayload

    @Test("Parses payload with all fields")
    func parseFullPayload() {
        let userInfo: [AnyHashable: Any] = [
            "implementationId": "impl-notify-1",
            "category": "IMPL_COMPLETED",
            "title": "Implementation completed",
            "message": "Your fix for login bug is done",
        ]

        let payload = service.parseNotificationPayload(userInfo)

        #expect(payload != nil)
        #expect(payload?.implementationId == "impl-notify-1")
        #expect(payload?.category == "IMPL_COMPLETED")
        #expect(payload?.title == "Implementation completed")
        #expect(payload?.message == "Your fix for login bug is done")
    }

    @Test("Parses payload with only implementationId (optional fields nil)")
    func parseMinimalPayload() {
        let userInfo: [AnyHashable: Any] = [
            "implementationId": "impl-notify-2",
        ]

        let payload = service.parseNotificationPayload(userInfo)

        #expect(payload != nil)
        #expect(payload?.implementationId == "impl-notify-2")
        #expect(payload?.category == nil)
        #expect(payload?.title == nil)
        #expect(payload?.message == nil)
    }

    @Test("Returns nil when implementationId is missing")
    func parseMissingId() {
        let userInfo: [AnyHashable: Any] = [
            "category": "IMPL_FAILED",
            "title": "Something failed",
        ]

        let payload = service.parseNotificationPayload(userInfo)
        #expect(payload == nil)
    }

    @Test("Returns nil for empty payload")
    func parseEmptyPayload() {
        let userInfo: [AnyHashable: Any] = [:]
        let payload = service.parseNotificationPayload(userInfo)
        #expect(payload == nil)
    }

    @Test("Returns nil when implementationId is wrong type")
    func parseWrongType() {
        let userInfo: [AnyHashable: Any] = [
            "implementationId": 12345, // Int instead of String
        ]

        let payload = service.parseNotificationPayload(userInfo)
        #expect(payload == nil)
    }

    // MARK: - handleAction

    @Test("VIEW_DETAIL action returns navigateToDetail")
    func handleViewDetail() {
        let payload = NotificationPayload(
            implementationId: "impl-action-1",
            category: nil, title: nil, message: nil
        )

        let action = service.handleAction("VIEW_DETAIL", payload: payload)

        if case .navigateToDetail(let id) = action {
            #expect(id == "impl-action-1")
        } else {
            Issue.record("Expected .navigateToDetail, got \(action)")
        }
    }

    @Test("SEND_MESSAGE action returns navigateToDetail")
    func handleSendMessage() {
        let payload = NotificationPayload(
            implementationId: "impl-action-2",
            category: nil, title: nil, message: nil
        )

        let action = service.handleAction("SEND_MESSAGE", payload: payload)

        if case .navigateToDetail(let id) = action {
            #expect(id == "impl-action-2")
        } else {
            Issue.record("Expected .navigateToDetail, got \(action)")
        }
    }

    @Test("OPEN_PR action returns navigateToDetail")
    func handleOpenPR() {
        let payload = NotificationPayload(
            implementationId: "impl-action-3",
            category: nil, title: nil, message: nil
        )

        let action = service.handleAction("OPEN_PR", payload: payload)

        if case .navigateToDetail(let id) = action {
            #expect(id == "impl-action-3")
        } else {
            Issue.record("Expected .navigateToDetail, got \(action)")
        }
    }

    @Test("RESTART action returns restart")
    func handleRestart() {
        let payload = NotificationPayload(
            implementationId: "impl-action-4",
            category: nil, title: nil, message: nil
        )

        let action = service.handleAction("RESTART", payload: payload)

        if case .restart(let id) = action {
            #expect(id == "impl-action-4")
        } else {
            Issue.record("Expected .restart, got \(action)")
        }
    }

    @Test("Unknown action identifier defaults to navigateToDetail")
    func handleUnknownAction() {
        let payload = NotificationPayload(
            implementationId: "impl-action-5",
            category: nil, title: nil, message: nil
        )

        let action = service.handleAction("UNKNOWN_ACTION", payload: payload)

        if case .navigateToDetail(let id) = action {
            #expect(id == "impl-action-5")
        } else {
            Issue.record("Expected .navigateToDetail for unknown action, got \(action)")
        }
    }

    // MARK: - Device Token Encoding

    @Test("Device token data is hex-encoded correctly")
    func deviceTokenHexEncoding() {
        // Simulate APNs device token bytes
        let tokenBytes: [UInt8] = [0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89]
        let tokenData = Data(tokenBytes)

        service.didRegisterForRemoteNotifications(deviceToken: tokenData)

        #expect(service.deviceToken == "abcdef0123456789")
    }

    // MARK: - Category Raw Values

    @Test("Notification categories have correct raw values")
    func categoryRawValues() {
        #expect(NotificationService.Category.implementationCompleted.rawValue == "IMPL_COMPLETED")
        #expect(NotificationService.Category.implementationFailed.rawValue == "IMPL_FAILED")
        #expect(NotificationService.Category.waitingForInput.rawValue == "IMPL_WAITING_INPUT")
        #expect(NotificationService.Category.prReady.rawValue == "IMPL_PR_READY")
    }

    @Test("Action identifiers have correct raw values")
    func actionRawValues() {
        #expect(NotificationService.Action.viewDetail.rawValue == "VIEW_DETAIL")
        #expect(NotificationService.Action.sendMessage.rawValue == "SEND_MESSAGE")
        #expect(NotificationService.Action.openPR.rawValue == "OPEN_PR")
        #expect(NotificationService.Action.restart.rawValue == "RESTART")
    }
}
