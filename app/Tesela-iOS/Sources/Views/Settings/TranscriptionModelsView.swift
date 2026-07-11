import SwiftUI

/// Settings → Voice → Models. Lists the curated TranscriptionCatalog
/// with per-row download / progress / delete / set-active actions.
struct TranscriptionModelsView: View {
    @ObservedObject var store: TranscriptionStore

    @Environment(\.theme) private var theme
    @State private var confirmDeleteId: String? = nil

    var body: some View {
        Form {
            activeRow
            ForEach(TranscriptionCatalog.grouped, id: \.family) { group in
                Section {
                    ForEach(group.models) { model in
                        modelRow(model)
                            .listRowBackground(theme.bg2)
                    }
                } header: {
                    Text(group.family.displayName)
                        .font(.system(size: 10, design: .monospaced))
                        .tracking(1.2)
                        .foregroundStyle(theme.fgFaint)
                } footer: {
                    if group.family == .parakeet {
                        Text("Parakeet runs on-device via the FluidAudio package. Downloading fetches a CoreML model set (a few hundred MB).")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(theme.fgFaint)
                    }
                }
            }
            storageFooter
        }
        .scrollContentBackground(.hidden)
        .background(theme.bg)
        .navigationTitle("Models")
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog(
            "Delete model?",
            isPresented: deleteDialogBinding(),
            titleVisibility: .visible,
            presenting: confirmDeleteId
        ) { id in
            Button("Delete", role: .destructive) {
                store.deleteModel(id)
                confirmDeleteId = nil
            }
            Button("Cancel", role: .cancel) { confirmDeleteId = nil }
        } message: { id in
            if let m = TranscriptionCatalog.find(id) {
                Text("Frees \(m.sizeBytes.humanReadableModelSize) on this device. You can re-download anytime.")
            }
        }
    }

    // ── Active row ──────────────────────────────────────────────────────

    @ViewBuilder
    private var activeRow: some View {
        Section {
            if let active = TranscriptionCatalog.find(store.activeModelId) {
                LabeledContent {
                    Text(active.displayName)
                        .font(.system(size: 13, design: .monospaced))
                        .foregroundStyle(theme.accentPrimary)
                } label: {
                    Label("Active model", systemImage: "checkmark.circle.fill")
                        .foregroundStyle(theme.fgDefault)
                }
            } else {
                Label {
                    Text("No active model")
                        .foregroundStyle(theme.fgMuted)
                } icon: {
                    Image(systemName: "circle.dashed")
                        .foregroundStyle(theme.fgFaint)
                }
            }
        } footer: {
            Text("Voice capture will use the active model on-device. Pick one from the list below.")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
    }

    // ── Model row ───────────────────────────────────────────────────────

    @ViewBuilder
    private func modelRow(_ model: TranscriptionModel) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                Text(model.displayName)
                    .font(.system(size: 14.5, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                if store.activeModelId == model.id {
                    Text("ACTIVE")
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 1)
                        .foregroundStyle(theme.accentPrimary)
                        .background(theme.accentPrimary.opacity(0.18))
                        .clipShape(Capsule())
                }
                Spacer()
                Text(model.sizeBytes.humanReadableModelSize)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
            Text(model.shortDescription)
                .font(.system(size: 12))
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            if !model.suggestedFor.isEmpty {
                HStack(spacing: 4) {
                    ForEach(model.suggestedFor, id: \.self) { tag in
                        Text(tag)
                            .font(.system(size: 9.5, weight: .medium, design: .monospaced))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 1)
                            .foregroundStyle(theme.fgMuted)
                            .background(theme.bg3)
                            .clipShape(Capsule())
                    }
                }
            }
            actionRow(for: model)
            if case .failed(let message) = store.state(for: model.id) {
                Text(message)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(theme.typeTask)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private func actionRow(for model: TranscriptionModel) -> some View {
        let state = store.state(for: model.id)
        HStack(spacing: 8) {
            switch state {
            case .available, .failed:
                Button {
                    store.startDownload(model)
                } label: {
                    Label("Download", systemImage: "arrow.down.circle")
                        .font(.system(size: 12, weight: .semibold))
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .tint(theme.accentPrimary)
            case .downloading(let progress, let written, let total):
                if total > 0 {
                    ProgressView(value: progress)
                        .frame(maxWidth: .infinity)
                    Text(progressLabel(written: written, total: total))
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                } else {
                    ProgressView()
                    Text("Downloading model…")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
                Button(role: .destructive) {
                    store.cancelDownload(model.id)
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 18))
                        .foregroundStyle(theme.fgMuted)
                }
                .buttonStyle(.plain)
            case .downloaded:
                if store.activeModelId == model.id {
                    Button(role: .destructive) {
                        confirmDeleteId = model.id
                    } label: {
                        Label("Delete", systemImage: "trash")
                            .font(.system(size: 12, weight: .semibold))
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                } else if !model.inferenceSupported {
                    // Downloaded but iOS can't run it yet (Parakeet).
                    Text("Inference not yet supported on iOS")
                        .font(.system(size: 10.5, design: .monospaced))
                        .foregroundStyle(theme.typeNote)
                    Spacer()
                    Button(role: .destructive) {
                        confirmDeleteId = model.id
                    } label: {
                        Image(systemName: "trash")
                            .font(.system(size: 16))
                            .foregroundStyle(theme.fgMuted)
                    }
                    .buttonStyle(.plain)
                } else {
                    Button {
                        store.activate(model.id)
                    } label: {
                        Label("Set active", systemImage: "checkmark.circle")
                            .font(.system(size: 12, weight: .semibold))
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .tint(theme.accentPrimary)
                    Button(role: .destructive) {
                        confirmDeleteId = model.id
                    } label: {
                        Image(systemName: "trash")
                            .font(.system(size: 16))
                            .foregroundStyle(theme.fgMuted)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .padding(.top, 4)
    }

    private func progressLabel(written: Int64, total: Int64) -> String {
        if total > 0 {
            return "\(written.humanReadableModelSize) / \(total.humanReadableModelSize)"
        }
        return written.humanReadableModelSize
    }

    // ── Storage footer ──────────────────────────────────────────────────

    private var storageFooter: some View {
        Section {
            LabeledContent("Storage used") {
                Text(totalDownloadedSize.humanReadableModelSize)
                    .font(.system(size: 13, design: .monospaced))
                    .foregroundStyle(theme.fgMuted)
            }
        } footer: {
            Text("Models are stored on this device under Application Support. Sync mirrors them automatically across paired devices when the desktop client is configured.")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
    }

    private var totalDownloadedSize: Int64 {
        store.states.values.reduce(into: 0) { acc, state in
            if case .downloaded(let size) = state { acc += size }
        }
    }

    private func deleteDialogBinding() -> Binding<Bool> {
        Binding(
            get: { confirmDeleteId != nil },
            set: { if !$0 { confirmDeleteId = nil } }
        )
    }
}
