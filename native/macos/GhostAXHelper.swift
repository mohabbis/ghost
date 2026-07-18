#!/usr/bin/env swift
// GhostAXHelper — macOS Accessibility semantic automation helper (Ghost 2.0).
// Build: make ax-helper  (or bash scripts/build-ghost-ax-helper.sh)
// Bundled via Tauri externalBin as Ghost.app/Contents/MacOS/ghost-ax-helper
// Protocol: one JSON request object on stdin → one JSON response object on stdout.
// Ops: permission_status, frontmost_app, resolve_target, list_matches,
//      activate_element, set_value, verify_element, enumerate_children.
//
// Targeting order (see docs/macos-automation-architecture.md):
//   exact bundle id → exact app name → contains fallback
//   optional window_title narrows the search root
//   identifier (when set) is required
//   AXPress preferred over coordinate-style raise/focus

import Foundation
import AppKit
import ApplicationServices

struct AXRequest: Codable {
    let op: String
    let app: String?
    let role: String?
    let title: String?
    let value: String?
    let fingerprint: String?
    let expected_value: String?
    let identifier: String?
    let bundle_id: String?
    let window_title: String?
}

struct AXResponse: Codable {
    let ok: Bool
    let detail: String
    let match_count: UInt32?
    let fingerprint: String?
    let value: String?
    let ax_quality: UInt32?
    let identifier: String?
    let actionable: Bool?
}

func respond(_ resp: AXResponse) {
    let data = try! JSONEncoder().encode(resp)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
}

func fail(
    _ detail: String,
    count: UInt32? = nil,
    fingerprint: String? = nil,
    value: String? = nil,
    ax_quality: UInt32? = nil,
    identifier: String? = nil,
    actionable: Bool? = nil
) {
    respond(AXResponse(
        ok: false,
        detail: detail,
        match_count: count,
        fingerprint: fingerprint,
        value: value,
        ax_quality: ax_quality,
        identifier: identifier,
        actionable: actionable
    ))
}

func ok(
    _ detail: String,
    count: UInt32? = nil,
    fingerprint: String? = nil,
    value: String? = nil,
    ax_quality: UInt32? = nil,
    identifier: String? = nil,
    actionable: Bool? = nil
) {
    respond(AXResponse(
        ok: true,
        detail: detail,
        match_count: count,
        fingerprint: fingerprint,
        value: value,
        ax_quality: ax_quality,
        identifier: identifier,
        actionable: actionable
    ))
}

func axString(_ element: AXUIElement, _ attr: String) -> String? {
    var value: CFTypeRef?
    let err = AXUIElementCopyAttributeValue(element, attr as CFString, &value)
    guard err == .success, let raw = value else { return nil }
    if let s = raw as? String { return s }
    if let url = raw as? URL { return url.path }
    return nil
}

func axChildren(_ element: AXUIElement) -> [AXUIElement] {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &value) == .success,
          let arr = value as? [AXUIElement] else { return [] }
    return arr
}

func axFrame(_ element: AXUIElement) -> (x: Int, y: Int, w: Int, h: Int)? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXPositionAttribute as CFString, &value) == .success,
          let posVal = value,
          AXValueGetType(posVal as! AXValue) == .cgPoint else { return nil }
    var point = CGPoint.zero
    AXValueGetValue(posVal as! AXValue, .cgPoint, &point)
    var sizeVal: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXSizeAttribute as CFString, &sizeVal) == .success,
          let sizeRef = sizeVal,
          AXValueGetType(sizeRef as! AXValue) == .cgSize else { return nil }
    var size = CGSize.zero
    AXValueGetValue(sizeRef as! AXValue, .cgSize, &size)
    return (Int(point.x), Int(point.y), Int(size.width), Int(size.height))
}

func axActionable(_ element: AXUIElement) -> Bool {
    var actionsRef: CFTypeRef?
    if AXUIElementCopyActionNames(element, &actionsRef) == .success,
       let names = actionsRef as? [String]
    {
        if names.contains(kAXPressAction as String) || names.contains(kAXRaiseAction as String) {
            return true
        }
    }
    var settable: DarwinBoolean = false
    if AXUIElementIsAttributeSettable(element, kAXValueAttribute as CFString, &settable) == .success,
       settable.boolValue
    {
        return true
    }
    if AXUIElementIsAttributeSettable(element, kAXFocusedAttribute as CFString, &settable) == .success,
       settable.boolValue
    {
        return true
    }
    return false
}

