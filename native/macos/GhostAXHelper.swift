#!/usr/bin/env swift
// GhostAXHelper — optional macOS semantic automation helper (Ghost 2.0).
// Build on macOS: swiftc -o ghost-ax-helper native/macos/GhostAXHelper.swift
// Rust calls this binary when present; otherwise falls back to in-process AX in platform/macos.rs.

import Foundation
import AppKit

struct AXRequest: Codable {
    let op: String
    let app: String?
    let role: String?
    let title: String?
}

struct AXResponse: Codable {
    let ok: Bool
    let detail: String
}

func respond(_ resp: AXResponse) {
    let data = try! JSONEncoder().encode(resp)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
}

guard let line = readLine(strippingNewline: true),
      let input = line.data(using: .utf8),
      let req = try? JSONDecoder().decode(AXRequest.self, from: input) else {
    respond(AXResponse(ok: false, detail: "invalid JSON request"))
    exit(1)
}

switch req.op {
case "frontmost_app":
    let app = NSWorkspace.shared.frontmostApplication?.localizedName ?? "unknown"
    respond(AXResponse(ok: true, detail: app))
case "permission_status":
    let trusted = AXIsProcessTrusted()
    respond(AXResponse(ok: trusted, detail: trusted ? "accessibility granted" : "accessibility denied"))
default:
    respond(AXResponse(ok: false, detail: "unknown op: \(req.op)"))
}
