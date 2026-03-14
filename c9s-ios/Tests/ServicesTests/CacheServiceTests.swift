import Testing
import Foundation
import SwiftData
@testable import C9sLib

/// Tests for CacheService offline caching behavior.
/// Uses in-memory ModelContainer to avoid filesystem side effects.
@Suite("CacheService Offline Behavior Tests")
@MainActor
struct CacheServiceTests {

    /// Creates an in-memory ModelContainer and CacheService for testing.
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

    // MARK: - Implementation Sync

    @Test("Sync list creates new cached implementations")
    func syncListCreatesNew() throws {
        let (cache, _) = try makeCacheService()
        let list = ImplementationList(
            items: [
                ImplementationSummary(
                    id: "impl-1", title: "Fix bug", status: "running", mode: "bugfix",
                    repoUrl: "https://github.com/user/repo", branch: "fix/bug",
                    prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
                ImplementationSummary(
                    id: "impl-2", title: "Add feature", status: "completed", mode: "feature",
                    repoUrl: nil, branch: nil, prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
            ],
            total: 2
        )

        try cache.syncImplementationList(list)
        let cached = try cache.fetchCachedImplementations()
        #expect(cached.count == 2)
    }

    @Test("Sync list updates existing implementations")
    func syncListUpdatesExisting() throws {
        let (cache, _) = try makeCacheService()

        // First sync: create
        let initial = ImplementationList(
            items: [
                ImplementationSummary(
                    id: "impl-1", title: "Fix bug", status: "running", mode: "bugfix",
                    repoUrl: nil, branch: nil, prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
            ],
            total: 1
        )
        try cache.syncImplementationList(initial)

        // Second sync: update status
        let updated = ImplementationList(
            items: [
                ImplementationSummary(
                    id: "impl-1", title: "Fix bug", status: "completed", mode: "bugfix",
                    repoUrl: nil, branch: nil, prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
            ],
            total: 1
        )
        try cache.syncImplementationList(updated)

        let cached = try cache.fetchCachedImplementations()
        #expect(cached.count == 1)
        #expect(cached[0].status == "completed")
    }

    @Test("Sync detail creates or updates cached implementation")
    func syncDetailUpsert() throws {
        let (cache, _) = try makeCacheService()

        let detail = TestFixtures.makeImplementationDetail(
            id: "impl-42", status: "running"
        )
        try cache.syncImplementationDetail(detail)

        let cached = try cache.fetchCachedImplementations()
        #expect(cached.count == 1)
        #expect(cached[0].id == "impl-42")
        #expect(cached[0].plan != nil)
    }

    // MARK: - Timeline Sync

    @Test("Sync timeline inserts new messages")
    func syncTimelineInserts() throws {
        let (cache, _) = try makeCacheService()

        let messages = [
            TervezoTimelineMessage(
                id: "msg-1", type: "user_prompt",
                timestamp: "2024-03-07T12:00:00Z",
                rawJSON: ["text": .string("Fix the bug")]
            ),
            TervezoTimelineMessage(
                id: "msg-2", type: "assistant_text",
                timestamp: "2024-03-07T12:00:05Z",
                rawJSON: ["text": .string("I'll look into it.")]
            ),
        ]

        try cache.syncTimelineMessages(messages, implementationId: "impl-1")
        let cached = try cache.fetchCachedTimeline(implementationId: "impl-1")
        #expect(cached.count == 2)
        #expect(cached[0].messageId == "msg-1")
        #expect(cached[0].type == "user_prompt")
    }

    @Test("Sync timeline skips existing messages (idempotent)")
    func syncTimelineIdempotent() throws {
        let (cache, _) = try makeCacheService()

        let messages = [
            TervezoTimelineMessage(
                id: "msg-1", type: "user_prompt",
                timestamp: "2024-03-07T12:00:00Z",
                rawJSON: ["text": .string("Fix the bug")]
            ),
        ]

        try cache.syncTimelineMessages(messages, implementationId: "impl-1")
        try cache.syncTimelineMessages(messages, implementationId: "impl-1") // Second sync
        let cached = try cache.fetchCachedTimeline(implementationId: "impl-1")
        #expect(cached.count == 1) // Not duplicated
    }

    // MARK: - Workspaces Sync

    @Test("Sync workspaces creates and updates")
    func syncWorkspaces() throws {
        let (cache, _) = try makeCacheService()

        let workspaces = [
            TervezoWorkspace(id: "ws-1", name: "Team Alpha", slug: "team-alpha", logo: nil),
            TervezoWorkspace(id: "ws-2", name: "Team Beta", slug: "team-beta", logo: "logo.png"),
        ]

        try cache.syncWorkspaces(workspaces)
        let cached = try cache.fetchCachedWorkspaces()
        #expect(cached.count == 2)
    }

    // MARK: - Settings

    @Test("Get or create settings returns defaults on first call")
    func settingsDefaults() throws {
        let (cache, _) = try makeCacheService()

        let settings = try cache.getOrCreateSettings()
        #expect(settings.pollIntervalSeconds == 30)
        #expect(settings.sortField == "updatedAt")
        #expect(settings.onboardingCompleted == false)
    }

    @Test("Get or create settings returns existing on second call")
    func settingsSingleton() throws {
        let (cache, _) = try makeCacheService()

        let first = try cache.getOrCreateSettings()
        first.pollIntervalSeconds = 60

        let second = try cache.getOrCreateSettings()
        #expect(second.pollIntervalSeconds == 60)
    }

    // MARK: - Offline Behavior

    @Test("Cached data survives when fresh sync fails")
    func offlineCacheSurvives() throws {
        let (cache, _) = try makeCacheService()

        // Sync some data
        let list = ImplementationList(
            items: [
                ImplementationSummary(
                    id: "impl-1", title: "Offline test", status: "running", mode: "bugfix",
                    repoUrl: nil, branch: nil, prUrl: nil, prStatus: nil,
                    createdAt: Date(timeIntervalSince1970: 1709836800), updatedAt: nil
                ),
            ],
            total: 1
        )
        try cache.syncImplementationList(list)

        // Simulate "API fails" by not syncing new data
        // Verify cached data is still available
        let cached = try cache.fetchCachedImplementations()
        #expect(cached.count == 1)
        #expect(cached[0].title == "Offline test")
    }

    // MARK: - Cache Pruning

    @Test("Prune removes old timeline messages")
    func pruneOldMessages() throws {
        let (cache, context) = try makeCacheService()

        // Insert an old message manually
        let old = CachedTimelineMessage(
            messageId: "old-msg", implementationId: "impl-1",
            type: "user_prompt", timestamp: "2024-01-01T00:00:00Z",
            text: "Old message",
            cachedAt: Date(timeIntervalSince1970: 0) // Very old
        )
        context.insert(old)
        try context.save()

        // Insert a new message
        let messages = [
            TervezoTimelineMessage(
                id: "new-msg", type: "user_prompt",
                timestamp: "2024-03-07T12:00:00Z",
                rawJSON: ["text": .string("New message")]
            ),
        ]
        try cache.syncTimelineMessages(messages, implementationId: "impl-1")

        // Prune messages cached before a cutoff
        let cutoff = Date(timeIntervalSince1970: 100)
        try cache.pruneOldTimelineMessages(olderThan: cutoff)

        let remaining = try cache.fetchCachedTimeline(implementationId: "impl-1")
        #expect(remaining.count == 1)
        #expect(remaining[0].messageId == "new-msg")
    }
}
