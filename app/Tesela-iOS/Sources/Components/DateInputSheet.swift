import SwiftUI

/// Sheet for entering a date / recurrence on a block.
struct DateInputSheet: View {
    /// Pre-fill state from the block being edited (nil for a fresh entry).
    let initialScheduled: String?
    let initialDeadline: String?
    let initialRecurrence: String?
    /// Whether the underlying block already carries `recurring::` — used to gate the Skip button.
    let canSkip: Bool
    /// User's bareDateField default ("scheduled" / "deadline").
    let bareDateFieldDefault: String
    /// Called on Set with the resolved (field, isoDate, time?, recurrence?).
    let onCommit: (DateField, String, String?, String?) -> Void
    /// Called when the user taps Skip (only relevant if canSkip).
    let onSkip: () -> Void
    /// Called on Cancel or dismiss.
    let onCancel: () -> Void

    @State private var nlInput: String = ""
    @State private var pickedDate: Date = Date()
    @State private var pickedTime: Date? = nil
    @State private var pickedRecurrence: String? = nil
    @State private var resolvedField: DateField = .scheduled

    var body: some View {
        NavigationStack {
            Form {
                Section("Date") {
                    TextField("e.g. tomorrow, next fri, deadline may 23", text: $nlInput)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    DatePicker(
                        "Date",
                        selection: $pickedDate,
                        displayedComponents: [.date]
                    )
                    .datePickerStyle(.graphical)
                    Toggle("Set time", isOn: Binding(
                        get: { pickedTime != nil },
                        set: { pickedTime = $0 ? (pickedTime ?? Date()) : nil }
                    ))
                    if let timeBinding = Binding($pickedTime) {
                        DatePicker("Time", selection: timeBinding, displayedComponents: [.hourAndMinute])
                    }
                }
                Section("Repeat") {
                    Picker("Repeat", selection: Binding(
                        get: { pickedRecurrence ?? "none" },
                        set: { pickedRecurrence = $0 == "none" ? nil : $0 }
                    )) {
                        Text("None").tag("none")
                        Text("Daily").tag("daily")
                        Text("Weekdays").tag("weekdays")
                        Text("Weekly").tag("weekly")
                        Text("Monthly").tag("monthly")
                        Text("Yearly").tag("yearly")
                        // When the stored recurrence is a non-preset string
                        // (e.g. "every mon, wed" from natural language input),
                        // add a dynamic row so the Picker highlights it rather
                        // than showing nothing selected.
                        if let r = pickedRecurrence,
                           !["daily", "weekdays", "weekly", "monthly", "yearly"].contains(r) {
                            Text(r).tag(r)
                        }
                    }
                }
                Section {
                    HStack {
                        Text("Will set")
                        Spacer()
                        Text(resolvedField == .deadline ? "Deadline" : "Scheduled")
                            .foregroundStyle(.secondary)
                    }
                }
                if canSkip {
                    Section {
                        Button("Skip to next occurrence") { onSkip() }
                    }
                }
            }
            .navigationTitle("Date")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { onCancel() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Set") { commit() }
                }
            }
            .onChange(of: nlInput) { _, newValue in reparse(newValue) }
            .onAppear { seedFromInitial() }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
    }

    private func seedFromInitial() {
        // Prefer scheduled over deadline for the pre-fill date.
        let initialDate = initialScheduled ?? initialDeadline
        if let raw = initialDate {
            let stripped = stripBrackets(raw)
            if let d = parseIso(stripped) { pickedDate = d }
            // Seed the time picker when the stored value contains an
            // `HH:mm` tail (e.g. "2026-05-25 14:30"). `parseIso` only
            // looks at the first 10 chars, so we handle the tail here.
            // "YYYY-MM-DD HH:MM" is exactly 16 chars; guard defensively.
            if stripped.count >= 16 {
                let timeSuffix = String(stripped.suffix(5))
                if let tm = parseTime(timeSuffix) { pickedTime = tm }
            }
        }
        if let initialRecurrence { pickedRecurrence = initialRecurrence }
        resolvedField = (initialDeadline != nil && initialScheduled == nil)
            ? .deadline
            : (bareDateFieldDefault == "deadline" ? .deadline : .scheduled)
    }

    private func reparse(_ s: String) {
        guard !s.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        guard let parsed = DateParser.parse(s) else { return }
        if let d = parseIso(parsed.date) { pickedDate = d }
        // Time: write when parsed; clear when the fresh parse yields no time
        // (e.g. user deleted "at 3pm" from "tomorrow at 3pm").
        if let t = parsed.time, let tm = parseTime(t) {
            pickedTime = tm
        } else {
            pickedTime = nil
        }
        // Recurrence: same — clear when not present in the current parse.
        pickedRecurrence = parsed.recurrence
        resolvedField = parsed.field ?? (bareDateFieldDefault == "deadline" ? .deadline : .scheduled)
    }

    private func commit() {
        let iso = isoString(pickedDate)
        let timeStr = pickedTime.map { timeString($0) }
        onCommit(resolvedField, iso, timeStr, pickedRecurrence)
    }

    private func stripBrackets(_ s: String) -> String {
        var out = s.trimmingCharacters(in: .whitespaces)
        if out.hasPrefix("[[") { out.removeFirst(2) }
        if out.hasSuffix("]]") { out.removeLast(2) }
        return out
    }

    private func parseIso(_ s: String) -> Date? {
        // Accept "YYYY-MM-DD" or "YYYY-MM-DD HH:mm".
        let date = String(s.prefix(10))
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone.current
        return f.date(from: date)
    }
    private func parseTime(_ s: String) -> Date? {
        let f = DateFormatter()
        f.dateFormat = "HH:mm"
        f.locale = Locale(identifier: "en_US_POSIX")
        return f.date(from: s)
    }
    private func isoString(_ d: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone.current
        return f.string(from: d)
    }
    private func timeString(_ d: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "HH:mm"
        f.locale = Locale(identifier: "en_US_POSIX")
        return f.string(from: d)
    }
}
