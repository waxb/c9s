import Testing
import Foundation
@testable import C9sLib

/// Tests for DeepLinkRouter URL parsing and navigation state.
/// Covers URL scheme validation, path parsing, action handling, and state consumption.
///
/// Critical path: deep links from push notifications and URL schemes route through
/// handleURL/handle. A regression here silently breaks notification tap navigation.
@Suite("DeepLinkRouter Tests")
@MainActor
struct DeepLinkRouterTests {

    private let router = DeepLinkRouter.shared

    // Clean up pending state before each test
    private func cleanRouter() {
        router.consumePendingNavigation()
    }

    // MARK: - handleURL

    @Test("Handles valid implementation deep link URL")
    func handleValidURL() {
        cleanRouter()
        let url = URL(string: "c9s://implementation/impl-deep-1")!

        let handled = router.handleURL(url)

        #expect(handled == true)
        #expect(router.pendingImplementationId == "impl-deep-1")
        #expect(router.hasPendingNavigation == true)
        cleanRouter()
    }

    @Test("Rejects wrong URL scheme")
    func rejectWrongScheme() {
        cleanRouter()
        let url = URL(string: "https://implementation/impl-123")!

        let handled = router.handleURL(url)

        #expect(handled == false)
        #expect(router.pendingImplementationId == nil)
        cleanRouter()
    }

    @Test("Rejects URL with wrong host")
    func rejectWrongHost() {
        cleanRouter()
        let url = URL(string: "c9s://settings/something")!

        let handled = router.handleURL(url)

        #expect(handled == false)
        #expect(router.pendingImplementationId == nil)
        cleanRouter()
    }

    @Test("Rejects URL with no path component (no implementation ID)")
    func rejectNoPath() {
        cleanRouter()
        let url = URL(string: "c9s://implementation")!

        let handled = router.handleURL(url)

        // pathComponents after filtering "/" should be empty
        #expect(handled == false)
        cleanRouter()
    }

    // MARK: - handle (NotificationAction)

    @Test("handle navigateToDetail sets pending ID")
    func handleNavigateToDetail() {
        cleanRouter()
        router.handle(.navigateToDetail("impl-nav-1"))

        #expect(router.pendingImplementationId == "impl-nav-1")
        #expect(router.hasPendingNavigation == true)
        cleanRouter()
    }

    @Test("handle restart sets pending ID")
    func handleRestart() {
        cleanRouter()
        router.handle(.restart("impl-restart-1"))

        #expect(router.pendingImplementationId == "impl-restart-1")
        cleanRouter()
    }

    // MARK: - consumePendingNavigation

    @Test("consumePendingNavigation clears pending state")
    func consumeClears() {
        cleanRouter()
        router.handle(.navigateToDetail("impl-consume-1"))
        #expect(router.hasPendingNavigation == true)

        router.consumePendingNavigation()

        #expect(router.pendingImplementationId == nil)
        #expect(router.hasPendingNavigation == false)
    }

    @Test("consumePendingNavigation is safe when nothing pending")
    func consumeWhenEmpty() {
        cleanRouter()
        #expect(router.hasPendingNavigation == false)

        // Should not crash
        router.consumePendingNavigation()

        #expect(router.hasPendingNavigation == false)
    }

    // MARK: - hasPendingNavigation

    @Test("hasPendingNavigation reflects pending state")
    func hasPendingNavigation() {
        cleanRouter()
        #expect(router.hasPendingNavigation == false)

        router.handle(.navigateToDetail("impl-pending"))
        #expect(router.hasPendingNavigation == true)

        router.consumePendingNavigation()
        #expect(router.hasPendingNavigation == false)
    }
}
