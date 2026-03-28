import Foundation

// MARK: - APIClient
// Thin URLSession wrapper for the tesela-server REST API at localhost:7474

actor APIClient {
    private let baseURL: URL
    private let session: URLSession
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder

    init(baseURL: URL = URL(string: "http://localhost:7474")!) {
        self.baseURL = baseURL
        self.session = URLSession.shared
        self.decoder = JSONDecoder()
        self.decoder.dateDecodingStrategy = .iso8601
        self.encoder = JSONEncoder()
        self.encoder.dateEncodingStrategy = .iso8601
    }

    // MARK: - Health
    func health() async throws -> Bool {
        let (data, response) = try await get("/health")
        guard (response as? HTTPURLResponse)?.statusCode == 200 else { return false }
        // Server returns {"status":"ok"} or similar
        let json = try? JSONSerialization.jsonObject(with: data) as? [String: String]
        return json?["status"] == "ok" || !data.isEmpty
    }

    // MARK: - Notes
    func listNotes(tag: String? = nil, limit: Int = 200, offset: Int = 0) async throws -> [Page] {
        var query: [URLQueryItem] = [
            URLQueryItem(name: "limit", value: String(limit)),
            URLQueryItem(name: "offset", value: String(offset))
        ]
        if let tag { query.append(URLQueryItem(name: "tag", value: tag)) }
        return try await getDecoded("/notes", query: query)
    }

    func getNote(id: String) async throws -> Page {
        try await getDecoded("/notes/\(id)")
    }

    func getDailyNote(date: String? = nil) async throws -> Page {
        var query: [URLQueryItem] = []
        if let date { query.append(URLQueryItem(name: "date", value: date)) }
        return try await getDecoded("/notes/daily", query: query)
    }

    func createNote(title: String, content: String, tags: [String] = []) async throws -> Page {
        let body = CreateNoteRequest(title: title, content: content, tags: tags)
        return try await postDecoded("/notes", body: body)
    }

    func updateNote(id: String, content: String) async throws -> Page {
        let body = UpdateNoteRequest(content: content)
        return try await putDecoded("/notes/\(id)", body: body)
    }

    func deleteNote(id: String) async throws {
        try await delete("/notes/\(id)")
    }

    func getBacklinks(id: String) async throws -> [Link] {
        try await getDecoded("/notes/\(id)/backlinks")
    }

    func getLinks(id: String) async throws -> [Link] {
        try await getDecoded("/notes/\(id)/links")
    }

    func getAllEdges() async throws -> [GraphEdge] {
        try await getDecoded("/links")
    }

    // MARK: - Search
    func search(query: String, limit: Int = 20, offset: Int = 0) async throws -> [SearchHit] {
        let params: [URLQueryItem] = [
            URLQueryItem(name: "q", value: query),
            URLQueryItem(name: "limit", value: String(limit)),
            URLQueryItem(name: "offset", value: String(offset))
        ]
        return try await getDecoded("/search", query: params)
    }

    // MARK: - Types & Properties
    func getTypes() async throws -> [TypeDefinition] {
        try await getDecoded("/types")
    }

    func getProperties() async throws -> [PropertyDef] {
        try await getDecoded("/properties")
    }

    func getResolvedType(name: String) async throws -> TypeDefinition {
        try await getDecoded("/types/\(name)")
    }

    func getTypedNodes(typeName: String) async throws -> [Page] {
        try await getDecoded("/types/\(typeName)/nodes")
    }

    func getTypedBlocks(typeName: String, filterProperty: String? = nil, filterValue: String? = nil, sortBy: String? = nil, sortDir: String? = nil) async throws -> [TypedBlock] {
        var query: [URLQueryItem] = []
        if let prop = filterProperty, let val = filterValue {
            query.append(URLQueryItem(name: "filter_property", value: prop))
            query.append(URLQueryItem(name: "filter_value", value: val))
        }
        if let sort = sortBy {
            query.append(URLQueryItem(name: "sort_by", value: sort))
            if let dir = sortDir { query.append(URLQueryItem(name: "sort_dir", value: dir)) }
        }
        return try await getDecoded("/types/\(typeName)/blocks", query: query)
    }

    // MARK: - Tags
    func listTags() async throws -> [String] {
        try await getDecoded("/tags")
    }

    // MARK: - Private helpers

    private func get(_ path: String, query: [URLQueryItem] = []) async throws -> (Data, URLResponse) {
        let url = url(path: path, query: query)
        var request = URLRequest(url: url)
        request.timeoutInterval = 10
        return try await session.data(for: request)
    }

    private func getDecoded<T: Decodable>(_ path: String, query: [URLQueryItem] = []) async throws -> T {
        let (data, response) = try await get(path, query: query)
        try validate(response, data: data)
        return try decoder.decode(T.self, from: data)
    }

    private func postDecoded<Body: Encodable, T: Decodable>(_ path: String, body: Body) async throws -> T {
        let data = try await send("POST", path: path, body: body)
        return try decoder.decode(T.self, from: data)
    }

    private func putDecoded<Body: Encodable, T: Decodable>(_ path: String, body: Body) async throws -> T {
        let data = try await send("PUT", path: path, body: body)
        return try decoder.decode(T.self, from: data)
    }

    private func delete(_ path: String) async throws {
        var request = URLRequest(url: url(path: path))
        request.httpMethod = "DELETE"
        request.timeoutInterval = 10
        let (data, response) = try await session.data(for: request)
        try validate(response, data: data)
    }

    private func send<Body: Encodable>(_ method: String, path: String, body: Body) async throws -> Data {
        var request = URLRequest(url: url(path: path))
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try encoder.encode(body)
        request.timeoutInterval = 30
        let (data, response) = try await session.data(for: request)
        try validate(response, data: data)
        return data
    }

    private func url(path: String, query: [URLQueryItem] = []) -> URL {
        var comps = URLComponents(url: baseURL.appendingPathComponent(path), resolvingAgainstBaseURL: false)!
        if !query.isEmpty { comps.queryItems = query }
        return comps.url!
    }

    private func validate(_ response: URLResponse, data: Data) throws {
        guard let http = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }
        guard (200..<300).contains(http.statusCode) else {
            let message = String(data: data, encoding: .utf8) ?? "Unknown error"
            throw APIError.httpError(statusCode: http.statusCode, message: message)
        }
    }
}

// MARK: - Request bodies
private struct CreateNoteRequest: Encodable {
    let title: String
    let content: String
    let tags: [String]
}

private struct UpdateNoteRequest: Encodable {
    let content: String
}

// MARK: - Errors
enum APIError: LocalizedError {
    case invalidResponse
    case httpError(statusCode: Int, message: String)
    case decodingFailed(String)

    var errorDescription: String? {
        switch self {
        case .invalidResponse: "Invalid server response"
        case .httpError(let code, let msg): "HTTP \(code): \(msg)"
        case .decodingFailed(let reason): "Decoding failed: \(reason)"
        }
    }
}
