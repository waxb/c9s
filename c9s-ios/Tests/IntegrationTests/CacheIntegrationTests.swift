import Testing
import Foundation
import SwiftData
@testable import C9sLib

/// Integration tests for offline → online transitions.
/// Verifies that the CacheService correctly persists API data and serves
/// it when the network is unavailable, then updates when connectivity returns.
///
/// Why these tests matter: The app should provide a seamless experience
/// regardless of network state. Cached data must be accurate, stale data
/// must be replaced by fresh data, and transitions must not corrupt state.
@Suite("Cache Integration Tests")
@MainActor
struct CacheIntegrationTests {

    // MARK: - Helpers

    private func makeCacheService() throws -> (CacheService, ModelContext) {
        let schema = Schema([
            CachedImplementation.self,
            CachedWorkspace.self,
            CachedTimelineMessage.self,
            AppSettings.self,
        ])
        let config = ModelConfiguration(isStoredInMemoryOnly: true)
        let container = try ModelContainer(for: schema, configurations: [config])
        let context = container.mainContext
        return (CacheService(modelContext: context), context)
    }

    // MARK: - Offline → Online Transition

    @Test("Offline to online: cached list is available offline, then replaced by fresh data")
    func offlineToOnlineListTransition() throws {
        let (cache, _) = try makeCacheService()

        // Simulate online: sync API data into cache
        let initialList = ImplementationList(
            items: [
                ImplementationSummary(
                    id: "impl-oo-1", title: "Task A", status: "running", mode: "feature",
                    repoUrl: "https://github.com/test/repo", branch: "feat/a",
                    prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
                ImplementationSummary(
                    id: "impl-oo-2", title: "Task B", status: "completed", mode: "bugfix",
                    repoUrl: nil, branch: nil, prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
            ],
            total: 2
        )
        try cache.syncImplementationList(initialList)

        // Verify cache has data
        let cachedBefore = try cache.fetchCachedImplementations()
        #expect(cachedBefore.count == 2)

        // Simulate offline: no sync happens, but data is still accessible
        let cachedOffline = try cache.fetchCachedImplementations()
        #expect(cachedOffline.count == 2)
        #expect(cachedOffline.contains { $0.title == "Task A" })
        #expect(cachedOffline.contains { $0.title == "Task B" })

        // Simulate back online: fresh data has updates
        let freshList = ImplementationList(
            items: [
                ImplementationSummary(
                    id: "impl-oo-1", title: "Task A (updated)", status: "completed", mode: "feature",
                    repoUrl: "https://github.com/test/repo", branch: "feat/a",
                    prUrl: "https://github.com/test/repo/pull/5", prStatus: "open",
                    createdAt: Date(timeIntervalSince1970: 1709836800),
                    updatedAt: Date(timeIntervalSince1970: 1709840400)
                ),
                ImplementationSummary(
                    id: "impl-oo-2", title: "Task B", status: "completed", mode: "bugfix",
                    repoUrl: nil, branch: nil, prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
                ImplementationSummary(
                    id: "impl-oo-3", title: "Task C (new)", status: "running", mode: "feature",
                    repoUrl: nil, branch: "feat/c", prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709844000), updatedAt: nil
                ),
            ],
            total: 3
        )
        try cache.syncImplementationList(freshList)

        // Verify cache reflects fresh data
        let cachedAfter = try cache.fetchCachedImplementations()
        #expect(cachedAfter.count == 3) // Old 2 updated + 1 new

        let taskA = cachedAfter.first { $0.id == "impl-oo-1" }
        #expect(taskA?.title == "Task A (updated)")
        #expect(taskA?.status == "completed")
        #expect(taskA?.prUrl == "https://github.com/test/repo/pull/5")

        let taskC = cachedAfter.first { $0.id == "impl-oo-3" }
        #expect(taskC?.title == "Task C (new)")
        #expect(taskC?.status == "running")
    }

    @Test("Detail cache survives network failure and is replaced on reconnection")
    func detailCacheOfflineOnline() throws {
        let (cache, _) = try makeCacheService()

        // Sync initial detail
        let initialDetail = TestFixtures.makeImplementationDetail(
            id: "impl-detail-oo", status: "running"
        )
        try cache.syncImplementationDetail(initialDetail)

        // Verify cached detail exists
        let cachedList = try cache.fetchCachedImplementations()
        let cached = cachedList.first { $0.id == "impl-detail-oo" }
        #expect(cached != nil)
        #expect(cached?.status == "running")
        #expect(cached?.plan != nil)

        // Simulate fresh detail after completion
        let completedDetail = ImplementationDetail(
            id: "impl-detail-oo",
            title: "Fix login bug",
            status: "completed",
            mode: "bugfix",
            prompt: "Fix the login bug",
            plan: "## Updated Plan\n1. Fixed token validation",
            analysis: "JWT tokens now validated correctly",
            error: nil,
            isRunning: false,
            repoUrl: "https://github.com/user/repo",
            branch: "fix/login-bug",
            baseBranch: "main",
            branchPushed: true,
            prUrl: "https://github.com/user/repo/pull/10",
            prStatus: "open",
            sandboxId: nil,
            iterations: 3,
            currentIteration: 3,
            createdAt: Date(timeIntervalSince1970: 1709836800),
            updatedAt: Date(timeIntervalSince1970: 1709844000),
            steps: [],
            timelineMessageCount: 50
        )
        try cache.syncImplementationDetail(completedDetail)

        // Verify cache was updated, not duplicated
        let allCached = try cache.fetchCachedImplementations()
        #expect(allCached.count == 1) // Same ID, upserted
        let updated = allCached.first { $0.id == "impl-detail-oo" }
        #expect(updated?.status == "completed")
        #expect(updated?.plan?.contains("Updated Plan") == true)
        #expect(updated?.prUrl == "https://github.com/user/repo/pull/10")
    }

    // MARK: - Timeline Cache Accumulation

    @Test("Timeline messages accumulate across multiple syncs")
    func timelineAccumulation() throws {
        let (cache, _) = try makeCacheService()
        let implId = "impl-timeline-acc"

        // First sync: initial messages
        let batch1 = [
            TervezoTimelineMessage(
                id: "msg-batch1-1", type: "user_prompt",
                timestamp: "2026-03-07T10:00:00Z",
                rawJSON: ["text": .string("Start the task")]
            ),
            TervezoTimelineMessage(
                id: "msg-batch1-2", type: "assistant_text",
                timestamp: "2026-03-07T10:00:05Z",
                rawJSON: ["text": .string("Working on it...")]
            ),
        ]
        try cache.syncTimelineMessages(batch1, implementationId: implId)

        // Second sync: new messages (simulates SSE or polling update)
        let batch2 = [
            TervezoTimelineMessage(
                id: "msg-batch1-2", type: "assistant_text", // Duplicate — should not be inserted
                timestamp: "2026-03-07T10:00:05Z",
                rawJSON: ["text": .string("Working on it...")]
            ),
            TervezoTimelineMessage(
                id: "msg-batch2-1", type: "tool_call",
                timestamp: "2026-03-07T10:00:10Z",
                rawJSON: ["toolName": .string("Read")]
            ),
            TervezoTimelineMessage(
                id: "msg-batch2-2", type: "file_change",
                timestamp: "2026-03-07T10:01:00Z",
                rawJSON: ["filename": .string("src/auth.rs")]
            ),
        ]
        try cache.syncTimelineMessages(batch2, implementationId: implId)

        // Verify total messages (3 unique, not 5)
        let timeline = try cache.fetchCachedTimeline(implementationId: implId)
        #expect(timeline.count == 4) // 2 from batch1 + 2 new from batch2 (1 duplicate skipped)

        // Verify order
        #expect(timeline[0].messageId == "msg-batch1-1")
        #expect(timeline[1].messageId == "msg-batch1-2")
        #expect(timeline[2].messageId == "msg-batch2-1")
        #expect(timeline[3].messageId == "msg-batch2-2")
    }

    // MARK: - Workspace Sync Idempotency

    @Test("Workspace sync is idempotent: re-syncing same data doesn't duplicate")
    func workspaceSyncIdempotent() throws {
        let (cache, _) = try makeCacheService()

        let workspaces = [
            TervezoWorkspace(id: "ws-sync-1", name: "Alpha", slug: "alpha", logo: nil),
            TervezoWorkspace(id: "ws-sync-2", name: "Beta", slug: "beta", logo: "logo.png"),
        ]

        // Sync three times
        try cache.syncWorkspaces(workspaces)
        try cache.syncWorkspaces(workspaces)
        try cache.syncWorkspaces(workspaces)

        // Should still have exactly 2
        let cached = try cache.fetchCachedWorkspaces()
        #expect(cached.count == 2)
    }

    @Test("Workspace sync updates existing names")
    func workspaceSyncUpdates() throws {
        let (cache, _) = try makeCacheService()

        let initial = [
            TervezoWorkspace(id: "ws-update-1", name: "Old Name", slug: "old-name", logo: nil),
        ]
        try cache.syncWorkspaces(initial)

        let updated = [
            TervezoWorkspace(id: "ws-update-1", name: "New Name", slug: "new-name", logo: "new-logo.png"),
        ]
        try cache.syncWorkspaces(updated)

        let cached = try cache.fetchCachedWorkspaces()
        #expect(cached.count == 1)
        #expect(cached[0].name == "New Name")
    }

    // MARK: - Settings Persistence

    @Test("Settings persist across get-or-create calls and modification")
    func settingsPersistence() throws {
        let (cache, _) = try makeCacheService()

        // First call: creates defaults
        let settings1 = try cache.getOrCreateSettings()
        #expect(settings1.pollIntervalSeconds == 30)
        #expect(settings1.onboardingCompleted == false)

        // Modify
        settings1.pollIntervalSeconds = 15
        settings1.onboardingCompleted = true
        settings1.sortField = "createdAt"

        // Second call: returns same instance with modifications
        let settings2 = try cache.getOrCreateSettings()
        #expect(settings2.pollIntervalSeconds == 15)
        #expect(settings2.onboardingCompleted == true)
        #expect(settings2.sortField == "createdAt")
    }

    // MARK: - Combined API + Cache Flow

    @Test("Combined flow: API sync into cache, offline read, online refresh")
    func combinedAPICacheFlow() throws {
        let (cache, _) = try makeCacheService()

        // Phase 1: Online — sync list from API
        let apiList = ImplementationList(
            items: [
                ImplementationSummary(
                    id: "impl-combined-1", title: "Feature X", status: "running", mode: "feature",
                    repoUrl: "https://github.com/test/repo", branch: "feat/x",
                    prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
            ],
            total: 1
        )
        try cache.syncImplementationList(apiList)

        // Phase 1: Online — sync detail from API
        let apiDetail = ImplementationDetail(
            id: "impl-combined-1",
            title: "Feature X",
            status: "running",
            mode: "feature",
            prompt: "Build feature X",
            plan: "## Steps\n1. Design\n2. Implement\n3. Test",
            analysis: "Clean architecture pattern detected",
            error: nil,
            isRunning: true,
            repoUrl: "https://github.com/test/repo",
            branch: "feat/x",
            baseBranch: "main",
            branchPushed: true,
            prUrl: nil,
            prStatus: nil,
            sandboxId: "sb-combined-1",
            iterations: 1,
            currentIteration: 1,
            createdAt: Date(timeIntervalSince1970: 1709836800),
            updatedAt: Date(timeIntervalSince1970: 1709838600),
            steps: [],
            timelineMessageCount: 5
        )
        try cache.syncImplementationDetail(apiDetail)

        // Phase 1: Online — sync timeline
        let apiTimeline = [
            TervezoTimelineMessage(
                id: "msg-c1", type: "user_prompt",
                timestamp: "2026-03-07T10:00:00Z",
                rawJSON: ["text": .string("Build feature X")]
            ),
        ]
        try cache.syncTimelineMessages(apiTimeline, implementationId: "impl-combined-1")

        // Phase 2: Offline — all data available from cache
        let offlineList = try cache.fetchCachedImplementations()
        #expect(offlineList.count == 1)
        #expect(offlineList[0].status == "running")
        #expect(offlineList[0].plan?.contains("Design") == true)

        let offlineTimeline = try cache.fetchCachedTimeline(implementationId: "impl-combined-1")
        #expect(offlineTimeline.count == 1)
        #expect(offlineTimeline[0].type == "user_prompt")

        // Phase 3: Back online — implementation completed, sync fresh data
        let freshDetail = ImplementationDetail(
            id: "impl-combined-1",
            title: "Feature X",
            status: "completed",
            mode: "feature",
            prompt: "Build feature X",
            plan: "## Steps\n1. Design\n2. Implement\n3. Test\n\n**All steps completed.**",
            analysis: "Clean architecture pattern detected",
            error: nil,
            isRunning: false,
            repoUrl: "https://github.com/test/repo",
            branch: "feat/x",
            baseBranch: "main",
            branchPushed: true,
            prUrl: "https://github.com/test/repo/pull/15",
            prStatus: "open",
            sandboxId: nil,
            iterations: 3,
            currentIteration: 3,
            createdAt: Date(timeIntervalSince1970: 1709836800),
            updatedAt: Date(timeIntervalSince1970: 1709844000),
            steps: [],
            timelineMessageCount: 25
        )
        try cache.syncImplementationDetail(freshDetail)

        // Verify cache updated correctly
        let refreshedList = try cache.fetchCachedImplementations()
        #expect(refreshedList.count == 1) // Same entity, not duplicated
        #expect(refreshedList[0].status == "completed")
        #expect(refreshedList[0].prUrl == "https://github.com/test/repo/pull/15")
        #expect(refreshedList[0].plan?.contains("All steps completed") == true)
    }

    // MARK: - Cache Isolation

    @Test("Timeline messages for different implementations don't interfere")
    func timelineIsolation() throws {
        let (cache, _) = try makeCacheService()

        let messages1 = [
            TervezoTimelineMessage(id: "m1-a", type: "user_prompt", timestamp: "2026-03-07T10:00:00Z",
                                   rawJSON: ["text": .string("Prompt for impl 1")]),
            TervezoTimelineMessage(id: "m1-b", type: "assistant_text", timestamp: "2026-03-07T10:00:05Z",
                                   rawJSON: ["text": .string("Response for impl 1")]),
        ]

        let messages2 = [
            TervezoTimelineMessage(id: "m2-a", type: "user_prompt", timestamp: "2026-03-07T11:00:00Z",
                                   rawJSON: ["text": .string("Prompt for impl 2")]),
        ]

        try cache.syncTimelineMessages(messages1, implementationId: "impl-iso-1")
        try cache.syncTimelineMessages(messages2, implementationId: "impl-iso-2")

        let timeline1 = try cache.fetchCachedTimeline(implementationId: "impl-iso-1")
        let timeline2 = try cache.fetchCachedTimeline(implementationId: "impl-iso-2")

        #expect(timeline1.count == 2)
        #expect(timeline2.count == 1)
        #expect(timeline1[0].messageId == "m1-a")
        #expect(timeline2[0].messageId == "m2-a")
    }
}
