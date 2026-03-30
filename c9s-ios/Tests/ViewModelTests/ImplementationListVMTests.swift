import Testing
import Foundation
@testable import C9sLib

@Suite("ImplementationListVM Tests")
@MainActor
struct ImplementationListVMTests {

    private func makeVM(items: [ImplementationSummary] = []) -> (ImplementationListVM, MockTervezoService) {
        let mock = MockTervezoService()
        mock.listImplementationsResult = .success(ImplementationList(items: items, total: items.count))
        let vm = ImplementationListVM(service: mock)
        return (vm, mock)
    }

    private func sampleItems() -> [ImplementationSummary] {
        [
            ImplementationSummary(
                id: "impl-1", title: "Fix login bug", status: "running", mode: "bugfix",
                repoUrl: "https://github.com/user/repo", branch: "fix/login",
                prUrl: nil, prStatus: nil,
                createdAt: Date(timeIntervalSince1970: 1709836800),
                updatedAt: Date(timeIntervalSince1970: 1709840400)
            ),
            ImplementationSummary(
                id: "impl-2", title: "Add dark mode", status: "completed", mode: "feature",
                repoUrl: "https://github.com/user/repo", branch: "feat/dark-mode",
                prUrl: "https://github.com/user/repo/pull/5", prStatus: "open",
                createdAt: Date(timeIntervalSince1970: 1709750400),
                updatedAt: Date(timeIntervalSince1970: 1709754000)
            ),
            ImplementationSummary(
                id: "impl-3", title: "Refactor auth", status: "failed", mode: "feature",
                repoUrl: nil, branch: "refactor/auth",
                prUrl: nil, prStatus: nil,
                createdAt: Date(timeIntervalSince1970: 1709664000),
                updatedAt: Date(timeIntervalSince1970: 1709667600)
            ),
            ImplementationSummary(
                id: "impl-4", title: nil, status: "pending", mode: "standard",
                repoUrl: nil, branch: nil,
                prUrl: nil, prStatus: nil,
                createdAt: Date(timeIntervalSince1970: 1709577600),
                updatedAt: nil
            ),
        ]
    }

    // MARK: - Loading

    @Test("Load implementations populates list")
    func loadSuccess() async {
        let (vm, mock) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        #expect(vm.implementations.count == 4)
        #expect(vm.filteredImplementations.count == 4)
        #expect(vm.isLoading == false)
        #expect(vm.errorMessage == nil)
        #expect(mock.listImplementationsCallCount == 1)
    }

    @Test("Load failure sets error message")
    func loadFailure() async {
        let mock = MockTervezoService()
        mock.listImplementationsResult = .failure(TervezoServiceError.networkError("Connection refused"))
        let vm = ImplementationListVM(service: mock)

        await vm.loadImplementations()

        #expect(vm.implementations.isEmpty)
        #expect(vm.errorMessage != nil)
        #expect(vm.isLoading == false)
    }

    // MARK: - Refresh

    @Test("Refresh updates implementations")
    func refreshSuccess() async {
        let (vm, mock) = makeVM(items: [sampleItems()[0]])
        await vm.loadImplementations()
        #expect(vm.implementations.count == 1)

        // Update mock to return more items
        mock.listImplementationsResult = .success(ImplementationList(items: sampleItems(), total: 4))
        await vm.refresh()

        #expect(vm.implementations.count == 4)
        #expect(vm.isRefreshing == false)
        #expect(mock.listImplementationsCallCount == 2)
    }

    // MARK: - Status Filter

    @Test("Filter by running shows only running items")
    func filterRunning() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.statusFilter = .running
        #expect(vm.filteredImplementations.count == 1)
        #expect(vm.filteredImplementations[0].status == "running")
    }

    @Test("Filter by completed shows only completed items")
    func filterCompleted() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.statusFilter = .completed
        #expect(vm.filteredImplementations.count == 1)
        #expect(vm.filteredImplementations[0].status == "completed")
    }

    @Test("Filter by failed shows only failed items")
    func filterFailed() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.statusFilter = .failed
        #expect(vm.filteredImplementations.count == 1)
        #expect(vm.filteredImplementations[0].status == "failed")
    }

    @Test("Filter All shows all items")
    func filterAll() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.statusFilter = .running
        #expect(vm.filteredImplementations.count == 1)

        vm.statusFilter = .all
        #expect(vm.filteredImplementations.count == 4)
    }

    // MARK: - Search

    @Test("Search by title filters results")
    func searchByTitle() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.searchText = "login"
        #expect(vm.filteredImplementations.count == 1)
        #expect(vm.filteredImplementations[0].id == "impl-1")
    }

    @Test("Search by branch filters results")
    func searchByBranch() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.searchText = "dark-mode"
        #expect(vm.filteredImplementations.count == 1)
        #expect(vm.filteredImplementations[0].id == "impl-2")
    }

    @Test("Search is case-insensitive")
    func searchCaseInsensitive() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.searchText = "REFACTOR"
        #expect(vm.filteredImplementations.count == 1)
        #expect(vm.filteredImplementations[0].id == "impl-3")
    }

    @Test("Empty search shows all items")
    func searchEmpty() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.searchText = "login"
        #expect(vm.filteredImplementations.count == 1)

        vm.searchText = ""
        #expect(vm.filteredImplementations.count == 4)
    }

    // MARK: - Sorting

    @Test("Sort by recent shows newest first")
    func sortRecent() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.sortOption = .updatedDesc
        #expect(vm.filteredImplementations.first?.id == "impl-1") // Most recently updated
    }

    @Test("Sort by oldest shows oldest first")
    func sortOldest() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.sortOption = .updatedAsc
        #expect(vm.filteredImplementations.first?.id == "impl-4") // Oldest (no updatedAt, uses createdAt)
    }

    @Test("Sort by status shows running first")
    func sortStatus() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.sortOption = .statusAsc
        #expect(vm.filteredImplementations.first?.status == "running")
    }

    @Test("Sort by title is alphabetical")
    func sortTitle() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.sortOption = .titleAsc
        // nil title sorts as "" (first), then "Add dark mode", "Fix login bug", "Refactor auth"
        #expect(vm.filteredImplementations.last?.title == "Refactor auth")
    }

    // MARK: - Sections

    @Test("Sections group by status category")
    func sectionsGrouping() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        let sections = vm.sections
        #expect(sections.count == 3) // Active, Failed, Completed

        let activeSection = sections.first { $0.title == "Active" }
        #expect(activeSection?.items.count == 2) // running + pending

        let failedSection = sections.first { $0.title == "Failed" }
        #expect(failedSection?.items.count == 1)

        let completedSection = sections.first { $0.title == "Completed" }
        #expect(completedSection?.items.count == 1)
    }

    // MARK: - Combined Filters

    @Test("Search + status filter both apply")
    func combinedFilters() async {
        let (vm, _) = makeVM(items: sampleItems())
        await vm.loadImplementations()

        vm.statusFilter = .all
        vm.searchText = "auth"
        #expect(vm.filteredImplementations.count == 1)

        vm.statusFilter = .running
        // "Refactor auth" is failed, not running — no match
        #expect(vm.filteredImplementations.count == 0)
    }
}
