import SwiftUI

/// Saved-view editor sheet (saved-views spec, 2026-06-10). DSL-first:
/// the query text field is the primary editor (Todoist-filter feel) and
/// the chip registry from `InboxChips.swift` becomes one-tap fragment
/// INSERTERS that toggle their clause in/out of the string — chips never
/// own the query, the text does. Validation mirrors the server's
/// `validate_dsl` rule via `SavedViewLogic.dslValidationError` (non-empty
/// input with zero recognized predicates is rejected inline, so a typo
/// can't silently become a match-everything view).
///
/// Display mode is a stored preference: iOS always renders results as a
/// list, so picking table/kanban shows an honest "applies on web" note.
/// Builtins (the seeded Inbox) are editable but never deletable — the
/// delete affordance is replaced by a caption saying so.
struct GrViewEditorSheet: View {
    /// nil = creating a new view.
    let existing: SavedView?
    /// The current ordered registry — used to mint the new view's order
    /// (append after the last, steps of 10, the server's rule).
    let siblings: [SavedView]
    /// Persist the record. `(record, isNew)`; throws surface inline.
    let onSave: (SavedView, Bool) async throws -> Void
    /// Delete the view (never called for builtins/new views).
    let onDelete: (String) async throws -> Void
    /// The live property/type registry — drives the query-completion
    /// strip's KEY (property names) and VALUE (select choices / type
    /// names) tiers (tesela-vp9.5, spec decision 4). Defaults to an
    /// empty registry so existing call sites/previews keep compiling;
    /// `GrInboxView` wires `mosaic.propertyRegistry`.
    var propertyRegistry: PropertyRegistry = PropertyRegistry()

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    @State private var name: String
    @State private var dsl: String
    @State private var displayMode: String
    @State private var errorText: String? = nil
    @State private var busy = false
    @State private var confirmDelete = false

    init(
        existing: SavedView?,
        siblings: [SavedView],
        onSave: @escaping (SavedView, Bool) async throws -> Void,
        onDelete: @escaping (String) async throws -> Void,
        propertyRegistry: PropertyRegistry = PropertyRegistry()
    ) {
        self.existing = existing
        self.siblings = siblings
        self.onSave = onSave
        self.onDelete = onDelete
        self.propertyRegistry = propertyRegistry
        self._name = State(initialValue: existing?.name ?? "")
        self._dsl = State(initialValue: existing?.dsl ?? "")
        self._displayMode = State(initialValue: existing?.displayMode ?? "list")
    }

