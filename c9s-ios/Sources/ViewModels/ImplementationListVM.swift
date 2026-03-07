import Foundation
import Observation
import SwiftData

/// ViewModel for the implementation list screen.
/// Manages fetching, filtering, sorting, polling, and search.
@Observable
@MainActor
final class ImplementationListVM {

    // MARK: - Published State

    var implementations: [ImplementationSummary] = []
    var filteredImplementations: [ImplementationSummary] = []
    var isLoading = false
    var isRefreshing = false
    var errorMessage: String?
    var searchText = "" { didSet { applyFilters() } }
    var statusFilter: StatusFilter = .all { didSet { applyFilters() } }
    var sortOption: SortOption = .updatedDesc { didSet { applyFilters() } }

    // MARK: - Types

    enum StatusFilter: String, CaseIterable, Identifiable {
        case all = "All"
        case running = "Running"
        case waitingForInput = "Waiting"
        case completed = "Completed"
        case failed = "Failed"
        case pending = "Pending"
        case stopped = "Stopped"

        var id: String { rawValue }

        var apiValue: String? {
            switch self {
            case .all: nil
            case .running: "running"
            case .waitingForInput: nil // Filtered client-side
            case .completed: "completed"
            case .failed: "failed"
            case .pending: "pending"
            case .stopped: "stopped"
            }
        }
    }

    enum SortOption: String, CaseIterable, Identifiable {
        case updatedDesc = "Recent First"
        case updatedAsc = "Oldest First"
        case statusAsc = "Status"
        case titleAsc = "Title A-Z"

        var id: String { rawValue }
    }

    // MARK: - Dependencies

    private let service: TervezoServiceProtocol
    private var pollingTask: Task<Void, Never>?
    private var pollInterval: TimeInterval = 30

    init(service: TervezoServiceProtocol = TervezoService()) {
        self.service = service
    }

    deinit {
        pollingTask?.cancel()
    }

    // MARK: - Fetching

    /// Initial load of implementations.
    func loadImplementations() async {
        isLoading = true
        errorMessage = nil

        do {
            let list = try await service.listImplementations(status: nil)
            implementations = list.items
            applyFilters()
        } catch {
            errorMessage = error.localizedDescription
        }

        isLoading = false
    }

    /// Pull-to-refresh handler.
    func refresh() async {
        isRefreshing = true
        errorMessage = nil

        do {
            let list = try await service.listImplementations(status: nil)
            implementations = list.items
            applyFilters()
        } catch {
            errorMessage = error.localizedDescription
        }

        isRefreshing = false
    }

    // MARK: - Polling

    /// Start background polling at the configured interval.
    func startPolling(interval: TimeInterval = 30) {
        pollInterval = interval
        pollingTask?.cancel()
        pollingTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(interval))
                guard !Task.isCancelled else { break }
                await self?.silentRefresh()
            }
        }
    }

    /// Stop background polling.
    func stopPolling() {
        pollingTask?.cancel()
        pollingTask = nil
    }

    /// Refresh without setting loading/refreshing states (for polling).
    private func silentRefresh() async {
        do {
            let list = try await service.listImplementations(status: nil)
            implementations = list.items
            applyFilters()
        } catch {
            // Silently ignore polling errors
        }
    }

    // MARK: - Filtering & Sorting

    private func applyFilters() {
        var result = implementations

        // Status filter
        switch statusFilter {
        case .all:
            break
        case .waitingForInput:
            // No server-side filter available; would need step info.
            // For now, show running implementations (closest match).
            result = result.filter { $0.status == "running" }
        default:
            if let apiValue = statusFilter.apiValue {
                result = result.filter { $0.status == apiValue }
            }
        }

        // Search filter
        if !searchText.isEmpty {
            let query = searchText.lowercased()
            result = result.filter { impl in
                (impl.title?.lowercased().contains(query) ?? false) ||
                (impl.branch?.lowercased().contains(query) ?? false) ||
                impl.id.lowercased().contains(query)
            }
        }

        // Sort
        switch sortOption {
        case .updatedDesc:
            result.sort { ($0.updatedAt ?? $0.createdAt) > ($1.updatedAt ?? $1.createdAt) }
        case .updatedAsc:
            result.sort { ($0.updatedAt ?? $0.createdAt) < ($1.updatedAt ?? $1.createdAt) }
        case .statusAsc:
            result.sort { statusOrder($0.status) < statusOrder($1.status) }
        case .titleAsc:
            result.sort { ($0.title ?? "") < ($1.title ?? "") }
        }

        filteredImplementations = result
    }

    /// Priority ordering for status sort: running first, then waiting, pending, failed, completed, etc.
    private func statusOrder(_ status: String) -> Int {
        switch status {
        case "running": 0
        case "pending", "queued": 1
        case "failed": 2
        case "stopped": 3
        case "completed": 4
        case "merged": 5
        case "cancelled": 6
        default: 7
        }
    }

    // MARK: - Grouped Sections

    /// Implementations grouped into sections for the list view.
    var sections: [ImplementationSection] {
        let active = filteredImplementations.filter { $0.status == "running" || $0.status == "pending" || $0.status == "queued" }
        let failed = filteredImplementations.filter { $0.status == "failed" || $0.status == "stopped" }
        let completed = filteredImplementations.filter { $0.status == "completed" || $0.status == "merged" || $0.status == "cancelled" }

        var sections: [ImplementationSection] = []
        if !active.isEmpty { sections.append(ImplementationSection(title: "Active", items: active)) }
        if !failed.isEmpty { sections.append(ImplementationSection(title: "Failed", items: failed)) }
        if !completed.isEmpty { sections.append(ImplementationSection(title: "Completed", items: completed)) }
        return sections
    }
}

struct ImplementationSection: Identifiable {
    let title: String
    let items: [ImplementationSummary]
    var id: String { title }
}