func scoreQuality(role: String?, title: String?, identifier: String?, actionable: Bool, matchCount: Int) -> UInt32 {
    var score: UInt32 = 0
    if let role, !role.isEmpty, role != "?" { score += 25 }
    if let title, !title.isEmpty { score += 20 }
    if let identifier, !identifier.isEmpty { score += 35 }
    if actionable { score += 15 }
    if matchCount == 1 { score += 5 }
    return score
}

func fingerprint(_ element: AXUIElement) -> String {
    let role = axString(element, kAXRoleAttribute) ?? "?"
    let title = axString(element, kAXTitleAttribute)
        ?? axString(element, kAXDescriptionAttribute)
        ?? ""
    let identifier = axString(element, kAXIdentifierAttribute) ?? ""
    let frame = axFrame(element)
    let framePart: String
    if let f = frame {
        framePart = "\(f.x),\(f.y),\(f.w),\(f.h)"
    } else {
        framePart = "noframe"
    }
    let idPart = identifier.isEmpty ? "-" : identifier
    return "\(role)|\(title)|\(idPart)|\(framePart)"
}

struct Match {
    let element: AXUIElement
    let fp: String
    let role: String
    let title: String
    let identifier: String
    let actionable: Bool
}

func titleMatches(_ element: AXUIElement, _ wanted: String?) -> Bool {
    guard let wanted = wanted, !wanted.isEmpty else { return true }
    let title = axString(element, kAXTitleAttribute) ?? ""
    let desc = axString(element, kAXDescriptionAttribute) ?? ""
    let lower = wanted.lowercased()
    return title.lowercased().contains(lower) || desc.lowercased().contains(lower)
}

func roleMatches(_ element: AXUIElement, _ wanted: String) -> Bool {
    let role = axString(element, kAXRoleAttribute) ?? ""
    return role == wanted || role.lowercased() == wanted.lowercased()
}

func identifierMatches(_ element: AXUIElement, _ wanted: String?) -> Bool {
    guard let wanted = wanted, !wanted.isEmpty else { return true }
    let id = axString(element, kAXIdentifierAttribute) ?? ""
    return id == wanted || id.lowercased() == wanted.lowercased()
}

func findApp(named name: String, bundleId: String?) -> AXUIElement? {
    let apps = NSWorkspace.shared.runningApplications
    let needle = name.lowercased()

    if let bundleId, !bundleId.isEmpty {
        if let app = apps.first(where: { $0.bundleIdentifier == bundleId }) {
            return AXUIElementCreateApplication(app.processIdentifier)
        }
    }

    if let app = apps.first(where: { ($0.localizedName ?? "").lowercased() == needle }) {
        return AXUIElementCreateApplication(app.processIdentifier)
    }

    if let app = apps.first(where: { ($0.bundleIdentifier ?? "").lowercased() == needle }) {
        return AXUIElementCreateApplication(app.processIdentifier)
    }

    for app in apps {
        if let localized = app.localizedName, localized.lowercased().contains(needle) {
            return AXUIElementCreateApplication(app.processIdentifier)
        }
        if let bundle = app.bundleIdentifier, bundle.lowercased().contains(needle) {
            return AXUIElementCreateApplication(app.processIdentifier)
        }
    }
    return nil
}

func axWindows(_ appElement: AXUIElement) -> [AXUIElement] {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(appElement, kAXWindowsAttribute as CFString, &value) == .success,
          let arr = value as? [AXUIElement] else { return [] }
    return arr
}

func searchRoot(appElement: AXUIElement, windowTitle: String?) -> AXUIElement {
    guard let wanted = windowTitle, !wanted.isEmpty else { return appElement }
    let lower = wanted.lowercased()
    for window in axWindows(appElement) {
        let title = axString(window, kAXTitleAttribute) ?? ""
        if title.lowercased() == lower || title.lowercased().contains(lower) {
            return window
        }
    }
    // Fall back to the full app tree when the window title drifted.
    return appElement
}

func makeMatch(_ element: AXUIElement) -> Match {
    Match(
        element: element,
        fp: fingerprint(element),
        role: axString(element, kAXRoleAttribute) ?? "?",
        title: axString(element, kAXTitleAttribute)
            ?? axString(element, kAXDescriptionAttribute)
            ?? "",
        identifier: axString(element, kAXIdentifierAttribute) ?? "",
        actionable: axActionable(element)
    )
}

