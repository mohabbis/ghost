import Foundation
import OSLog

private let bridgeProtocolVersion = 1

private struct BridgeRequest<Payload: Encodable>: Encodable {
    let version: Int
    let requestID: String
    let command: String
    let payload: Payload

    enum CodingKeys: String, CodingKey {
        case version
        case requestID = "request_id"
        case command
        case payload
    }
}

private struct EmptyPayload: Codable {}
private struct PasswordPayload: Codable { let password: String }
private struct ScanPayload: Codable { let folder: String }
private struct ScanIDPayload: Codable {
    let scanID: ScanID
    enum CodingKeys: String, CodingKey { case scanID = "scan_id" }
}
private struct PlanIDPayload: Codable {
    let planID: PlanID
    enum CodingKeys: String, CodingKey { case planID = "plan_id" }
}
private struct ExecutePayload: Codable {
    let planID: PlanID
    let approvalToken: String

    enum CodingKeys: String, CodingKey {
        case planID = "plan_id"
        case approvalToken = "approval_token"
    }
}
private struct ExecutionIDPayload: Codable {
    let executionID: ExecutionID
    enum CodingKeys: String, CodingKey { case executionID = "execution_id" }
}

private struct BridgeErrorPayload: Codable {
    let code: String
    let message: String
    let recoverable: Bool
}

