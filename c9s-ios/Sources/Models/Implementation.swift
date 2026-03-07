import Foundation
import SwiftData

/// Cached implementation for offline display.
/// Mirrors the API response shape from both list and detail endpoints.
/// Updated via TervezoService sync logic when API responses arrive.
@Model
final class CachedImplementation {
    /// Unique implementation ID from the API (e.g., "impl-abc123").
    @Attribute(.unique)
    var id: String

    var title: String?
    var status: String
    var mode: String
    var prompt: String?
    var plan: String?
    var analysis: String?
    var error: String?
    var isRunning: Bool
    var repoUrl: String?
    var branch: String?
    var baseBranch: String?
    var branchPushed: Bool?
    var prUrl: String?
    var prStatus: String?
    var sandboxId: String?
    var iterations: Int
    var currentIteration: Int
    var createdAt: Date
    var updatedAt: Date?
    var timelineMessageCount: Int

    /// When this cache entry was last synced from the API.
    var lastSyncedAt: Date

    init(
        id: String,
        title: String? = nil,
        status: String = "pending",
        mode: String = "standard",
        prompt: String? = nil,
        plan: String? = nil,
        analysis: String? = nil,
        error: String? = nil,
        isRunning: Bool = false,
        repoUrl: String? = nil,
        branch: String? = nil,
        baseBranch: String? = nil,
        branchPushed: Bool? = nil,
        prUrl: String? = nil,
        prStatus: String? = nil,
        sandboxId: String? = nil,
        iterations: Int = 0,
        currentIteration: Int = 0,
        createdAt: Date = .now,
        updatedAt: Date? = nil,
        timelineMessageCount: Int = 0,
        lastSyncedAt: Date = .now
    ) {
        self.id = id
        self.title = title
        self.status = status
        self.mode = mode
        self.prompt = prompt
        self.plan = plan
        self.analysis = analysis
        self.error = error
        self.isRunning = isRunning
        self.repoUrl = repoUrl
        self.branch = branch
        self.baseBranch = baseBranch
        self.branchPushed = branchPushed
        self.prUrl = prUrl
        self.prStatus = prStatus
        self.sandboxId = sandboxId
        self.iterations = iterations
        self.currentIteration = currentIteration
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.timelineMessageCount = timelineMessageCount
        self.lastSyncedAt = lastSyncedAt
    }

    /// Display name: uses title or falls back to "(untitled)".
    var displayName: String {
        title ?? "(untitled)"
    }

    /// Whether the implementation is in a terminal state.
    var isTerminal: Bool {
        ["completed", "merged", "failed", "stopped", "cancelled"].contains(status)
    }

    /// Short repository name (last path component of URL).
    var repoShort: String {
        guard let url = repoUrl else { return "-" }
        return url.split(separator: "/").last.map(String.init) ?? url
    }

    /// Relative time since last activity.
    var lastActivityDisplay: String {
        let date = updatedAt ?? createdAt
        let seconds = Int(Date.now.timeIntervalSince(date))
        if seconds < 60 { return "\(seconds)s ago" }
        if seconds < 3600 { return "\(seconds / 60)m ago" }
        if seconds < 86400 { return "\(seconds / 3600)h ago" }
        return "\(seconds / 86400)d ago"
    }

    /// Update this cached entry from an API summary (list endpoint).
    func updateFromSummary(_ summary: ImplementationSummary) {
        title = summary.title
        status = summary.status
        mode = summary.mode
        repoUrl = summary.repoUrl
        branch = summary.branch
        prUrl = summary.prUrl
        prStatus = summary.prStatus
        updatedAt = summary.updatedAt
        lastSyncedAt = .now
    }

    /// Update this cached entry from an API detail (getById endpoint).
    func updateFromDetail(_ detail: ImplementationDetail) {
        title = detail.title
        status = detail.status
        mode = detail.mode
        prompt = detail.prompt
        plan = detail.plan
        analysis = detail.analysis
        error = detail.error
        isRunning = detail.isRunning
        repoUrl = detail.repoUrl
        branch = detail.branch
        baseBranch = detail.baseBranch
        branchPushed = detail.branchPushed
        prUrl = detail.prUrl
        prStatus = detail.prStatus
        sandboxId = detail.sandboxId
        iterations = detail.iterations
        currentIteration = detail.currentIteration
        updatedAt = detail.updatedAt
        timelineMessageCount = detail.timelineMessageCount
        lastSyncedAt = .now
    }

    /// Create a cached entry from an API summary.
    static func fromSummary(_ summary: ImplementationSummary) -> CachedImplementation {
        CachedImplementation(
            id: summary.id,
            title: summary.title,
            status: summary.status,
            mode: summary.mode,
            repoUrl: summary.repoUrl,
            branch: summary.branch,
            prUrl: summary.prUrl,
            prStatus: summary.prStatus,
            createdAt: summary.createdAt,
            updatedAt: summary.updatedAt
        )
    }

    /// Create a cached entry from an API detail.
    static func fromDetail(_ detail: ImplementationDetail) -> CachedImplementation {
        CachedImplementation(
            id: detail.id,
            title: detail.title,
            status: detail.status,
            mode: detail.mode,
            prompt: detail.prompt,
            plan: detail.plan,
            analysis: detail.analysis,
            error: detail.error,
            isRunning: detail.isRunning,
            repoUrl: detail.repoUrl,
            branch: detail.branch,
            baseBranch: detail.baseBranch,
            branchPushed: detail.branchPushed,
            prUrl: detail.prUrl,
            prStatus: detail.prStatus,
            sandboxId: detail.sandboxId,
            iterations: detail.iterations,
            currentIteration: detail.currentIteration,
            createdAt: detail.createdAt,
            updatedAt: detail.updatedAt,
            timelineMessageCount: detail.timelineMessageCount
        )
    }
}