func collectMatches(
    root: AXUIElement,
    role: String,
    title: String?,
    identifier: String?,
    maxDepth: Int
) -> [Match] {
    var results: [Match] = []
    func walk(_ element: AXUIElement, depth: Int) {
        if depth > maxDepth { return }
        if roleMatches(element, role)
            && titleMatches(element, title)
            && identifierMatches(element, identifier)
        {
            results.append(makeMatch(element))
        }
        for child in axChildren(element) {
            walk(child, depth: depth + 1)
        }
    }
    walk(root, depth: 0)
    return results
}

func resolve(req: AXRequest) -> [Match] {
    guard let appName = req.app, !appName.isEmpty else { return [] }
    guard let role = req.role, !role.isEmpty else { return [] }
    guard let appEl = findApp(named: appName, bundleId: req.bundle_id) else { return [] }
    let root = searchRoot(appElement: appEl, windowTitle: req.window_title)
    return collectMatches(
        root: root,
        role: role,
        title: req.title,
        identifier: req.identifier,
        maxDepth: 8
    )
}

func quality(for match: Match, matchCount: Int) -> UInt32 {
    scoreQuality(
        role: match.role,
        title: match.title.isEmpty ? nil : match.title,
        identifier: match.identifier.isEmpty ? nil : match.identifier,
        actionable: match.actionable,
        matchCount: matchCount
    )
}

func activate(_ match: Match) -> Bool {
    // Prefer a semantic press when the control exposes it.
    var err = AXUIElementPerformAction(match.element, kAXPressAction as CFString)
    if err == .success { return true }
    err = AXUIElementPerformAction(match.element, kAXRaiseAction as CFString)
    if err == .success { return true }
    err = AXUIElementSetAttributeValue(match.element, kAXFocusedAttribute as CFString, true as CFTypeRef)
    return err == .success
}

func setValue(_ match: Match, value: String) -> Bool {
    let err = AXUIElementSetAttributeValue(match.element, kAXValueAttribute as CFString, value as CFTypeRef)
    return err == .success
}

func readValue(_ match: Match) -> String? {
    axString(match.element, kAXValueAttribute)
}

guard let line = readLine(strippingNewline: true),
      let input = line.data(using: .utf8),
      let req = try? JSONDecoder().decode(AXRequest.self, from: input) else {
    fail("invalid JSON request")
    exit(1)
}

if req.op == "permission_status" {
    let trusted = AXIsProcessTrusted()
    if trusted {
        ok("accessibility granted")
    } else {
        fail("accessibility denied")
    }
    exit(trusted ? 0 : 1)
}

if !AXIsProcessTrusted() {
    fail("accessibility denied")
    exit(1)
}

switch req.op {
case "frontmost_app":
    let app = NSWorkspace.shared.frontmostApplication
    let name = app?.localizedName ?? "unknown"
    let bundle = app?.bundleIdentifier ?? ""
    let detail = bundle.isEmpty ? name : "\(name) (\(bundle))"
    ok(detail)

case "resolve_target":
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("no matching element", count: 0, ax_quality: 0, actionable: false)
    } else if matches.count > 1 {
        let best = matches[0]
        fail(
            "ambiguous semantic target (\(matches.count) matches)",
            count: UInt32(matches.count),
            fingerprint: best.fp,
            ax_quality: quality(for: best, matchCount: matches.count),
            identifier: best.identifier.isEmpty ? nil : best.identifier,
            actionable: best.actionable
        )
    } else {
        let m = matches[0]
        let q = quality(for: m, matchCount: 1)
        ok(
            "resolved \(m.fp)",
            count: 1,
            fingerprint: m.fp,
            ax_quality: q,
            identifier: m.identifier.isEmpty ? nil : m.identifier,
            actionable: m.actionable
        )
    }

case "activate_element":
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("no matching element", count: 0, ax_quality: 0, actionable: false)
    } else if matches.count > 1 {
        fail("ambiguous activate (\(matches.count) matches)", count: UInt32(matches.count))
    } else {
        let m = matches[0]
        let q = quality(for: m, matchCount: 1)
        if let fp = req.fingerprint, fp != m.fp {
            fail(
                "stale target (expected \(fp), observed \(m.fp))",
                fingerprint: m.fp,
                ax_quality: q,
                identifier: m.identifier.isEmpty ? nil : m.identifier,
                actionable: m.actionable
            )
        } else if activate(m) {
            ok(
                "activated \(m.fp)",
                fingerprint: m.fp,
                ax_quality: q,
                identifier: m.identifier.isEmpty ? nil : m.identifier,
                actionable: m.actionable
            )
        } else {
            fail(
                "activate failed for \(m.fp)",
                fingerprint: m.fp,
                ax_quality: q,
                identifier: m.identifier.isEmpty ? nil : m.identifier,
                actionable: m.actionable
            )
        }
    }