actor ProcessGhostCoreClient: GhostCoreClient {
    private let logger = Logger(subsystem: "com.muhammadrafiq.ghost.native", category: "RustBridge")
    private let process: Process
    private let inputHandle: FileHandle
    private let outputHandle: FileHandle
    private let errorHandle: FileHandle
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder

    private var outputBuffer = Data()
    private var pendingUnary: [String: CheckedContinuation<Data, Error>] = [:]
    private var executionStreams: [
        String: AsyncThrowingStream<ExecutionEvent, Error>.Continuation
    ] = [:]
    private var undoStreams: [
        String: AsyncThrowingStream<UndoEvent, Error>.Continuation
    ] = [:]

    init(bridgeURL: URL? = nil) throws {
        let executable: URL
        if let bridgeURL {
            executable = bridgeURL
        } else {
            executable = try Self.resolveBridgeURL()
        }
        guard FileManager.default.isExecutableFile(atPath: executable.path) else {
            throw GhostCoreError.bridgeUnavailable(
                "No executable bridge was found at \(executable.path). Run ./script/build_and_run.sh."
            )
        }

        let process = Process()
        let inputPipe = Pipe()
        let outputPipe = Pipe()
        let errorPipe = Pipe()
        process.executableURL = executable
        process.standardInput = inputPipe
        process.standardOutput = outputPipe
        process.standardError = errorPipe

        self.process = process
        self.inputHandle = inputPipe.fileHandleForWriting
        self.outputHandle = outputPipe.fileHandleForReading
        self.errorHandle = errorPipe.fileHandleForReading
        self.encoder = JSONEncoder()
        self.decoder = JSONDecoder()

        try process.run()

        outputHandle.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            Task {
                await self?.consumeOutput(data)
            }
        }

        errorHandle.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty, let text = String(data: data, encoding: .utf8) else { return }
            Task {
                await self?.logBridgeError(text)
            }
        }

        process.terminationHandler = { [weak self] terminated in
            Task {
                await self?.bridgeTerminated(status: terminated.terminationStatus)
            }
        }
    }

    deinit {
        outputHandle.readabilityHandler = nil
        errorHandle.readabilityHandler = nil
        if process.isRunning {
            process.terminate()
        }
    }

    func handshake() async throws -> CoreHandshake {
        try await request(command: "handshake", payload: EmptyPayload())
    }

    func authStatus() async throws -> AuthStatus {
        try await request(command: "auth_status", payload: EmptyPayload())
    }

    func unlock(password: String) async throws -> AuthStatus {
        try await request(command: "auth_unlock", payload: PasswordPayload(password: password))
    }

    func lock() async throws -> AuthStatus {
        try await request(command: "auth_lock", payload: EmptyPayload())
    }

    func scan(folder: URL) async throws -> ScanResult {
        try await request(command: "scan", payload: ScanPayload(folder: folder.path))
    }

    func createPlan(scanID: ScanID) async throws -> ExecutionPlan {
        try await request(command: "create_plan", payload: ScanIDPayload(scanID: scanID))
    }

    func approve(planID: PlanID) async throws -> ApprovalToken {
        try await request(command: "approve", payload: PlanIDPayload(planID: planID))
    }

    func execute(
        planID: PlanID,
        approvalToken: ApprovalToken
    ) async -> AsyncThrowingStream<ExecutionEvent, Error> {
        let requestID = UUID().uuidString
        var streamContinuation: AsyncThrowingStream<ExecutionEvent, Error>.Continuation!
        let stream = AsyncThrowingStream<ExecutionEvent, Error> { continuation in
            streamContinuation = continuation
        }
        executionStreams[requestID] = streamContinuation
        streamContinuation.onTermination = { [weak self] _ in
            Task { await self?.removeExecutionStream(requestID) }
        }

        do {
            try writeRequest(
                BridgeRequest(
                    version: bridgeProtocolVersion,
                    requestID: requestID,
                    command: "execute",
                    payload: ExecutePayload(
                        planID: planID,
                        approvalToken: approvalToken.serializedToken
                    )
                )
            )
        } catch {
            executionStreams.removeValue(forKey: requestID)
            streamContinuation.finish(throwing: error)
        }
        return stream
    }

    func receipt(executionID: ExecutionID) async throws -> ExecutionReceipt {
        try await request(
            command: "receipt",
            payload: ExecutionIDPayload(executionID: executionID)
        )
    }

    func undo(executionID: ExecutionID) async -> AsyncThrowingStream<UndoEvent, Error> {
        let requestID = UUID().uuidString
        var streamContinuation: AsyncThrowingStream<UndoEvent, Error>.Continuation!
        let stream = AsyncThrowingStream<UndoEvent, Error> { continuation in
            streamContinuation = continuation
        }
        undoStreams[requestID] = streamContinuation
        streamContinuation.onTermination = { [weak self] _ in
            Task { await self?.removeUndoStream(requestID) }
        }

        do {
            try writeRequest(
                BridgeRequest(
                    version: bridgeProtocolVersion,
                    requestID: requestID,
                    command: "undo",
                    payload: ExecutionIDPayload(executionID: executionID)
                )
            )
        } catch {
            undoStreams.removeValue(forKey: requestID)
            streamContinuation.finish(throwing: error)
        }
        return stream
    }

    func listExecutions() async throws -> [ExecutionSummary] {
        try await request(command: "list_executions", payload: EmptyPayload())
    }

    private func request<Payload: Encodable, Response: Decodable>(
        command: String,
        payload: Payload
    ) async throws -> Response {
        let requestID = UUID().uuidString
        let responseData: Data = try await withCheckedThrowingContinuation { continuation in
            pendingUnary[requestID] = continuation
            do {
                try writeRequest(
                    BridgeRequest(
                        version: bridgeProtocolVersion,
                        requestID: requestID,
                        command: command,
                        payload: payload
                    )
                )
            } catch {
                pendingUnary.removeValue(forKey: requestID)
                continuation.resume(throwing: error)
            }
        }

        do {
            return try decoder.decode(Response.self, from: responseData)
        } catch {
            throw GhostCoreError.invalidResponse(
                "Could not decode \(command) response: \(error.localizedDescription)"
            )
        }
    }

    private func writeRequest<Payload: Encodable>(_ request: BridgeRequest<Payload>) throws {
        guard process.isRunning else {
            throw GhostCoreError.processTerminated("bridge process is not running")
        }
        var data = try encoder.encode(request)
        data.append(0x0A)
        try inputHandle.write(contentsOf: data)
    }

    private func consumeOutput(_ data: Data) {
        guard !data.isEmpty else { return }
        outputBuffer.append(data)

        while let newline = outputBuffer.firstIndex(of: 0x0A) {
            let line = outputBuffer.prefix(upTo: newline)
            outputBuffer.removeSubrange(...newline)
            guard !line.isEmpty else { continue }
            handleLine(Data(line))
        }
    }

    private func handleLine(_ data: Data) {
        do {
            guard
                let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                let requestID = object["request_id"] as? String,
                let kind = object["kind"] as? String
            else {
                throw GhostCoreError.invalidResponse("missing request_id or kind")
            }

            switch kind {
            case "event":
                try handleEvent(object["payload"], requestID: requestID)
            case "result":
                try handleResult(object["payload"], requestID: requestID)
            case "error":
                handleRemoteError(object["error"], requestID: requestID)
            default:
                throw GhostCoreError.invalidResponse("unknown response kind \(kind)")
            }
        } catch {
            logger.error("Bridge response parse failure: \(error.localizedDescription, privacy: .public)")
        }
    }

    private func handleEvent(_ rawPayload: Any?, requestID: String) throws {
        guard let payload = rawPayload as? [String: Any], let event = payload["event"] as? String else {
            throw GhostCoreError.invalidResponse("event payload is missing its event tag")
        }

        if let continuation = executionStreams[requestID] {
            switch event {
            case "execution_started":
                guard
                    let rawID = payload["execution_id"] as? String,
                    let totalSteps = payload["total_steps"] as? Int
                else {
                    throw GhostCoreError.invalidResponse("invalid execution_started event")
                }
                continuation.yield(
                    .started(
                        executionID: ExecutionID(rawValue: rawID),
                        totalSteps: totalSteps
                    )
                )
            case "execution_progress":
                guard
                    let rawID = payload["execution_id"] as? String,
                    let completed = payload["completed"] as? Int,
                    let total = payload["total"] as? Int,
                    let applied = payload["applied"] as? Int,
                    let skipped = payload["skipped"] as? Int,
                    let failed = payload["failed"] as? Int
                else {
                    throw GhostCoreError.invalidResponse("invalid execution_progress event")
                }
                continuation.yield(
                    .progress(
                        executionID: ExecutionID(rawValue: rawID),
                        completed: completed,
                        total: total,
                        applied: applied,
                        skipped: skipped,
                        failed: failed,
                        currentStep: payload["current_step"] as? String
                    )
                )
            default:
                throw GhostCoreError.invalidResponse("unknown execution event \(event)")
            }
            return
        }

        if let continuation = undoStreams[requestID] {
            switch event {
            case "undo_started":
                guard let rawID = payload["execution_id"] as? String else {
                    throw GhostCoreError.invalidResponse("invalid undo_started event")
                }
                continuation.yield(.started(executionID: ExecutionID(rawValue: rawID)))
            default:
                throw GhostCoreError.invalidResponse("unknown undo event \(event)")
            }
        }
    }

    private func handleResult(_ rawPayload: Any?, requestID: String) throws {
        let payloadData = try serializedPayload(rawPayload)

        if let continuation = pendingUnary.removeValue(forKey: requestID) {
            continuation.resume(returning: payloadData)
            return
        }

        if let continuation = executionStreams.removeValue(forKey: requestID) {
            do {
                let receipt = try decoder.decode(ExecutionReceipt.self, from: payloadData)
                continuation.yield(.completed(receipt))
                continuation.finish()
            } catch {
                continuation.finish(
                    throwing: GhostCoreError.invalidResponse(
                        "Could not decode execution result: \(error.localizedDescription)"
                    )
                )
            }
            return
        }

        if let continuation = undoStreams.removeValue(forKey: requestID) {
            do {
                let report = try decoder.decode(UndoReport.self, from: payloadData)
                continuation.yield(.completed(report))
                continuation.finish()
            } catch {
                continuation.finish(
                    throwing: GhostCoreError.invalidResponse(
                        "Could not decode undo result: \(error.localizedDescription)"
                    )
                )
            }
        }
    }

    private func handleRemoteError(_ rawError: Any?, requestID: String) {
        let remote: GhostCoreError
        if
            let object = rawError as? [String: Any],
            let code = object["code"] as? String,
            let message = object["message"] as? String
        {
            remote = .remote(
                code: code,
                message: message,
                recoverable: object["recoverable"] as? Bool ?? false
            )
        } else {
            remote = .invalidResponse("bridge error payload was malformed")
        }

        if let continuation = pendingUnary.removeValue(forKey: requestID) {
            continuation.resume(throwing: remote)
        }
        if let continuation = executionStreams.removeValue(forKey: requestID) {
            continuation.finish(throwing: remote)
        }
        if let continuation = undoStreams.removeValue(forKey: requestID) {
            continuation.finish(throwing: remote)
        }
    }

    private func serializedPayload(_ rawPayload: Any?) throws -> Data {
        let object = rawPayload ?? NSNull()
        guard JSONSerialization.isValidJSONObject(object) || object is NSNull else {
            throw GhostCoreError.invalidResponse("payload is not valid JSON")
        }
        if object is NSNull {
            return Data("null".utf8)
        }
        return try JSONSerialization.data(withJSONObject: object)
    }

    private func removeExecutionStream(_ requestID: String) {
        executionStreams.removeValue(forKey: requestID)
    }

    private func removeUndoStream(_ requestID: String) {
        undoStreams.removeValue(forKey: requestID)
    }

    private func logBridgeError(_ text: String) {
        logger.error("\(text, privacy: .public)")
    }

    private func bridgeTerminated(status: Int32) {
        let error = GhostCoreError.processTerminated("exit status \(status)")
        let unary = Array(pendingUnary.values)
        let executions = Array(executionStreams.values)
        let undos = Array(undoStreams.values)
        pendingUnary.removeAll()
        executionStreams.removeAll()
        undoStreams.removeAll()

        unary.forEach { $0.resume(throwing: error) }
        executions.forEach { $0.finish(throwing: error) }
        undos.forEach { $0.finish(throwing: error) }
    }

    private nonisolated static func resolveBridgeURL() throws -> URL {
        let fileManager = FileManager.default
        let environment = ProcessInfo.processInfo.environment

        if let override = environment["GHOST_CORE_BRIDGE"], !override.isEmpty {
            return URL(fileURLWithPath: override)
        }

        if let executable = Bundle.main.executableURL {
            let bundled = executable
                .deletingLastPathComponent()
                .appendingPathComponent("ghost_core_bridge")
            if fileManager.isExecutableFile(atPath: bundled.path) {
                return bundled
            }
        }

        let workingDirectory = URL(fileURLWithPath: fileManager.currentDirectoryPath)
        let candidates = [
            workingDirectory.appendingPathComponent("src-tauri/target/debug/ghost_core_bridge"),
            workingDirectory.appendingPathComponent("../../src-tauri/target/debug/ghost_core_bridge"),
            workingDirectory.appendingPathComponent("../../../src-tauri/target/debug/ghost_core_bridge")
        ]
        if let candidate = candidates.first(where: {
            fileManager.isExecutableFile(atPath: $0.path)
        }) {
            return candidate
        }

        throw GhostCoreError.bridgeUnavailable(
            "Set GHOST_CORE_BRIDGE or build src-tauri/src/bin/ghost_core_bridge.rs."
        )
    }
}