    private var isNew: Bool { existing == nil }
    private var isBuiltin: Bool { existing?.builtin == true }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                GrHeader(
                    title: isNew ? "New View" : "Edit View",
                    subtitle: isBuiltin ? "BUILT-IN" : "SAVED VIEW"
                ) {
                    GrButton(variant: .ghost, label: "Cancel") { dismiss() }
                }
                ScrollView {
                    VStack(alignment: .leading, spacing: 22) {
                        Spacer().frame(height: 10)
                        nameSection
                        querySection
                        insertersSection
                        displaySection
                        saveSection
                        if !isNew {
                            deleteSection
                        }
                        Spacer().frame(height: 80)
                    }
                    .padding(.horizontal, 18)
                }
            }
            .background(theme.bg)
        }
    }

    // ── Sections (GrSettingsView idiom) ─────────────────────────────────

    private var nameSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Name")
            card {
                TextField("This week", text: $name)
                    .font(.system(size: 14.5))
                    .foregroundStyle(theme.fgDefault)
                    .tint(theme.accentPrimary)
                    .submitLabel(.done)
            }
        }
    }

    private var querySection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Query")
            card {
                VStack(alignment: .leading, spacing: 8) {
                    TextField(
                        "status:todo tag:project -has:scheduled",
                        text: $dsl,
                        axis: .vertical
                    )
                    .font(.system(size: 13.5, design: .monospaced))
                    .foregroundStyle(theme.fgDefault)
                    .tint(theme.accentPrimary)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .lineLimit(2...6)
                    if !dsl.isEmpty {
                        tokenPreviewRow
                    }
                }
            }
            completionStrip
            if let inlineError {
                Text(inlineError)
                    .font(.system(size: 11))
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.horizontal, 4)
            } else {
                sectionCaption(
                    "key:value filters, comma = OR within a key "
                    + "(status:backlog,todo), - negates (-has:scheduled)."
                )
            }
        }
    }

    /// The inline validation/save error. A live DSL parse error wins so
    /// the user sees it while typing; otherwise the last save failure.
    /// tesela-vp9.5: the parse error text now comes from
    /// `SavedViewLogic.dslValidationError`'s real `QueryDiagnostic` hint
    /// (span-located, e.g. "'AND' has no right-hand predicate — near
    /// “AND”") rather than one generic message.
    private var inlineError: String? {
        if !dsl.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
           let err = SavedViewLogic.dslValidationError(dsl) {
            return err
        }
        return errorText
    }

    /// Read-only, horizontally-scrollable line under the query field
    /// rendering the CURRENT text's tokens colored by kind (spec item 2)
    /// — key/operator/value/string/number/paren, theme colors mirroring
    /// how `InlineNLPHighlighter` colors inline-NLP token kinds in the
    /// UITextView-backed editors — with diagnostic spans underlined red.
    private var tokenPreviewRow: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            previewText
                .font(.system(size: 11.5, design: .monospaced))
                .fixedSize(horizontal: true, vertical: false)
        }
        .scrollClipDisabled()
    }

    private var previewText: Text {
        let diagnostics = LocalQueryEngine.parseSimpleDslWithDiagnostics(dsl).diagnostics
        let spans = QueryAuthoring.buildPreviewSpans(dsl, diagnostics: diagnostics)
        return spans.reduce(Text("")) { acc, span in
            var piece = Text(span.text).foregroundStyle(previewColor(for: span.kind))
            if span.diagnostic {
                piece = piece.underline(true, color: .red)
            }
            return acc + piece
        }
    }

    private func previewColor(for kind: QueryAuthoring.PreviewTokenKind?) -> Color {
        switch kind {
        case .key: return theme.accentSecondary
        case .operatorKind: return theme.fgMuted
        case .value: return theme.fgDefault
        case .string: return theme.typeNote
        case .number: return theme.typeProject
        case .paren: return theme.fgFaint
        case nil: return theme.fgFaint
        }
    }

    /// Suggestion strip driven by caret-context classification (spec
    /// item 1). SwiftUI's `TextField` doesn't expose a real caret
    /// position without a `UIViewRepresentable` migration the spec
    /// defers past v1 — so the "working caret" here is always the END of
    /// the current text (`dsl.utf8.count`). Practically: completions
    /// reflect "what comes next if you keep typing", not mid-string
    /// editing; tapping a suggestion always appends/completes at the end.
    private var completionStrip: some View {
        let ctx = QueryAuthoring.caretContext(dsl, cursor: dsl.utf8.count)
        let items = QueryAuthoring.buildCompletions(
            ctx,
            properties: propertyRegistry.properties,
            typeNames: propertyRegistry.typeNames()
        )
        return Group {
            if !items.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 7) {
                        ForEach(items) { item in
                            GrChip(label: item.label) {
                                applyCompletion(ctx, item.label)
                            }
                        }
                    }
                    .padding(.horizontal, 4)
                }
                .scrollClipDisabled()
            }
        }
    }

    private func applyCompletion(_ ctx: QueryAuthoring.CaretContext, _ label: String) {
        let result = QueryAuthoring.applyCompletion(dsl, ctx, label)
        dsl = result.text
        errorText = nil
    }

    private var insertersSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Insert filters")
            ForEach(categories, id: \.self) { category in
                chipRow(for: category)
            }
            sectionCaption("Tap a chip to add or remove its fragment from the query.")
        }
    }

    private var categories: [ChipDef.Category] {
        var seen: [ChipDef.Category] = []
        for chip in chipRegistry where !seen.contains(chip.category) {
            seen.append(chip.category)
        }
        return seen
    }

    private func chipRow(for category: ChipDef.Category) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 7) {
                ForEach(chipRegistry.filter { $0.category == category }, id: \.id) { chip in
                    // tesela-vp9.5: inserter chips write/toggle the
                    // chip's canonical JQL clause (`chip.jqlClause`), not
                    // the legacy `chip.clauses` colon-DSL fragment the
                    // live Inbox toolbar's `chipsFromDsl`/`dslFromChips`
                    // round-trip still uses — see `ChipDef.jqlClause`'s
                    // doc for why the two stay separate.
                    GrChip(
                        label: "\(chip.glyph) \(chip.label)",
                        active: SavedViewLogic.fragmentActive(chip.jqlClause, in: dsl)
                    ) {
                        dsl = SavedViewLogic.toggleFragment(chip.jqlClause, in: dsl)
                        errorText = nil
                    }
                }
            }
            .padding(.horizontal, 4)
        }
        .scrollClipDisabled()
    }

    private var displaySection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("Display")
            card {
                HStack(spacing: 4) {
                    modeButton("list", title: "List")
                    modeButton("table", title: "Table")
                    modeButton("kanban", title: "Kanban")
                }
                .padding(3)
                .background(theme.bg3)
                .overlay(
                    RoundedRectangle(cornerRadius: 9)
                        .stroke(theme.lineSoft, lineWidth: 1)
                )
                .clipShape(RoundedRectangle(cornerRadius: 9))
            }
            if displayMode != "list" {
                sectionCaption(
                    "Stored for the web app — iOS renders every view as a "
                    + "list. The \(displayMode) layout applies on web."
                )
            }
        }
    }

    private func modeButton(_ mode: String, title: String) -> some View {
        let on = displayMode == mode
        return Button {
            displayMode = mode
        } label: {
            Text(title)
                .font(.system(size: 12.5, weight: on ? .semibold : .regular))
                .foregroundStyle(on ? theme.fgDefault : theme.fgMuted)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 7)
                .background(on ? theme.bg4 : .clear)
                .clipShape(RoundedRectangle(cornerRadius: 7))
        }
        .buttonStyle(.plain)
    }

    private var saveSection: some View {
        GrButton(
            variant: .cta,
            label: busy ? "Saving…" : (isNew ? "Create view" : "Save changes")
        ) {
            Task { await save() }
        }
        .disabled(busy)
    }

    @ViewBuilder
    private var deleteSection: some View {
        if isBuiltin {
            sectionCaption(
                "Built-in view — the name, query, and display mode are "
                + "editable, but it can't be deleted."
            )
        } else {
            Button {
                confirmDelete = true
            } label: {
                Text("Delete view")
                    .font(.system(size: 13.5, weight: .medium))
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 11)
                    .background(theme.bg2)
                    .overlay(
                        RoundedRectangle(cornerRadius: 10)
                            .stroke(theme.lineSoft, lineWidth: 1)
                    )
                    .clipShape(RoundedRectangle(cornerRadius: 10))
            }
            .buttonStyle(.plain)
            .disabled(busy)
            .confirmationDialog(
                "Delete “\(existing?.name ?? "")”?",
                isPresented: $confirmDelete,
                titleVisibility: .visible
            ) {
                Button("Delete view", role: .destructive) {
                    Task { await deleteNow() }
                }
            }
        }
    }

    // ── Actions ─────────────────────────────────────────────────────────

    private func save() async {
        let trimmedName = name.trimmingCharacters(in: .whitespaces)
        guard !trimmedName.isEmpty else {
            errorText = "Name must not be empty"
            return
        }
        let trimmedDsl = dsl.trimmingCharacters(in: .whitespacesAndNewlines)
        if let err = SavedViewLogic.dslValidationError(trimmedDsl) {
            errorText = err
            return
        }
        busy = true
        defer { busy = false }
        var record = existing ?? SavedView(
            id: UUID().uuidString,
            name: trimmedName,
            dsl: trimmedDsl,
            // Append after the current last view, steps of 10 — the
            // server's order-minting rule.
            order: (siblings.map(\.order).max() ?? -10) + 10,
            builtin: false,
            displayMode: displayMode,
            displayGroupBy: nil,
            displayShowDone: nil
        )
        record.name = trimmedName
        record.dsl = trimmedDsl
        record.displayMode = displayMode
        do {
            try await onSave(record, isNew)
            dismiss()
        } catch {
            errorText = error.localizedDescription
        }
    }

    private func deleteNow() async {
        guard let existing, !existing.builtin else { return }
        busy = true
        defer { busy = false }
        do {
            try await onDelete(existing.id)
            dismiss()
        } catch {
            errorText = error.localizedDescription
        }
    }

    // ── Graphite section idiom (mirrors GrSettingsView) ─────────────────

    private func sectionLabel(_ text: String) -> some View {
        Text(text.uppercased())
            .font(.system(size: 11, weight: .semibold))
            .tracking(0.6)
            .foregroundStyle(theme.fgMuted)
            .padding(.horizontal, 4)
    }

    private func sectionCaption(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 10.5, design: .monospaced))
            .foregroundStyle(theme.fgFaint)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 4)
    }

    private func card<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(spacing: 0) { content() }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(theme.bg2)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(theme.lineSoft, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
