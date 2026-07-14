import SwiftUI

struct BlockMoveSheet: View {
    @ObservedObject var mosaic: MockMosaicService
    let intent: BlockMoveIntent

    @Environment(\.dismiss) private var dismiss
    @State private var query = ""
    @State private var selectedDate: Date
    @State private var activeMoveId: UUID
    @State private var failedRequest: BlockMoveRequest?
    @State private var errorMessage: String?
    @State private var isMoving = false
    @State private var exactRetryRequired = false

    init(mosaic: MockMosaicService, intent: BlockMoveIntent) {
        self.mosaic = mosaic
        self.intent = intent
        let yesterday = Calendar.current.date(byAdding: .day, value: -1, to: Date()) ?? Date()
        _selectedDate = State(initialValue: yesterday)
        _activeMoveId = State(initialValue: intent.moveId)
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Text(intent.preview.isEmpty ? "Untitled block" : intent.preview)
                        .lineLimit(2)
                } header: {
                    Text("Moving block and children")
                }

                if isMoving {
                    Section {
                        ProgressView("Moving block and children…")
                    }
                }

                if let failedRequest, let errorMessage {
                    Section("Move failed") {
                        Text(errorMessage)
                            .foregroundStyle(.secondary)
                        Button("Retry") {
                            perform(failedRequest)
                        }
                        .disabled(isMoving)
                        if !exactRetryRequired {
                            Button("Choose another destination") {
                                self.failedRequest = nil
                                self.errorMessage = nil
                                activeMoveId = UUID()
                            }
                            .disabled(isMoving)
                        }
                    }
                } else {
                    Section("Choose a day") {
                        DatePicker(
                            "Date",
                            selection: $selectedDate,
                            displayedComponents: .date
                        )
                        Button {
                            submit(BlockMoveDestination(
                                slug: Self.daySlug(selectedDate),
                                title: Self.dayTitle(selectedDate),
                                kind: .daily
                            ))
                        } label: {
                            Label("Move to \(Self.dayTitle(selectedDate))", systemImage: "calendar")
                        }
                        .disabled(isMoving || Self.daySlug(selectedDate) == intent.sourceSlug)
                    }

                    let destinations = mosaic.blockMoveDestinations(
                        query: query,
                        excluding: intent.sourceSlug
                    )
                    let days = destinations.filter { $0.kind == .daily }
                    let pages = destinations.filter { $0.kind == .page }

                    if !days.isEmpty {
                        Section("Recent days") {
                            ForEach(days) { destination in
                                destinationRow(destination, icon: "calendar")
                            }
                        }
                    }
                    if !pages.isEmpty {
                        Section("Pages") {
                            ForEach(pages) { destination in
                                destinationRow(destination, icon: "doc.text")
                            }
                        }
                    }
                }
            }
            .navigationTitle("Move to…")
            .navigationBarTitleDisplayMode(.inline)
            .searchable(text: $query, prompt: "Search days and pages")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .disabled(isMoving || exactRetryRequired)
                }
            }
        }
        .interactiveDismissDisabled(isMoving || exactRetryRequired)
    }

    private func destinationRow(_ destination: BlockMoveDestination, icon: String) -> some View {
        Button {
            submit(destination)
        } label: {
            Label {
                VStack(alignment: .leading, spacing: 2) {
                    Text(destination.title)
                    if destination.title != destination.slug {
                        Text(destination.slug)
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                    }
                }
            } icon: {
                Image(systemName: icon)
            }
        }
        .disabled(isMoving)
    }

    private func submit(_ destination: BlockMoveDestination) {
        let request = BlockMoveIntent(
            moveId: activeMoveId,
            sourceSlug: intent.sourceSlug,
            rootBid: intent.rootBid,
            preview: intent.preview
        ).request(to: destination)
        perform(request)
    }

    private func perform(_ request: BlockMoveRequest) {
        guard !isMoving else { return }
        guard mosaic.backendMutationAdmissionIsOpen else {
            failedRequest = request
            errorMessage = "Finish switching mosaics, then retry the move."
            return
        }
        isMoving = true
        errorMessage = nil
        mosaic.enqueueBackendMutation { reservation in
            do {
                try await mosaic.moveSubtree(
                    request,
                    reservation: reservation
                )
                dismiss()
            } catch {
                failedRequest = request
                errorMessage = Self.message(for: error)
                exactRetryRequired = exactRetryRequired || Self.requiresExactRetry(error)
                isMoving = false
            }
        }
    }

    static func requiresExactRetry(_ error: Error) -> Bool {
        guard let error = error as? FfiSyncError else { return false }
        if case .RelocationRecoveryRequired = error { return true }
        return false
    }

    private static func message(for error: Error) -> String {
        guard let error = error as? FfiSyncError else {
            return error.localizedDescription
        }
        switch error {
        case .RelocationRejected(let message), .RelocationConflict(let message):
            return message
        case .RelocationRecoveryRequired(_, let message), .Other(let message):
            return message
        case .InvalidPairingCode(let message):
            return message
        }
    }

    private static func daySlug(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = .current
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter.string(from: date)
    }

    private static func dayTitle(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.locale = .current
        formatter.dateStyle = .medium
        return formatter.string(from: date)
    }
}