case "set_value":
    guard let value = req.value else {
        fail("set_value requires value")
        break
    }
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("no matching element", count: 0, ax_quality: 0, actionable: false)
    } else if matches.count > 1 {
        fail("ambiguous set_value (\(matches.count) matches)", count: UInt32(matches.count))
    } else {
        let m = matches[0]
        let q = quality(for: m, matchCount: 1)
        if let fp = req.fingerprint, fp != m.fp {
            fail(
                "stale target (expected \(fp), observed \(m.fp))",
                fingerprint: m.fp,
                ax_quality: q,
                identifier: m.identifier.isEmpty ? nil : m.identifier,
                actionable: m.actionable
            )
        } else if setValue(m, value: value) {
            ok(
                "set value on \(m.fp)",
                fingerprint: m.fp,
                value: value,
                ax_quality: q,
                identifier: m.identifier.isEmpty ? nil : m.identifier,
                actionable: m.actionable
            )
        } else {
            fail(
                "set_value failed for \(m.fp)",
                fingerprint: m.fp,
                ax_quality: q,
                identifier: m.identifier.isEmpty ? nil : m.identifier,
                actionable: m.actionable
            )
        }
    }

case "verify_element":
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("verify: element not found", count: 0, ax_quality: 0, actionable: false)
    } else if matches.count > 1 {
        fail("ambiguous verify (\(matches.count) matches)", count: UInt32(matches.count))
    } else {
        let m = matches[0]
        let q = quality(for: m, matchCount: 1)
        if let fp = req.fingerprint, fp != m.fp {
            fail(
                "stale target (expected \(fp), observed \(m.fp))",
                fingerprint: m.fp,
                ax_quality: q,
                identifier: m.identifier.isEmpty ? nil : m.identifier,
                actionable: m.actionable
            )
        } else {
            let observed = readValue(m) ?? axString(m.element, kAXTitleAttribute) ?? m.fp
            if let expected = req.expected_value,
               observed.trimmingCharacters(in: .whitespacesAndNewlines)
                != expected.trimmingCharacters(in: .whitespacesAndNewlines)
            {
                fail(
                    "verify mismatch (expected \(expected), observed \(observed))",
                    fingerprint: m.fp,
                    value: observed,
                    ax_quality: q,
                    identifier: m.identifier.isEmpty ? nil : m.identifier,
                    actionable: m.actionable
                )
            } else {
                ok(
                    "verified \(m.fp)",
                    fingerprint: m.fp,
                    value: observed,
                    ax_quality: q,
                    identifier: m.identifier.isEmpty ? nil : m.identifier,
                    actionable: m.actionable
                )
            }
        }
    }

case "list_matches":
    // Read-only inspection: enumerate every element that satisfies the
    // app/role/title/identifier query, with each candidate's fingerprint,
    // quality score, and value. Unlike resolve_target this never refuses on
    // ambiguity — it is the Inspect step used before approving an action.
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("no matching element", count: 0, ax_quality: 0, actionable: false)
    } else {
        let listing = matches.prefix(50).enumerated().map { idx, m -> String in
            let value = readValue(m) ?? ""
            let q = quality(for: m, matchCount: matches.count)
            let base = value.isEmpty ? "[\(idx)] \(m.fp)" : "[\(idx)] \(m.fp) = \(value)"
            return "\(base) q=\(q)"
        }.joined(separator: " ;; ")
        let best = matches[0]
        ok(
            "matches: \(listing)",
            count: UInt32(matches.count),
            fingerprint: best.fp,
            ax_quality: quality(for: best, matchCount: matches.count),
            identifier: best.identifier.isEmpty ? nil : best.identifier,
            actionable: best.actionable
        )
    }

case "enumerate_children":
    guard let appName = req.app, let appEl = findApp(named: appName, bundleId: req.bundle_id) else {
        fail("app not found")
        break
    }
    let root = searchRoot(appElement: appEl, windowTitle: req.window_title)
    let children = axChildren(root)
    let summary = children.prefix(20).map { child -> String in
        let role = axString(child, kAXRoleAttribute) ?? "?"
        let title = axString(child, kAXTitleAttribute) ?? ""
        let identifier = axString(child, kAXIdentifierAttribute) ?? ""
        if identifier.isEmpty {
            return "\(role):\(title)"
        }
        return "\(role):\(title)#\(identifier)"
    }.joined(separator: "; ")
    ok("children: \(summary)", count: UInt32(children.count))

default:
    fail("unknown op: \(req.op)")
}
