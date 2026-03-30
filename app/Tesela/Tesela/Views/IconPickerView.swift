import SwiftUI

// MARK: - IconPickerView
// Searchable grid of all SF Symbols for choosing a tag page icon.

struct IconPickerView: View {
    let currentColor: Color
    let onSelect: (String) -> Void
    @State private var searchText = ""

    private static let allSymbols: [String] = {
        guard let url = Bundle.main.url(forResource: "sf-symbols", withExtension: "txt"),
              let content = try? String(contentsOf: url) else {
            return fallbackSymbols
        }
        return content.components(separatedBy: "\n").filter { !$0.isEmpty }
    }()

    private var filteredSymbols: [String] {
        if searchText.isEmpty { return Self.allSymbols }
        let q = searchText.lowercased()
        return Self.allSymbols.filter { $0.contains(q) }
    }

    var body: some View {
        VStack(spacing: 0) {
            TextField("Search icons…", text: $searchText)
                .textFieldStyle(.roundedBorder)
                .padding(8)

            Text("\(filteredSymbols.count) icons")
                .font(.caption2).foregroundStyle(.tertiary)
                .padding(.bottom, 4)

            ScrollView {
                LazyVGrid(columns: Array(repeating: GridItem(.fixed(36)), count: 8), spacing: 4) {
                    ForEach(filteredSymbols, id: \.self) { symbol in
                        Button {
                            onSelect(symbol)
                        } label: {
                            Image(systemName: symbol)
                                .font(.system(size: 14))
                                .frame(width: 32, height: 32)
                                .foregroundStyle(currentColor)
                                .background(Color.secondary.opacity(0.06), in: RoundedRectangle(cornerRadius: 4))
                        }
                        .buttonStyle(.plain)
                        .help(symbol)
                    }
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 8)
            }
        }
        .frame(width: 320, height: 380)
    }

    // Fallback if sf-symbols.txt not found
    private static let fallbackSymbols: [String] = [
        "checkmark.square", "checkmark.circle", "circle", "square", "star", "star.fill",
        "heart", "flag", "flag.fill", "bell", "bolt", "clock", "hourglass",
        "person", "person.2", "person.circle", "envelope", "phone", "bubble.left",
        "doc", "doc.text", "folder", "folder.fill", "note.text", "book", "bookmark",
        "list.bullet", "list.clipboard", "chart.bar", "calendar",
        "wrench.and.screwdriver", "gearshape", "hammer", "paintbrush", "key", "lock",
        "magnifyingglass", "lightbulb", "cart", "bag", "gift", "creditcard",
        "globe", "map", "mappin", "house", "building.2", "airplane", "car", "leaf",
        "sun.max", "moon", "cloud", "flame", "snowflake", "mountain.2",
        "desktopcomputer", "laptopcomputer", "keyboard", "terminal", "cpu",
        "camera", "photo", "music.note", "gamecontroller", "puzzlepiece",
        "sparkles", "wand.and.stars", "paintpalette",
        "arrow.right", "link", "pin", "tag", "number", "scope",
    ]
}

// MARK: - ColorPickerButton
// Compact color picker for tag icon color
struct TagColorPicker: View {
    let currentColor: Color
    let onSelect: (Color) -> Void

    private static let presetColors: [(String, Color)] = [
        ("Default", .secondary),
        ("Red", .red),
        ("Orange", .orange),
        ("Yellow", .yellow),
        ("Green", .green),
        ("Mint", .mint),
        ("Teal", .teal),
        ("Cyan", .cyan),
        ("Blue", .blue),
        ("Indigo", .indigo),
        ("Purple", .purple),
        ("Pink", .pink),
        ("Brown", .brown),
    ]

    var body: some View {
        HStack(spacing: 6) {
            ForEach(Self.presetColors, id: \.0) { name, color in
                Button {
                    onSelect(color)
                } label: {
                    Circle()
                        .fill(color)
                        .frame(width: 18, height: 18)
                        .overlay {
                            if colorsMatch(currentColor, color) {
                                Image(systemName: "checkmark")
                                    .font(.caption2).bold()
                                    .foregroundStyle(.white)
                            }
                        }
                }
                .buttonStyle(.plain)
                .help(name)
            }
        }
    }

    private func colorsMatch(_ a: Color, _ b: Color) -> Bool {
        // Simple comparison via description
        a.description == b.description
    }
}
