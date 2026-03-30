import SwiftUI

// MARK: - IconPickerView
// Searchable grid of SF Symbols for choosing a tag page icon.

struct IconPickerView: View {
    let onSelect: (String) -> Void
    @State private var searchText = ""

    private var filteredIcons: [(name: String, symbol: String)] {
        if searchText.isEmpty { return Self.icons }
        let q = searchText.lowercased()
        return Self.icons.filter { $0.name.contains(q) }
    }

    var body: some View {
        VStack(spacing: 0) {
            TextField("Search icons…", text: $searchText)
                .textFieldStyle(.roundedBorder)
                .padding(8)

            Text("\(filteredIcons.count) icons")
                .font(.caption2).foregroundStyle(.tertiary)
                .padding(.bottom, 4)

            ScrollView {
                LazyVGrid(columns: Array(repeating: GridItem(.fixed(36)), count: 7), spacing: 6) {
                    ForEach(filteredIcons, id: \.symbol) { icon in
                        Button {
                            onSelect(icon.symbol)
                        } label: {
                            Image(systemName: icon.symbol)
                                .font(.system(size: 16))
                                .frame(width: 32, height: 32)
                                .foregroundStyle(.primary)
                                .background(Color.secondary.opacity(0.08), in: RoundedRectangle(cornerRadius: 6))
                        }
                        .buttonStyle(.plain)
                        .help(icon.name)
                    }
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 8)
            }
        }
        .frame(width: 290, height: 340)
    }

    // Curated SF Symbols organized by use case
    static let icons: [(name: String, symbol: String)] = [
        // Tasks & Status
        ("checkmark square", "checkmark.square"),
        ("checkmark circle", "checkmark.circle"),
        ("checkmark", "checkmark"),
        ("circle", "circle"),
        ("square", "square"),
        ("exclamationmark triangle", "exclamationmark.triangle"),
        ("xmark circle", "xmark.circle"),
        ("clock", "clock"),
        ("hourglass", "hourglass"),
        ("flag", "flag"),
        ("flag fill", "flag.fill"),
        ("bell", "bell"),
        ("star", "star"),
        ("star fill", "star.fill"),
        ("heart", "heart"),
        ("bolt", "bolt"),

        // People & Communication
        ("person", "person"),
        ("person 2", "person.2"),
        ("person 3", "person.3"),
        ("person circle", "person.circle"),
        ("envelope", "envelope"),
        ("phone", "phone"),
        ("bubble left", "bubble.left"),
        ("bubble left and right", "bubble.left.and.bubble.right"),
        ("at", "at"),
        ("video", "video"),

        // Documents & Files
        ("doc", "doc"),
        ("doc text", "doc.text"),
        ("doc plaintext", "doc.plaintext"),
        ("folder", "folder"),
        ("folder fill", "folder.fill"),
        ("archivebox", "archivebox"),
        ("tray", "tray"),
        ("paperclip", "paperclip"),
        ("note text", "note.text"),
        ("book", "book"),
        ("bookmark", "bookmark"),
        ("list bullet", "list.bullet"),
        ("list clipboard", "list.clipboard"),
        ("chart bar", "chart.bar"),

        // Tools & Objects
        ("wrench", "wrench.and.screwdriver"),
        ("gear", "gearshape"),
        ("slider horizontal", "slider.horizontal.3"),
        ("hammer", "hammer"),
        ("paintbrush", "paintbrush"),
        ("scissors", "scissors"),
        ("key", "key"),
        ("lock", "lock"),
        ("magnifyingglass", "magnifyingglass"),
        ("lightbulb", "lightbulb"),
        ("lamp desk", "lamp.desk"),
        ("cart", "cart"),
        ("bag", "bag"),
        ("gift", "gift"),
        ("creditcard", "creditcard"),
        ("banknote", "banknote"),

        // Nature & Places
        ("globe", "globe"),
        ("map", "map"),
        ("mappin", "mappin"),
        ("house", "house"),
        ("building", "building.2"),
        ("airplane", "airplane"),
        ("car", "car"),
        ("leaf", "leaf"),
        ("tree", "tree"),
        ("sun max", "sun.max"),
        ("moon", "moon"),
        ("cloud", "cloud"),
        ("drop", "drop"),
        ("flame", "flame"),
        ("snowflake", "snowflake"),
        ("mountain", "mountain.2"),

        // Technology
        ("desktopcomputer", "desktopcomputer"),
        ("laptop", "laptopcomputer"),
        ("phone", "iphone"),
        ("keyboard", "keyboard"),
        ("printer", "printer"),
        ("wifi", "wifi"),
        ("antenna radiowaves", "antenna.radiowaves.left.and.right"),
        ("server rack", "server.rack"),
        ("cpu", "cpu"),
        ("memorychip", "memorychip"),
        ("terminal", "terminal"),
        ("chevron left forwardslash chevron right", "chevron.left.forwardslash.chevron.right"),
        ("curly braces", "curlybraces"),

        // Creative
        ("paintpalette", "paintpalette"),
        ("camera", "camera"),
        ("photo", "photo"),
        ("music note", "music.note"),
        ("film", "film"),
        ("gamecontroller", "gamecontroller"),
        ("puzzlepiece", "puzzlepiece"),
        ("wand and stars", "wand.and.stars"),
        ("sparkles", "sparkles"),

        // Arrows & Symbols
        ("arrow right", "arrow.right"),
        ("arrow triangle branch", "arrow.triangle.branch"),
        ("link", "link"),
        ("pin", "pin"),
        ("tag", "tag"),
        ("number", "number"),
        ("textformat", "textformat"),
        ("calendar", "calendar"),
        ("scope", "scope"),
    ]
}
