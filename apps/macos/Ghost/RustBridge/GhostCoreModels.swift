import Foundation

struct Identifier<Tag>: RawRepresentable, Codable, Hashable, Sendable, CustomStringConvertible {
    let rawValue: String

    init(rawValue: String) {
        self.rawValue = rawValue
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        rawValue = try container.decode(String.self)
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }

    var description: String { rawValue }
}

enum ScanIdentifierTag {}
enum PlanIdentifierTag {}
enum ZoneIdentifierTag {}
enum ExecutionIdentifierTag {}

typealias ScanID = Identifier<ScanIdentifierTag>
typealias PlanID = Identifier<PlanIdentifierTag>
typealias ZoneID = Identifier<ZoneIdentifierTag>
typealias ExecutionID = Identifier<ExecutionIdentifierTag>

struct AuthStatus: Codable, Equatable, Sendable {
    let configured: Bool
    let unlocked: Bool

    static let unconfigured = AuthStatus(configured: false, unlocked: true)
}

struct CoreHandshake: Codable, Equatable, Sendable {
    let protocolVersion: Int
    let coreVersion: String
    let capabilities: [String]
    let auth: AuthStatus

    enum CodingKeys: String, CodingKey {
        case protocolVersion = "protocol_version"
        case coreVersion = "core_version"
        case capabilities
        case auth
    }
}

struct ScanFile: Codable, Identifiable, Hashable, Sendable {
    let path: String
    let name: String
    let fileExtension: String?
    let size: UInt64

    var id: String { path }

    enum CodingKeys: String, CodingKey {
        case path
        case name
        case fileExtension = "extension"
        case size
    }
}

struct ScanResult: Codable, Sendable {
    let scanID: ScanID
    let folder: String
    let fileCount: Int
    let totalBytes: UInt64
    let files: [ScanFile]
    let truncated: Bool

    enum CodingKeys: String, CodingKey {
        case scanID = "scan_id"
        case folder
        case fileCount = "file_count"
        case totalBytes = "total_bytes"
        case files
        case truncated
    }
}

enum PlanStepKind: String, Codable, Sendable {
    case createFolder = "create_folder"
    case moveFile = "move_file"
    case renameFile = "rename_file"
    case verifyPath = "verify_path"
    case unsupported
}

enum PolicyDecisionKind: String, Codable, Sendable {
    case allow
    case requireConfirmation = "require_confirmation"
    case deny
}

enum RiskLevel: String, Codable, Sendable {
    case low
    case medium
    case high
    case critical
}

struct PlanStep: Codable, Identifiable, Hashable, Sendable {
    let id: String
    let label: String
    let kind: PlanStepKind
    let sourcePath: String?
    let destinationPath: String?
    let decision: PolicyDecisionKind
    let decisionReason: String?
    let risk: RiskLevel?
    let rulePath: String?
    let confidence: Double
    let reason: String
    let conflict: String?

    enum CodingKeys: String, CodingKey {
        case id
        case label
        case kind
        case sourcePath = "source_path"
        case destinationPath = "destination_path"
        case decision
        case decisionReason = "decision_reason"
        case risk
        case rulePath = "rule_path"
        case confidence
        case reason
        case conflict
    }
}

struct PlanSummary: Codable, Hashable, Sendable {
    let filesScanned: Int
    let totalSteps: Int
    let createFolder: Int
    let moveFile: Int
    let renameFile: Int
    let conflicts: Int
    let lowConfidence: Int
    let denied: Int
    let needsConfirmation: Int
    let skipped: Int

    enum CodingKeys: String, CodingKey {
        case filesScanned = "files_scanned"
        case totalSteps = "total_steps"
        case createFolder = "create_folder"
        case moveFile = "move_file"
        case renameFile = "rename_file"
        case conflicts
        case lowConfidence = "low_confidence"
        case denied
        case needsConfirmation = "needs_confirmation"
        case skipped
    }
}

struct ExecutionPlan: Codable, Identifiable, Hashable, Sendable {
    let planID: PlanID
    let zoneID: ZoneID
    let planHash: String
    let title: String
    let folder: String
    let summary: PlanSummary
    let steps: [PlanStep]

    var id: PlanID { planID }

    enum CodingKeys: String, CodingKey {
        case planID = "plan_id"
        case zoneID = "zone_id"
        case planHash = "plan_hash"
        case title
        case folder
        case summary
        case steps
    }
}

struct ApprovalToken: Codable, Hashable, Sendable {
    let planID: PlanID
    let serializedToken: String
    let expiresAt: String

    enum CodingKeys: String, CodingKey {
        case planID = "plan_id"
        case serializedToken = "serialized_token"
        case expiresAt = "expires_at"
    }
}

enum VerificationStatus: String, Codable, Sendable {
    case verified
    case failed
    case skipped
    case notApplicable = "not_applicable"
}

struct StepVerification: Codable, Hashable, Sendable {
    let stepID: String
    let label: String
    let expected: String
    let observed: String
    let status: VerificationStatus
    let continueExecution: Bool

    enum CodingKeys: String, CodingKey {
        case stepID = "step_id"
        case label
        case expected
        case observed
        case status
        case continueExecution = "continue_execution"
    }
}

struct ReceiptStep: Codable, Identifiable, Hashable, Sendable {
    let id: String
    let label: String
    let outcome: String
    let verification: StepVerification
}

struct ExecutionReceipt: Codable, Identifiable, Hashable, Sendable {
    let executionID: ExecutionID?
    let planID: PlanID
    let planTitle: String
    let startedAt: String
    let finishedAt: String
    let applied: Int
    let skipped: Int
    let failed: Int
    let stoppedEarly: Bool
    let stopReason: String?
    let undoAvailable: Bool
    let steps: [ReceiptStep]
    let auditEventCount: Int

    var id: String { executionID?.rawValue ?? planID.rawValue }

    enum CodingKeys: String, CodingKey {
        case executionID = "execution_id"
        case planID = "plan_id"
        case planTitle = "plan_title"
        case startedAt = "started_at"
        case finishedAt = "finished_at"
        case applied
        case skipped
        case failed
        case stoppedEarly = "stopped_early"
        case stopReason = "stop_reason"
        case undoAvailable = "undo_available"
        case steps
        case auditEventCount = "audit_event_count"
    }
}

struct UndoReport: Codable, Hashable, Sendable {
    let reverted: Int
    let skipped: Int
    let failed: Int
}

struct ExecutionSummary: Codable, Identifiable, Hashable, Sendable {
    let id: ExecutionID
    let zoneID: ZoneID
    let createdAt: String
    let applied: Int
    let skipped: Int
    let failed: Int
    let sealed: Bool
    let finished: Bool

    enum CodingKeys: String, CodingKey {
        case id
        case zoneID = "zone_id"
        case createdAt = "created_at"
        case applied
        case skipped
        case failed
        case sealed
        case finished
    }
}
