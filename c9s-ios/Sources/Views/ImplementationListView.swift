import SwiftUI

/// Main implementation list view with search, filtering, and sections.
/// Uses NavigationStack on iPhone and NavigationSplitView on iPad.
struct ImplementationListView: View {
    @State private var viewModel: ImplementationListVM
    @State private var showCreateSheet = false
    @State private var navigationPath = NavigationPath()
    var onSignOut: () -> Void

    init(service: TervezoServiceProtocol = TervezoService(), onSignOut: @escaping () -> Void) {
        _viewModel = State(initialValue: ImplementationListVM(service: service))
        self.onSignOut = onSignOut
    }

    var body: some View {
        NavigationStack(path: $navigationPath) {
            Group {
                if viewModel.isLoading && viewModel.implementations.isEmpty {
                    loadingView
                } else if viewModel.implementations.isEmpty {
                    emptyStateView
                } else {
                    listContent
                }
            }
            .navigationTitle("Implementations")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    filterMenu
                }
                ToolbarItem(placement: .topBarTrailing) {
                    HStack(spacing: 12) {
                        Button {
                            showCreateSheet = true
                        } label: {
                            Image(systemName: "plus")
                        }

                        NavigationLink {
                            SettingsView(onSignOut: onSignOut)
                        } label: {
                            Image(systemName: "gearshape")
                        }
                    }
                }
            }
            .searchable(text: $viewModel.searchText, prompt: "Search implementations...")
            .refreshable {
                await viewModel.refresh()
            }
            .task {
                await viewModel.loadImplementations()
                viewModel.startPolling()
            }
            .onDisappear {
                viewModel.stopPolling()
            }
            .sheet(isPresented: $showCreateSheet) {
                CreateImplementationView { createdId in
                    navigationPath.append(createdId)
                    Task { await viewModel.refresh() }
                }
            }
        }
    }

    // MARK: - Subviews

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
            Text("Loading implementations...")
                .foregroundStyle(.secondary)
        }
    }

    private var emptyStateView: some View {
        ContentUnavailableView {
            Label("No Implementations", systemImage: "terminal")
        } description: {
            if let error = viewModel.errorMessage {
                Text(error)
            } else {
                Text("Create an implementation to get started.")
            }
        } actions: {
            Button("Create Implementation") {
                showCreateSheet = true
            }
            .buttonStyle(.borderedProminent)

            if viewModel.errorMessage != nil {
                Button("Retry") {
                    Task { await viewModel.refresh() }
                }
            }
        }
    }

    private var listContent: some View {
        List {
            if let error = viewModel.errorMessage {
                Section {
                    Label(error, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                        .font(.caption)
                }
            }

            ForEach(viewModel.sections) { section in
                Section(section.title) {
                    ForEach(section.items) { impl in
                        NavigationLink(value: impl.id) {
                            ImplementationRowView(implementation: impl)
                        }
                        .swipeActions(edge: .trailing) {
                            if impl.prUrl == nil && impl.branch != nil {
                                Button("Create PR") {
                                    Task { try? await TervezoService().createPR(id: impl.id) }
                                }
                                .tint(.blue)
                            }
                        }
                        .swipeActions(edge: .leading) {
                            if ["failed", "stopped", "cancelled"].contains(impl.status) {
                                Button("Restart") {
                                    Task { try? await TervezoService().restart(id: impl.id) }
                                }
                                .tint(.orange)
                            }
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationDestination(for: String.self) { implId in
            ImplementationDetailView(implementationId: implId)
        }
    }

    private var filterMenu: some View {
        Menu {
            // Status filter
            Section("Filter by Status") {
                ForEach(ImplementationListVM.StatusFilter.allCases) { filter in
                    Button {
                        viewModel.statusFilter = filter
                    } label: {
                        if viewModel.statusFilter == filter {
                            Label(filter.rawValue, systemImage: "checkmark")
                        } else {
                            Text(filter.rawValue)
                        }
                    }
                }
            }

            // Sort
            Section("Sort by") {
                ForEach(ImplementationListVM.SortOption.allCases) { option in
                    Button {
                        viewModel.sortOption = option
                    } label: {
                        if viewModel.sortOption == option {
                            Label(option.rawValue, systemImage: "checkmark")
                        } else {
                            Text(option.rawValue)
                        }
                    }
                }
            }
        } label: {
            Image(systemName: "line.3.horizontal.decrease.circle")
        }
    }
}
