import SwiftUI

/// Modal sheet that shows the active Inbox filter's raw DSL string and
/// lets the user edit it directly. Mirrors `web/src/lib/ambients/inbox/
/// RawDslSheet.svelte` — a multi-line `TextEditor`, a short JQL-style
/// example at the top of the hint, and a collapsible grammar reference
/// covering every clause shape the parser understands.
///
/// The sheet is purely presentational: the parent (`InboxView`) owns
/// the active slug + save path. Tapping Save calls back with the
/// trimmed DSL; tapping Cancel just dismisses.
struct InboxRawDslSheet: View {
    let initialDsl: String
    var onSave: (String) -> Void
    var onCancel: () -> Void

    @Environment(\.theme) private var theme
    @State private var draft: String
    @State private var grammarExpanded: Bool = false

    init(initialDsl: String, onSave: @escaping (String) -> Void, onCancel: @escaping () -> Void) {
        self.initialDsl = initialDsl
        self.onSave = onSave
        self.onCancel = onCancel
        self._draft = State(initialValue: initialDsl)
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                TextEditor(text: $draft)
                    .font(.system(size: 14, design: .monospaced))
                    .scrollContentBackground(.hidden)
                    .background(theme.bg2)
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .stroke(theme.lineSoft, lineWidth: 1)
                    )
                    .frame(minHeight: 120, maxHeight: 200)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)

                hint

                Spacer(minLength: 0)
            }
            .padding(16)
            .background(theme.bg)
            .navigationTitle("Edit Views query")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { onCancel() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        let trimmed = draft.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !trimmed.isEmpty else { return }
                        onSave(trimmed)
                    }
                    .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
        }
    }

    // MARK: - Hint + grammar reference

    @ViewBuilder
    private var hint: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("JQL-style. `kind:block` implicit. Example:")
                .font(.system(size: 11))
                .foregroundStyle(theme.fgFaint)
            Text(#"status != done AND type IN (task, issue) AND scheduled IS NOT NULL ORDER BY scheduled DESC"#)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
                .padding(.vertical, 4)
                .padding(.horizontal, 8)
                .background(theme.bg2.opacity(0.6))
                .clipShape(RoundedRectangle(cornerRadius: 4))

            DisclosureGroup(isExpanded: $grammarExpanded) {
                grammarReference
                    .padding(.top, 6)
            } label: {
                Text("grammar reference")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
    }

    private var grammarReference: some View {
        VStack(alignment: .leading, spacing: 4) {
            grammarRow("combinators", "AND  OR  NOT  ( )")
            grammarRow("compare", "=  !=  <  <=  >  >=")
            grammarRow("membership", "key IN (a, b, c)   key NOT IN (…)")
            grammarRow("presence", "key IS NULL   key IS NOT NULL   (EMPTY alias)")
            grammarRow("range", "key BETWEEN a AND b   (inclusive)")
            grammarRow("pattern", #"text LIKE "wood%"   key NOT LIKE "…"   (% any, _ one)"#)
            grammarRow("sort", "ORDER BY key [ASC|DESC] [, key2 …]")
            grammarRow("keys", "tag/type · status · has · is · on · page · block · text · <property>")
            grammarRow("legacy", "key:value · -key:value · has:foo · tag-in:a,b,c")
        }
    }

    private func grammarRow(_ label: String, _ value: String) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Text(label)
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(theme.fgFaint)
                .frame(width: 88, alignment: .leading)
            Text(value)
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(theme.fgSubtle)
                .textSelection(.enabled)
        }
    }
}
