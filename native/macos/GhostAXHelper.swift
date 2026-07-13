#!/usr/bin/env swift
// GhostAXHelper — macOS Accessibility semantic automation helper (Ghost 2.0).
// Build: swiftc -o ghost-ax-helper native/macos/GhostAXHelper.swift
// Protocol: one JSON object per line on stdin → one JSON object per line on stdout.

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
}

struct AXResponse: Codable {
    let ok: Bool
    let detail: String
    let match_count: UInt32?
    let fingerprint: String?
    let value: String?
}

func respond(_ resp: AXResponse) {
    let data = try! JSONEncoder().encode(resp)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
}

func fail(_ detail: String, count: UInt32? = nil) {
    respond(AXResponse(ok: false, detail: detail, match_count: count, fingerprint: nil, value: nil))
}

func ok(_ detail: String, count: UInt32? = nil, fingerprint: String? = nil, value: String? = nil) {
    respond(AXResponse(ok: true, detail: detail, match_count: count, fingerprint: fingerprint, value: value))
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

func fingerprint(_ element: AXUIElement) -> String {
    let role = axString(element, kAXRoleAttribute) ?? "?"
    let title = axString(element, kAXTitleAttribute) ?? axString(element, kAXDescriptionAttribute) ?? ""
    let frame = axFrame(element)
    let framePart: String
    if let f = frame {
        framePart = "\(f.x),\(f.y),\(f.w),\(f.h)"
    } else {
        framePart = "noframe"
    }
    return "\(role)|\(title)|\(framePart)"
}

struct Match {
    let element: AXUIElement
    let fp: String
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

func findApp(named name: String) -> AXUIElement? {
    let apps = NSWorkspace.shared.runningApplications
    for app in apps {
        if let localized = app.localizedName, localized.lowercased().contains(name.lowercased()) {
            return AXUIElementCreateApplication(app.processIdentifier)
        }
        if let bundle = app.bundleIdentifier, bundle.lowercased().contains(name.lowercased()) {
            return AXUIElementCreateApplication(app.processIdentifier)
        }
    }
    return nil
}

func collectMatches(appElement: AXUIElement, role: String, title: String?, maxDepth: Int) -> [Match] {
    var results: [Match] = []
    func walk(_ element: AXUIElement, depth: Int) {
        if depth > maxDepth { return }
        if roleMatches(element, role) && titleMatches(element, title) {
            results.append(Match(element: element, fp: fingerprint(element)))
        }
        for child in axChildren(element) {
            walk(child, depth: depth + 1)
        }
    }
    walk(appElement, depth: 0)
    return results
}

func resolve(req: AXRequest) -> [Match] {
    guard let appName = req.app, !appName.isEmpty else { return [] }
    guard let role = req.role, !role.isEmpty else { return [] }
    guard let appEl = findApp(named: appName) else { return [] }
    return collectMatches(appElement: appEl, role: role, title: req.title, maxDepth: 8)
}

func activate(_ match: Match) -> Bool {
    var err = AXUIElementPerformAction(match.element, kAXRaiseAction as CFString)
    if err != .success {
        err = AXUIElementSetAttributeValue(match.element, kAXFocusedAttribute as CFString, true as CFTypeRef)
    }
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
    let app = NSWorkspace.shared.frontmostApplication?.localizedName ?? "unknown"
    ok(app)

case "resolve_target":
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("no matching element", count: 0)
    } else if matches.count > 1 {
        fail("ambiguous semantic target (\(matches.count) matches)", count: UInt32(matches.count))
    } else {
        ok("resolved \(matches[0].fp)", count: 1, fingerprint: matches[0].fp)
    }

case "activate_element":
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("no matching element", count: 0)
    } else if matches.count > 1 {
        fail("ambiguous activate (\(matches.count) matches)", count: UInt32(matches.count))
    } else {
        let m = matches[0]
        if let fp = req.fingerprint, fp != m.fp {
            fail("stale target (expected \(fp), observed \(m.fp))", fingerprint: m.fp)
        } else if activate(m) {
            ok("activated \(m.fp)", fingerprint: m.fp)
        } else {
            fail("activate failed for \(m.fp)", fingerprint: m.fp)
        }
    }

case "set_value":
    guard let value = req.value else {
        fail("set_value requires value")
        break
    }
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("no matching element", count: 0)
    } else if matches.count > 1 {
        fail("ambiguous set_value (\(matches.count) matches)", count: UInt32(matches.count))
    } else {
        let m = matches[0]
        if let fp = req.fingerprint, fp != m.fp {
            fail("stale target (expected \(fp), observed \(m.fp))", fingerprint: m.fp)
        } else if setValue(m, value: value) {
            ok("set value on \(m.fp)", fingerprint: m.fp, value: value)
        } else {
            fail("set_value failed for \(m.fp)", fingerprint: m.fp)
        }
    }

case "verify_element":
    let matches = resolve(req: req)
    if matches.isEmpty {
        fail("verify: element not found", count: 0)
    } else if matches.count > 1 {
        fail("ambiguous verify (\(matches.count) matches)", count: UInt32(matches.count))
    } else {
        let m = matches[0]
        if let fp = req.fingerprint, fp != m.fp {
            fail("stale target (expected \(fp), observed \(m.fp))", fingerprint: m.fp)
        } else {
            let observed = readValue(m) ?? axString(m.element, kAXTitleAttribute) ?? m.fp
            if let expected = req.expected_value, observed.trimmingCharacters(in: .whitespacesAndNewlines) != expected.trimmingCharacters(in: .whitespacesAndNewlines) {
                fail("verify mismatch (expected \(expected), observed \(observed))", fingerprint: m.fp, value: observed)
            } else {
                ok("verified \(m.fp)", fingerprint: m.fp, value: observed)
            }
        }
    }

case "enumerate_children":
    guard let appName = req.app, let appEl = findApp(named: appName) else {
        fail("app not found")
        break
    }
    let children = axChildren(appEl)
    let summary = children.prefix(20).map { child -> String in
        let role = axString(child, kAXRoleAttribute) ?? "?"
        let title = axString(child, kAXTitleAttribute) ?? ""
        return "\(role):\(title)"
    }.joined(separator: "; ")
    ok("children: \(summary)", count: UInt32(children.count))

default:
    fail("unknown op: \(req.op)")
}
