import SwiftUI

/// Shared task-status marker (Todoist-style): a thick PRIORITY-COLORED ring
/// whose SHAPE is driven by `status` and whose COLOR is driven by `priority`
/// — except DONE, which is always green. The single source of truth for the
/// task marker across BlockRow, the Agenda views, and the Inbox, so the
/// surfaces never drift again. Color hexes are fixed brand/Tailwind values
/// (theme-independent), matching the web client (BlockOutliner statusChar/
/// statusColorClass + priority.ts).
struct TaskStatusMarker: View {
    /// `status::` value; trim/lowercase handled internally. nil/unknown → todo ring.
    var status: String?
    /// Raw `priority::` string (e.g. "p1" / "high" / "2"); nil → neutral gray.
    var priority: String? = nil
    /// Diameter. BlockRow/AgendaView use 16; GrAgendaView uses 18.
    var size: CGFloat = 16
    /// Tap handler (toggle done). nil → non-interactive.
    var onTap: (() -> Void)? = nil

    private var normalizedStatus: String {
        (status ?? "").trimmingCharacters(in: .whitespaces).lowercased()
    }

    /// Priority level 1–3 (web priority.ts); p4 / low / none / unset → nil.
    private var priorityLevel: Int? {
        guard let v = priority?.trimmingCharacters(in: .whitespaces).lowercased(),
              !v.isEmpty else { return nil }
        switch v {
        case "p1", "critical", "urgent", "1": return 1
        case "p2", "high", "2": return 2
        case "p3", "medium", "med", "3": return 3
        default: return nil
        }
    }

    /// Ring color from priority; neutral gray when no priority is set.
    private var markerColor: Color {
        switch priorityLevel {
        case 1: return Color(hex: 0xEB5C58)
        case 2: return Color(hex: 0xE8A33D)
        case 3: return Color(hex: 0x6B9AE0)
        default: return Color(hex: 0x8A909C)
        }
    }

    var body: some View {
        ZStack {
            switch normalizedStatus {
            case "done", "completed":
                Circle().fill(Color(hex: 0x34D399)) // done → always green
                Image(systemName: "checkmark")
                    .font(.system(size: size * 0.5, weight: .bold))
                    .foregroundStyle(.white)
            case "doing", "in-review":
                Circle().strokeBorder(markerColor, lineWidth: 2.5)
                Circle().fill(markerColor).frame(width: size * 0.375, height: size * 0.375)
            case "canceled", "cancelled":
                Circle().strokeBorder(markerColor, lineWidth: 2.5)
                Text("✗").font(.system(size: size * 0.56, weight: .bold)).foregroundStyle(markerColor)
            case "blocked":
                Circle().strokeBorder(markerColor, lineWidth: 2.5)
                Text("⧖").font(.system(size: size * 0.5)).foregroundStyle(markerColor)
            case "paused":
                Circle().strokeBorder(markerColor, lineWidth: 2.5)
                Text("⏸").font(.system(size: size * 0.5)).foregroundStyle(markerColor)
            default:
                Circle().strokeBorder(markerColor, lineWidth: 2.5) // todo / unset
            }
        }
        .frame(width: size, height: size)
        .contentShape(Rectangle())
        .onTapGesture { onTap?() }
    }
}
