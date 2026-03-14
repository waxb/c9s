import Foundation
import SwiftData

/// Synchronizes API responses into the local SwiftData cache.
/// Call these methods after successful API fetches to keep the
/// offline cache current. Operations are idempotent (upsert).
@MainActor
final class CacheService {
    private let modelContext: ModelContext

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    // MARK: - Implementations

    /// Upsert implementations from the list API response.
    /// Creates new entries or updates existing ones by ID.
    func syncImplementationList(_ list: ImplementationList) throws {
        for summary in list.items {
            let summaryId = summary.id
            let descriptor = FetchDescriptor<CachedImplementation>(
                predicate: #Predicate { $0.id == summaryId }
            )
            let existing = try modelContext.fetch(descriptor)

            if let cached = existing.first {
                cached.updateFromSummary(summary)
            } else {
                modelContext.insert(CachedImplementation.fromSummary(summary))
            }
        }
        try modelContext.save()
    }

    /// Upsert a single implementation from the detail API response.
    func syncImplementationDetail(_ detail: ImplementationDetail) throws {
        let detailId = detail.id
        let descriptor = FetchDescriptor<CachedImplementation>(
            predicate: #Predicate { $0.id == detailId }
        )
        let existing = try modelContext.fetch(descriptor)

        if let cached = existing.first {
            cached.updateFromDetail(detail)
        } else {
            modelContext.insert(CachedImplementation.fromDetail(detail))
        }
        try modelContext.save()
    }

    // MARK: - Timeline Messages

    /// Upsert timeline messages for a given implementation.
    /// New messages are inserted; existing messages (by ID) are skipped.
    func syncTimelineMessages(_ messages: [TervezoTimelineMessage], implementationId: String) throws {
        for message in messages {
            let msgId = message.id
            let descriptor = FetchDescriptor<CachedTimelineMessage>(
                predicate: #Predicate { $0.messageId == msgId }
            )
            let exists = try modelContext.fetchCount(descriptor)
            if exists == 0 {
                modelContext.insert(CachedTimelineMessage.from(message, implementationId: implementationId))
            }
        }
        try modelContext.save()
    }

    // MARK: - Workspaces

    /// Upsert workspaces from the list API response.
    func syncWorkspaces(_ workspaces: [TervezoWorkspace]) throws {
        for workspace in workspaces {
            let workspaceId = workspace.id
            let descriptor = FetchDescriptor<CachedWorkspace>(
                predicate: #Predicate { $0.id == workspaceId }
            )
            let existing = try modelContext.fetch(descriptor)

            if let cached = existing.first {
                cached.updateFrom(workspace)
            } else {
                modelContext.insert(CachedWorkspace.from(workspace))
            }
        }
        try modelContext.save()
    }

    // MARK: - Settings

    /// Get or create the singleton AppSettings instance.
    func getOrCreateSettings() throws -> AppSettings {
        let descriptor = FetchDescriptor<AppSettings>()
        let existing = try modelContext.fetch(descriptor)
        if let settings = existing.first {
            return settings
        }
        let settings = AppSettings.defaults
        modelContext.insert(settings)
        try modelContext.save()
        return settings
    }

    // MARK: - Cache Queries

    /// Fetch all cached implementations, sorted by updatedAt descending.
    func fetchCachedImplementations() throws -> [CachedImplementation] {
        var descriptor = FetchDescriptor<CachedImplementation>(
            sortBy: [SortDescriptor(\.updatedAt, order: .reverse)]
        )
        descriptor.fetchLimit = 200
        return try modelContext.fetch(descriptor)
    }

    /// Fetch cached timeline messages for an implementation, sorted by timestamp.
    func fetchCachedTimeline(implementationId: String) throws -> [CachedTimelineMessage] {
        let descriptor = FetchDescriptor<CachedTimelineMessage>(
            predicate: #Predicate { $0.implementationId == implementationId },
            sortBy: [SortDescriptor(\.timestamp)]
        )
        return try modelContext.fetch(descriptor)
    }

    /// Fetch all cached workspaces.
    func fetchCachedWorkspaces() throws -> [CachedWorkspace] {
        let descriptor = FetchDescriptor<CachedWorkspace>(
            sortBy: [SortDescriptor(\.name)]
        )
        return try modelContext.fetch(descriptor)
    }

    /// Clear stale timeline messages older than the given date.
    func pruneOldTimelineMessages(olderThan date: Date) throws {
        let descriptor = FetchDescriptor<CachedTimelineMessage>(
            predicate: #Predicate { $0.cachedAt < date }
        )
        let old = try modelContext.fetch(descriptor)
        for message in old {
            modelContext.delete(message)
        }
        try modelContext.save()
    }
}
