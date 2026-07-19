import Foundation

enum TeselaDeepLinkDestination: Equatable {
    case agenda
    case views

    var tab: AppTab {
        switch self {
        case .agenda: return .agenda
        case .views: return .views
        }
    }
}

enum TeselaDeepLink {
    static func destination(for url: URL) -> TeselaDeepLinkDestination? {
        guard url.scheme?.lowercased() == "tesela" else { return nil }
        let route = (url.host ?? url.pathComponents.dropFirst().first)?.lowercased()
        switch route {
        case "agenda": return .agenda
        case "views", "inbox": return .views
        default: return nil
        }
    }
}
