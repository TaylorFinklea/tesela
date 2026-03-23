import Foundation

// MARK: - BlockParser
// Bidirectional conversion between Markdown bullet-list format and Block trees.
//
// Format:
//   - Top-level block #tag
//     - Child block
//       - Grandchild
//
// Each `- ` prefix line is a block. Indentation (2 spaces per level) encodes nesting.

enum BlockParser {
    // MARK: - Parse
    static func parse(markdown: String) -> [Block] {
        let lines = markdown.components(separatedBy: "\n")
        var roots: [Block] = []
        var stack: [(block: Block, indent: Int)] = []
        var lastBlock: Block?

        for line in lines {
            guard !line.trimmingCharacters(in: .whitespaces).isEmpty else { continue }

            let (indent, text) = indentAndText(from: line)

            if text.hasPrefix("- ") {
                // New block
                let rawText = String(text.dropFirst(2))
                let block = makeBlock(text: rawText, indentLevel: indent)

                while let top = stack.last, top.indent >= indent {
                    stack.removeLast()
                }

                if let parent = stack.last {
                    parent.block.children.append(block)
                } else {
                    roots.append(block)
                }

                stack.append((block, indent))
                lastBlock = block
            } else if let block = lastBlock {
                // Continuation line (property or multi-line text) — belongs to last block
                block.text += "\n" + text.trimmingCharacters(in: .whitespaces)
                // Re-extract properties from the updated text
                let props = extractProperties(from: block.text)
                block.properties = props
                block.priority = Priority(rawValue: block.properties.removeValue(forKey: "priority") ?? "")
                block.deadline = stripWikiLink(block.properties.removeValue(forKey: "deadline"))
                block.scheduled = stripWikiLink(block.properties.removeValue(forKey: "scheduled"))
                block.effort = block.properties.removeValue(forKey: "effort")
            }
        }

        return roots
    }

    // MARK: - Serialize (tree-based)
    static func serialize(blocks: [Block]) -> String {
        var lines: [String] = []
        serializeBlocks(blocks, into: &lines, depth: 0)
        return lines.joined(separator: "\n")
    }

    private static func serializeBlocks(_ blocks: [Block], into lines: inout [String], depth: Int) {
        for block in blocks {
            let indent = String(repeating: "  ", count: depth)
            lines.append("\(indent)- \(block.text)")
            if !block.isCollapsed {
                serializeBlocks(block.children, into: &lines, depth: depth + 1)
            }
        }
    }

    // MARK: - Flatten tree to flat list
    // Sets indentLevel on each block from its depth in the tree.
    static func flatten(blocks: [Block], depth: Int = 0) -> [Block] {
        var result: [Block] = []
        for block in blocks {
            block.indentLevel = depth
            result.append(block)
            result += flatten(blocks: block.children, depth: depth + 1)
        }
        return result
    }

    // MARK: - Serialize flat block list using indentLevel
    // Use this for editor round-trips where blocks are stored as a flat array.
    static func serializeFlat(blocks: [Block]) -> String {
        blocks
            .map { block in
                let indent = String(repeating: "  ", count: block.indentLevel)
                let contIndent = indent + "  "
                let lines = block.text.components(separatedBy: "\n")
                let first = "\(indent)- \(lines[0])"
                if lines.count <= 1 { return first }
                let rest = lines.dropFirst().map { "\(contIndent)\($0)" }
                return ([first] + rest).joined(separator: "\n")
            }
            .joined(separator: "\n")
    }

    // MARK: - Helpers

    private static func indentAndText(from line: String) -> (indent: Int, text: String) {
        var spaces = 0
        for char in line {
            if char == " " { spaces += 1 }
            else { break }
        }
        let indent = spaces / 2
        let text = String(line.dropFirst(spaces))
        return (indent, text)
    }

    private static func makeBlock(text: String, indentLevel: Int) -> Block {
        let (todoState, cleanText) = extractTodo(from: text)
        let tags = extractTags(from: cleanText)
        var properties = extractProperties(from: cleanText)

        // Extract first-class task properties from generic properties
        let block = Block(
            text: cleanText,
            indentLevel: indentLevel,
            todoState: todoState,
            tags: tags,
            properties: properties
        )
        block.priority = Priority(rawValue: properties.removeValue(forKey: "priority") ?? "")
        block.deadline = stripWikiLink(properties.removeValue(forKey: "deadline"))
        block.scheduled = stripWikiLink(properties.removeValue(forKey: "scheduled"))
        block.effort = properties.removeValue(forKey: "effort")
        block.properties = properties  // remaining non-task properties
        return block
    }

    static func extractTodo(from text: String) -> (TodoState?, String) {
        for state in TodoState.allCases {
            let prefix = "\(state.rawValue) "
            if text.hasPrefix(prefix) {
                return (state, String(text.dropFirst(prefix.count)))
            }
        }
        return (nil, text)
    }

    static func extractTags(from text: String) -> [String] {
        let pattern = /#([A-Za-z0-9_\-]+)/
        return text.matches(of: pattern).map { String($0.output.1) }
    }

    static func extractProperties(from text: String) -> [String: String] {
        var props: [String: String] = [:]
        let pattern = /([A-Za-z_][A-Za-z0-9_]*):: (.+)/
        for match in text.matches(of: pattern) {
            props[String(match.output.1)] = String(match.output.2)
        }
        return props
    }

    // Strip [[...]] wrapper from a property value (dates stored as wiki-links)
    private static func stripWikiLink(_ value: String?) -> String? {
        guard var v = value else { return nil }
        if v.hasPrefix("[[") && v.hasSuffix("]]") {
            v = String(v.dropFirst(2).dropLast(2))
        }
        return v.isEmpty ? nil : v
    }

    // MARK: - Wiki-link extraction
    static func extractWikiLinks(from text: String) -> [String] {
        let pattern = /\[\[([^\]]+)\]\]/
        return text.matches(of: pattern).map { String($0.output.1) }
    }
}
